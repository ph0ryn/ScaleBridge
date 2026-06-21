use std::sync::Arc;
use std::thread;
use std::time::Duration;

use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral as PlatformPeripheral};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{OffsetDateTime, UtcOffset};
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::{
    ControlAckKind, DeviceAdvertisement, DeviceProfile, Measurement, PacketParser, ParsedPacket,
    ProtocolFamily, build_time_sync_command, build_unit_command, uuids,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WatcherStatus {
    Starting,
    Watching,
    Connecting,
    Connected,
    Subscribed,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacteristicInfo {
    pub uuid: String,
    pub service_uuid: String,
    pub properties: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub uuid: String,
    pub primary: bool,
    pub characteristics: Vec<CharacteristicInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub address: String,
    pub name: Option<String>,
    pub service_uuids: Vec<String>,
    pub profile: DeviceProfile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPacketEvent {
    pub device: DeviceInfo,
    #[serde(with = "time::serde::rfc3339")]
    pub seen_at: OffsetDateTime,
    pub direction: PacketDirection,
    pub characteristic_uuid: Option<String>,
    pub bytes: Vec<u8>,
    pub parser: Option<String>,
    pub parsed: Option<ParsedPacket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementEvent {
    pub device: DeviceInfo,
    #[serde(with = "time::serde::rfc3339")]
    pub measured_at: OffsetDateTime,
    pub measurement: Measurement,
    pub raw_bytes: Vec<u8>,
    pub characteristic_uuid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WatcherEvent {
    StatusChanged {
        status: WatcherStatus,
        message: Option<String>,
    },
    DeviceSeen {
        device: DeviceInfo,
    },
    Connected {
        device: DeviceInfo,
    },
    Disconnected {
        device: DeviceInfo,
    },
    ServicesDiscovered {
        device: DeviceInfo,
        services: Vec<ServiceInfo>,
    },
    InitWrite {
        device: DeviceInfo,
        characteristic_uuid: String,
        bytes: Vec<u8>,
    },
    RawPacket {
        packet: RawPacketEvent,
    },
    Measurement {
        measurement: MeasurementEvent,
    },
    ParseWarning {
        device: DeviceInfo,
        message: String,
        bytes: Vec<u8>,
    },
    TransportError {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct ScaleWatcherConfig {
    pub scan_duration: Duration,
    pub rescan_delay: Duration,
    pub connect_timeout: Duration,
    pub service_discovery_timeout: Duration,
    pub notification_idle_timeout: Duration,
}

impl Default for ScaleWatcherConfig {
    fn default() -> Self {
        Self {
            scan_duration: Duration::from_secs(6),
            rescan_delay: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(10),
            service_discovery_timeout: Duration::from_secs(10),
            notification_idle_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("BLE error: {0}")]
    Ble(#[from] btleplug::Error),
    #[error("watcher task failed: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("watcher runtime failed: {0}")]
    Runtime(#[from] std::io::Error),
    #[error("watcher thread panicked")]
    ThreadPanic,
}

pub struct ScaleWatcher;

pub struct ScaleWatcherHandle {
    stop_sender: watch::Sender<bool>,
    join_handle: WatcherJoinHandle,
}

enum WatcherJoinHandle {
    Tokio(tokio::task::JoinHandle<Result<(), WatcherError>>),
    Thread(thread::JoinHandle<Result<(), WatcherError>>),
}

impl ScaleWatcherHandle {
    pub fn stop(&self) {
        let _ = self.stop_sender.send(true);
    }

    pub async fn wait(self) -> Result<(), WatcherError> {
        match self.join_handle {
            WatcherJoinHandle::Tokio(join_handle) => join_handle.await?,
            WatcherJoinHandle::Thread(join_handle) => {
                tokio::task::spawn_blocking(move || {
                    join_handle.join().map_err(|_| WatcherError::ThreadPanic)?
                })
                .await?
            }
        }
    }
}

type EventSink = Arc<dyn Fn(WatcherEvent) + Send + Sync>;

impl ScaleWatcher {
    pub async fn run<F>(config: ScaleWatcherConfig, event_sink: F) -> Result<(), WatcherError>
    where
        F: Fn(WatcherEvent) + Send + Sync + 'static,
    {
        let (_stop_sender, stop_receiver) = watch::channel(false);
        run_watcher(config, Arc::new(event_sink), stop_receiver).await
    }

    #[must_use]
    pub fn spawn<F>(config: ScaleWatcherConfig, event_sink: F) -> ScaleWatcherHandle
    where
        F: Fn(WatcherEvent) + Send + Sync + 'static,
    {
        let (stop_sender, stop_receiver) = watch::channel(false);
        let event_sink = Arc::new(event_sink);
        let join_handle = spawn_watcher_task(config, event_sink, stop_receiver);

        ScaleWatcherHandle {
            stop_sender,
            join_handle,
        }
    }
}

fn spawn_watcher_task(
    config: ScaleWatcherConfig,
    event_sink: EventSink,
    stop_receiver: watch::Receiver<bool>,
) -> WatcherJoinHandle {
    if let Ok(handle) = Handle::try_current() {
        return WatcherJoinHandle::Tokio(
            handle.spawn(async move { run_watcher(config, event_sink, stop_receiver).await }),
        );
    }

    let join_handle = thread::Builder::new()
        .name("scalebridge-watcher".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            runtime.block_on(run_watcher(config, event_sink, stop_receiver))
        })
        .expect("failed to spawn ScaleBridge watcher thread");

    WatcherJoinHandle::Thread(join_handle)
}

async fn run_watcher(
    config: ScaleWatcherConfig,
    event_sink: EventSink,
    mut stop_receiver: watch::Receiver<bool>,
) -> Result<(), WatcherError> {
    emit(
        &event_sink,
        WatcherEvent::StatusChanged {
            status: WatcherStatus::Starting,
            message: None,
        },
    );

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        emit(
            &event_sink,
            WatcherEvent::TransportError {
                message: "no Bluetooth adapters found".to_string(),
            },
        );
    }

    loop {
        if *stop_receiver.borrow() {
            break;
        }

        for adapter in &adapters {
            scan_adapter(adapter, &config, &event_sink, &mut stop_receiver).await?;

            if *stop_receiver.borrow() {
                break;
            }
        }

        emit(
            &event_sink,
            WatcherEvent::StatusChanged {
                status: WatcherStatus::Watching,
                message: None,
            },
        );

        if wait_or_stop(config.rescan_delay, &mut stop_receiver).await {
            break;
        }
    }

    emit(
        &event_sink,
        WatcherEvent::StatusChanged {
            status: WatcherStatus::Stopped,
            message: None,
        },
    );

    Ok(())
}

async fn scan_adapter(
    adapter: &Adapter,
    config: &ScaleWatcherConfig,
    event_sink: &EventSink,
    stop_receiver: &mut watch::Receiver<bool>,
) -> Result<(), WatcherError> {
    let adapter_info = adapter.adapter_info().await.ok();
    emit(
        event_sink,
        WatcherEvent::StatusChanged {
            status: WatcherStatus::Watching,
            message: adapter_info,
        },
    );

    adapter.start_scan(ScanFilter::default()).await?;

    if wait_or_stop(config.scan_duration, stop_receiver).await {
        let _ = adapter.stop_scan().await;
        return Ok(());
    }

    let peripherals = adapter.peripherals().await?;

    for peripheral in peripherals {
        if *stop_receiver.borrow() {
            break;
        }

        let Some(device) = inspect_peripheral(&peripheral).await? else {
            continue;
        };

        emit(
            event_sink,
            WatcherEvent::DeviceSeen {
                device: device.clone(),
            },
        );

        match device.profile.family {
            ProtocolFamily::T9120 => {
                watch_t9120_device(&peripheral, device, config, event_sink, stop_receiver).await?;
            }
            ProtocolFamily::Fff0Unknown
            | ProtocolFamily::T9140V1
            | ProtocolFamily::T9140V2
            | ProtocolFamily::T9140V3
            | ProtocolFamily::T9148OrT9149
            | ProtocolFamily::T9150OrT9130 => {
                discover_candidate_services(&peripheral, device, config, event_sink).await?;
            }
            ProtocolFamily::Unknown => {}
        }
    }

    let _ = adapter.stop_scan().await;

    Ok(())
}

async fn inspect_peripheral(
    peripheral: &PlatformPeripheral,
) -> Result<Option<DeviceInfo>, WatcherError> {
    let properties = peripheral.properties().await?;
    let name = properties.as_ref().and_then(|properties| {
        properties
            .local_name
            .clone()
            .or_else(|| properties.advertisement_name.clone())
    });
    let service_uuids = properties
        .as_ref()
        .map(|properties| {
            properties
                .services
                .iter()
                .map(|uuid| uuid.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let advertisement = DeviceAdvertisement {
        name: name.clone(),
        service_uuids: service_uuids.clone(),
    };
    let profile = DeviceProfile::detect(&advertisement, &[]);

    if profile.family == ProtocolFamily::Unknown {
        return Ok(None);
    }

    Ok(Some(DeviceInfo {
        id: peripheral.id().to_string(),
        address: peripheral.id().to_string(),
        name,
        service_uuids,
        profile,
    }))
}

async fn discover_candidate_services(
    peripheral: &PlatformPeripheral,
    device: DeviceInfo,
    config: &ScaleWatcherConfig,
    event_sink: &EventSink,
) -> Result<(), WatcherError> {
    connect_if_needed(peripheral, config.connect_timeout).await?;
    emit(
        event_sink,
        WatcherEvent::Connected {
            device: device.clone(),
        },
    );

    peripheral
        .discover_services_with_timeout(config.service_discovery_timeout)
        .await?;
    emit(
        event_sink,
        WatcherEvent::ServicesDiscovered {
            device: device.clone(),
            services: collect_services(peripheral),
        },
    );

    let _ = peripheral.disconnect().await;
    emit(event_sink, WatcherEvent::Disconnected { device });

    Ok(())
}

async fn watch_t9120_device(
    peripheral: &PlatformPeripheral,
    device: DeviceInfo,
    config: &ScaleWatcherConfig,
    event_sink: &EventSink,
    stop_receiver: &mut watch::Receiver<bool>,
) -> Result<(), WatcherError> {
    emit(
        event_sink,
        WatcherEvent::StatusChanged {
            status: WatcherStatus::Connecting,
            message: device.name.clone(),
        },
    );

    connect_if_needed(peripheral, config.connect_timeout).await?;
    emit(
        event_sink,
        WatcherEvent::Connected {
            device: device.clone(),
        },
    );

    peripheral
        .discover_services_with_timeout(config.service_discovery_timeout)
        .await?;
    let services = collect_services(peripheral);
    emit(
        event_sink,
        WatcherEvent::ServicesDiscovered {
            device: device.clone(),
            services,
        },
    );

    let characteristics = peripheral.characteristics();
    let notify_characteristic = characteristics
        .iter()
        .find(|characteristic| {
            characteristic.uuid.to_string() == uuids::T9120_NOTIFY
                && characteristic.properties.contains(CharPropFlags::NOTIFY)
        })
        .cloned();
    let write_characteristic = characteristics
        .iter()
        .find(|characteristic| characteristic.uuid.to_string() == uuids::T9120_WRITE)
        .cloned();

    let (Some(notify_characteristic), Some(write_characteristic)) =
        (notify_characteristic, write_characteristic)
    else {
        emit(
            event_sink,
            WatcherEvent::TransportError {
                message: format!(
                    "missing T9120 notify/write characteristic for {}",
                    device.name.clone().unwrap_or_else(|| device.id.clone())
                ),
            },
        );
        let _ = peripheral.disconnect().await;
        return Ok(());
    };

    peripheral.subscribe(&notify_characteristic).await?;
    emit(
        event_sink,
        WatcherEvent::StatusChanged {
            status: WatcherStatus::Subscribed,
            message: Some(notify_characteristic.uuid.to_string()),
        },
    );

    write_t9120_init_sequence(peripheral, &write_characteristic, &device, event_sink).await?;

    emit(
        event_sink,
        WatcherEvent::StatusChanged {
            status: WatcherStatus::Connected,
            message: device.name.clone(),
        },
    );

    let mut notifications = peripheral.notifications().await?;

    loop {
        if *stop_receiver.borrow() {
            break;
        }

        let idle = tokio::time::sleep(config.notification_idle_timeout);
        tokio::pin!(idle);

        tokio::select! {
            notification = notifications.next() => {
                let Some(notification) = notification else {
                    break;
                };
                let should_disconnect = handle_notification(
                    &device,
                    notification.uuid.to_string(),
                    notification.value,
                    event_sink,
                );

                if should_disconnect {
                    break;
                }
            }
            _ = &mut idle => {
                break;
            }
            changed = stop_receiver.changed() => {
                if changed.is_err() || *stop_receiver.borrow() {
                    break;
                }
            }
        }
    }

    let _ = peripheral.disconnect().await;
    emit(event_sink, WatcherEvent::Disconnected { device });

    Ok(())
}

async fn connect_if_needed(
    peripheral: &PlatformPeripheral,
    timeout: Duration,
) -> Result<(), WatcherError> {
    if !peripheral.is_connected().await? {
        peripheral.connect_with_timeout(timeout).await?;
    }

    Ok(())
}

async fn write_t9120_init_sequence(
    peripheral: &PlatformPeripheral,
    characteristic: &btleplug::api::Characteristic,
    device: &DeviceInfo,
    event_sink: &EventSink,
) -> Result<(), WatcherError> {
    tokio::time::sleep(Duration::from_millis(200)).await;
    write_init_packet(
        peripheral,
        characteristic,
        device,
        event_sink,
        &build_unit_command(0),
    )
    .await?;

    tokio::time::sleep(Duration::from_millis(400)).await;
    let now = local_now();
    write_init_packet(
        peripheral,
        characteristic,
        device,
        event_sink,
        &build_time_sync_command(now),
    )
    .await?;

    tokio::time::sleep(Duration::from_millis(600)).await;
    write_init_packet(
        peripheral,
        characteristic,
        device,
        event_sink,
        &[0xf2, 0x00],
    )
    .await?;

    Ok(())
}

async fn write_init_packet(
    peripheral: &PlatformPeripheral,
    characteristic: &btleplug::api::Characteristic,
    device: &DeviceInfo,
    event_sink: &EventSink,
    bytes: &[u8],
) -> Result<(), WatcherError> {
    emit(
        event_sink,
        WatcherEvent::InitWrite {
            device: device.clone(),
            characteristic_uuid: characteristic.uuid.to_string(),
            bytes: bytes.to_vec(),
        },
    );
    emit(
        event_sink,
        WatcherEvent::RawPacket {
            packet: RawPacketEvent {
                device: device.clone(),
                seen_at: OffsetDateTime::now_utc(),
                direction: PacketDirection::Outbound,
                characteristic_uuid: Some(characteristic.uuid.to_string()),
                bytes: bytes.to_vec(),
                parser: None,
                parsed: None,
            },
        },
    );

    peripheral
        .write(characteristic, bytes, WriteType::WithoutResponse)
        .await?;

    Ok(())
}

fn handle_notification(
    device: &DeviceInfo,
    characteristic_uuid: String,
    bytes: Vec<u8>,
    event_sink: &EventSink,
) -> bool {
    let parsed = PacketParser::parse_notification(&bytes);
    let parsed_for_raw = parsed.clone().ok();
    let parser = parsed_for_raw.as_ref().map(|packet| match packet {
        ParsedPacket::T9120Live { .. } => "t9120_live",
        ParsedPacket::T9120HistoryCandidate { .. } => "t9120_history_candidate",
        ParsedPacket::ControlAck { .. } => "control_ack",
        ParsedPacket::Unknown { .. } => "unknown",
    });

    emit(
        event_sink,
        WatcherEvent::RawPacket {
            packet: RawPacketEvent {
                device: device.clone(),
                seen_at: OffsetDateTime::now_utc(),
                direction: PacketDirection::Inbound,
                characteristic_uuid: Some(characteristic_uuid.clone()),
                bytes: bytes.clone(),
                parser: parser.map(str::to_string),
                parsed: parsed_for_raw.clone(),
            },
        },
    );

    match parsed {
        Ok(ParsedPacket::T9120Live { measurement, .. }) => {
            emit_measurement(
                device,
                characteristic_uuid,
                bytes,
                OffsetDateTime::now_utc(),
                measurement,
                event_sink,
            );
            false
        }
        Ok(ParsedPacket::T9120HistoryCandidate {
            measurement,
            timestamp,
            ..
        }) => {
            let measured_at = timestamp
                .to_offset_date_time(local_now().offset())
                .unwrap_or_else(OffsetDateTime::now_utc);

            emit_measurement(
                device,
                characteristic_uuid,
                bytes,
                measured_at,
                measurement,
                event_sink,
            );
            false
        }
        Ok(ParsedPacket::ControlAck {
            ack: ControlAckKind::Disconnect,
        }) => true,
        Ok(ParsedPacket::ControlAck { .. }) | Ok(ParsedPacket::Unknown { .. }) => false,
        Err(error) => {
            emit(
                event_sink,
                WatcherEvent::ParseWarning {
                    device: device.clone(),
                    message: error.to_string(),
                    bytes,
                },
            );
            false
        }
    }
}

fn emit_measurement(
    device: &DeviceInfo,
    characteristic_uuid: String,
    bytes: Vec<u8>,
    measured_at: OffsetDateTime,
    measurement: Measurement,
    event_sink: &EventSink,
) {
    emit(
        event_sink,
        WatcherEvent::Measurement {
            measurement: MeasurementEvent {
                device: device.clone(),
                measured_at,
                measurement,
                raw_bytes: bytes,
                characteristic_uuid: Some(characteristic_uuid),
            },
        },
    );
}

fn collect_services(peripheral: &PlatformPeripheral) -> Vec<ServiceInfo> {
    peripheral
        .services()
        .into_iter()
        .map(|service| ServiceInfo {
            uuid: service.uuid.to_string(),
            primary: service.primary,
            characteristics: service
                .characteristics
                .into_iter()
                .map(|characteristic| CharacteristicInfo {
                    uuid: characteristic.uuid.to_string(),
                    service_uuid: characteristic.service_uuid.to_string(),
                    properties: characteristic_properties(characteristic.properties),
                })
                .collect(),
        })
        .collect()
}

fn characteristic_properties(properties: CharPropFlags) -> Vec<String> {
    let mut names = Vec::new();

    if properties.contains(CharPropFlags::BROADCAST) {
        names.push("broadcast");
    }
    if properties.contains(CharPropFlags::READ) {
        names.push("read");
    }
    if properties.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE) {
        names.push("write_without_response");
    }
    if properties.contains(CharPropFlags::WRITE) {
        names.push("write");
    }
    if properties.contains(CharPropFlags::NOTIFY) {
        names.push("notify");
    }
    if properties.contains(CharPropFlags::INDICATE) {
        names.push("indicate");
    }

    names.into_iter().map(str::to_string).collect()
}

fn local_now() -> OffsetDateTime {
    UtcOffset::current_local_offset()
        .map(|offset| OffsetDateTime::now_utc().to_offset(offset))
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn emit(event_sink: &EventSink, event: WatcherEvent) {
    event_sink(event);
}

async fn wait_or_stop(duration: Duration, stop_receiver: &mut watch::Receiver<bool>) -> bool {
    let sleep = tokio::time::sleep(duration);
    tokio::pin!(sleep);

    tokio::select! {
        _ = &mut sleep => false,
        changed = stop_receiver.changed() => changed.is_err() || *stop_receiver.borrow(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceProfile, Measurement, WeightStatus};

    #[test]
    fn serializes_watching_status_as_public_status() {
        let json = serde_json::to_value(WatcherStatus::Watching).unwrap();

        assert_eq!(json, "watching");
    }

    #[test]
    fn serializes_measurement_event_time_as_rfc3339_string() {
        let measured_at = OffsetDateTime::from_unix_timestamp(1_766_194_280).unwrap();
        let event = MeasurementEvent {
            device: DeviceInfo {
                id: "test-device".to_string(),
                address: "test-address".to_string(),
                name: Some("test scale".to_string()),
                service_uuids: Vec::new(),
                profile: DeviceProfile::t9120(),
            },
            measured_at,
            measurement: Measurement {
                weight_raw: 532,
                weight_kg: 53.2,
                impedance: 5880,
                encrypted_impedance: 0,
                fat_mode: 0,
                status: WeightStatus::Stable,
            },
            raw_bytes: Vec::new(),
            characteristic_uuid: None,
        };

        let json = serde_json::to_value(event).unwrap();

        assert!(json["measured_at"].is_string());
    }
}
