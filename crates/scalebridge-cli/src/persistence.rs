use std::path::Path;
use std::sync::{Arc, Mutex};

use scalebridge_core::{
    DeviceInfo, Measurement, PacketDirection, ParsedPacket, RawPacketEvent, WatcherEvent,
    WatcherStatus,
};
use scalebridge_storage::{
    AppEventInsert, DeviceRecord, DeviceUpsert, MeasurementInsert,
    PacketDirection as StoragePacketDirection, RawPacketInsert, Storage,
};
use time::{Duration, OffsetDateTime};

pub type SharedStorage = Arc<Mutex<Storage>>;

struct ParsedMeasurement {
    measured_at: OffsetDateTime,
    measurement: Measurement,
}

pub fn open_storage(path: &Path) -> Result<SharedStorage, String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create DB parent directory: {error}"))?;
    }

    let storage = Storage::open(path).map_err(|error| format!("failed to open DB: {error}"))?;

    Ok(Arc::new(Mutex::new(storage)))
}

pub fn persist_parsed_packet(
    storage: &SharedStorage,
    bytes: Vec<u8>,
    parsed: ParsedPacket,
) -> Result<(), String> {
    let now = OffsetDateTime::now_utc();
    let parser = parser_name(&parsed).to_string();
    let parsed_json = serde_json::to_string(&parsed).map_err(|error| error.to_string())?;
    let raw_packet_id = storage
        .lock()
        .map_err(|error| error.to_string())?
        .insert_raw_packet(&RawPacketInsert {
            device_id: None,
            seen_at: now,
            direction: StoragePacketDirection::Inbound,
            characteristic_uuid: None,
            hex: hex::encode(&bytes),
            parser: Some(parser),
            parsed_json: Some(parsed_json),
        })
        .map_err(|error| error.to_string())?;

    if let Some(parsed_measurement) = measurement_from_packet(&parsed, now) {
        storage
            .lock()
            .map_err(|error| error.to_string())?
            .insert_measurement(&MeasurementInsert {
                device_id: None,
                measured_at: parsed_measurement.measured_at,
                weight_kg: Some(parsed_measurement.measurement.weight_kg),
                impedance: Some(i64::from(parsed_measurement.measurement.impedance)),
                encrypted_impedance: Some(i64::from(
                    parsed_measurement.measurement.encrypted_impedance,
                )),
                stable: parsed_measurement.measurement.stable(),
                raw_packet_id: Some(raw_packet_id),
            })
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub fn persist_watcher_event(storage: &SharedStorage, event: &WatcherEvent) {
    let result = match event {
        WatcherEvent::DeviceSeen { device }
        | WatcherEvent::Connected { device }
        | WatcherEvent::Disconnected { device } => upsert_device(storage, device).map(|_| ()),
        WatcherEvent::ServicesDiscovered { device, .. } => {
            upsert_device(storage, device).map(|_| ())
        }
        WatcherEvent::RawPacket { packet } => {
            persist_raw_packet_event(storage, packet).and_then(|raw_packet_id| {
                persist_measurement_from_raw_packet(storage, packet, raw_packet_id)
            })
        }
        WatcherEvent::Measurement { .. } => Ok(()),
        WatcherEvent::StatusChanged { status, message } => persist_app_event(
            storage,
            "info",
            &format!("watcher status changed: {}", format_watcher_status(*status)),
            message
                .as_ref()
                .map(|message| serde_json::json!({ "message": message }).to_string()),
        ),
        WatcherEvent::InitWrite {
            device,
            characteristic_uuid,
            bytes,
        } => {
            let raw_packet = RawPacketEvent {
                device: device.clone(),
                seen_at: OffsetDateTime::now_utc(),
                direction: PacketDirection::Outbound,
                characteristic_uuid: Some(characteristic_uuid.clone()),
                bytes: bytes.clone(),
                parser: None,
                parsed: None,
            };

            persist_raw_packet_event(storage, &raw_packet).map(|_| ())
        }
        WatcherEvent::ParseWarning {
            device,
            message,
            bytes,
        } => {
            let context = serde_json::json!({
                "device_id": device.id,
                "hex": hex::encode(bytes),
            })
            .to_string();

            persist_app_event(storage, "warn", message, Some(context))
        }
        WatcherEvent::TransportError { message } => {
            persist_app_event(storage, "error", message, None)
        }
    };

    if let Err(error) = result {
        eprintln!("event=storage_error message={error}");
    }
}

fn persist_raw_packet_event(
    storage: &SharedStorage,
    packet: &RawPacketEvent,
) -> Result<i64, String> {
    let device = upsert_device(storage, &packet.device)?;
    let parsed_json = packet
        .parsed
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())?;

    storage
        .lock()
        .map_err(|error| error.to_string())?
        .insert_raw_packet(&RawPacketInsert {
            device_id: Some(device.id),
            seen_at: packet.seen_at,
            direction: map_packet_direction(packet.direction),
            characteristic_uuid: packet.characteristic_uuid.clone(),
            hex: hex::encode(&packet.bytes),
            parser: packet.parser.clone(),
            parsed_json,
        })
        .map_err(|error| error.to_string())
}

fn persist_measurement_from_raw_packet(
    storage: &SharedStorage,
    packet: &RawPacketEvent,
    raw_packet_id: i64,
) -> Result<(), String> {
    let Some(parsed) = &packet.parsed else {
        return Ok(());
    };
    let Some(parsed_measurement) = measurement_from_packet(parsed, packet.seen_at) else {
        return Ok(());
    };
    let device = upsert_device(storage, &packet.device)?;

    insert_measurement(
        storage,
        Some(device.id),
        Some(raw_packet_id),
        parsed_measurement,
    )
}

fn insert_measurement(
    storage: &SharedStorage,
    device_id: Option<i64>,
    raw_packet_id: Option<i64>,
    parsed_measurement: ParsedMeasurement,
) -> Result<(), String> {
    storage
        .lock()
        .map_err(|error| error.to_string())?
        .insert_measurement_debounced(
            &MeasurementInsert {
                device_id,
                measured_at: parsed_measurement.measured_at,
                weight_kg: Some(parsed_measurement.measurement.weight_kg),
                impedance: Some(i64::from(parsed_measurement.measurement.impedance)),
                encrypted_impedance: Some(i64::from(
                    parsed_measurement.measurement.encrypted_impedance,
                )),
                stable: parsed_measurement.measurement.stable(),
                raw_packet_id,
            },
            Duration::milliseconds(500),
        )
        .map_err(|error| error.to_string())?;

    Ok(())
}

fn persist_app_event(
    storage: &SharedStorage,
    level: &str,
    message: &str,
    context_json: Option<String>,
) -> Result<(), String> {
    storage
        .lock()
        .map_err(|error| error.to_string())?
        .insert_app_event(&AppEventInsert {
            created_at: OffsetDateTime::now_utc(),
            level: level.to_string(),
            message: message.to_string(),
            context_json,
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn upsert_device(storage: &SharedStorage, device: &DeviceInfo) -> Result<DeviceRecord, String> {
    let service_uuids_json =
        serde_json::to_string(&device.service_uuids).map_err(|error| error.to_string())?;

    storage
        .lock()
        .map_err(|error| error.to_string())?
        .upsert_device(&DeviceUpsert {
            model: Some(format!("{:?}", device.profile.family)),
            name: device.name.clone(),
            address: Some(device.address.clone()),
            service_uuids_json,
            seen_at: OffsetDateTime::now_utc(),
        })
        .map_err(|error| error.to_string())
}

fn map_packet_direction(direction: PacketDirection) -> StoragePacketDirection {
    match direction {
        PacketDirection::Inbound => StoragePacketDirection::Inbound,
        PacketDirection::Outbound => StoragePacketDirection::Outbound,
    }
}

fn measurement_from_packet(
    parsed: &ParsedPacket,
    fallback_measured_at: OffsetDateTime,
) -> Option<ParsedMeasurement> {
    match parsed {
        ParsedPacket::T9120Live { measurement, .. } => Some(ParsedMeasurement {
            measured_at: fallback_measured_at,
            measurement: measurement.clone(),
        }),
        ParsedPacket::T9120HistoryCandidate {
            measurement,
            timestamp,
            ..
        } => Some(ParsedMeasurement {
            measured_at: timestamp
                .to_offset_date_time(local_now().offset())
                .unwrap_or(fallback_measured_at),
            measurement: measurement.clone(),
        }),
        ParsedPacket::ControlAck { .. } | ParsedPacket::Unknown { .. } => None,
    }
}

fn local_now() -> OffsetDateTime {
    time::UtcOffset::current_local_offset()
        .map(|offset| OffsetDateTime::now_utc().to_offset(offset))
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn parser_name(parsed: &ParsedPacket) -> &'static str {
    match parsed {
        ParsedPacket::T9120Live { .. } => "t9120_live",
        ParsedPacket::T9120HistoryCandidate { .. } => "t9120_history_candidate",
        ParsedPacket::ControlAck { .. } => "control_ack",
        ParsedPacket::Unknown { .. } => "unknown",
    }
}

fn format_watcher_status(status: WatcherStatus) -> &'static str {
    match status {
        WatcherStatus::Starting => "starting",
        WatcherStatus::Scanning => "scanning",
        WatcherStatus::Connecting => "connecting",
        WatcherStatus::Connected => "connected",
        WatcherStatus::Subscribed => "subscribed",
        WatcherStatus::Idle => "idle",
        WatcherStatus::Stopping => "stopping",
        WatcherStatus::Stopped => "stopped",
    }
}
