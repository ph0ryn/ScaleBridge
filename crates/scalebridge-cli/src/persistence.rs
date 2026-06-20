use std::path::Path;
use std::sync::{Arc, Mutex};

use scalebridge_core::{
    DeviceInfo, MeasurementEvent, PacketDirection, ParsedPacket, RawPacketEvent, WatcherEvent,
    WatcherStatus,
};
use scalebridge_storage::{
    AppEventInsert, DeviceRecord, DeviceUpsert, MeasurementInsert,
    PacketDirection as StoragePacketDirection, RawPacketInsert, Storage,
};
use time::OffsetDateTime;

pub type SharedStorage = Arc<Mutex<Storage>>;

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

    if let Some(measurement) = measurement_from_packet(parsed) {
        storage
            .lock()
            .map_err(|error| error.to_string())?
            .insert_measurement(&MeasurementInsert {
                device_id: None,
                measured_at: now,
                weight_kg: Some(measurement.weight_kg),
                impedance: Some(i64::from(measurement.impedance)),
                encrypted_impedance: Some(i64::from(measurement.encrypted_impedance)),
                stable: measurement.stable(),
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
        WatcherEvent::RawPacket { packet } => persist_raw_packet_event(storage, packet).map(|_| ()),
        WatcherEvent::Measurement { measurement } => {
            persist_measurement_event(storage, measurement)
        }
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

fn persist_measurement_event(
    storage: &SharedStorage,
    measurement: &MeasurementEvent,
) -> Result<(), String> {
    let device = upsert_device(storage, &measurement.device)?;
    let raw_packet_id = storage
        .lock()
        .map_err(|error| error.to_string())?
        .insert_raw_packet(&RawPacketInsert {
            device_id: Some(device.id),
            seen_at: measurement.measured_at,
            direction: StoragePacketDirection::Inbound,
            characteristic_uuid: measurement.characteristic_uuid.clone(),
            hex: hex::encode(&measurement.raw_bytes),
            parser: Some("measurement".to_string()),
            parsed_json: None,
        })
        .map_err(|error| error.to_string())?;

    storage
        .lock()
        .map_err(|error| error.to_string())?
        .insert_measurement(&MeasurementInsert {
            device_id: Some(device.id),
            measured_at: measurement.measured_at,
            weight_kg: Some(measurement.measurement.weight_kg),
            impedance: Some(i64::from(measurement.measurement.impedance)),
            encrypted_impedance: Some(i64::from(measurement.measurement.encrypted_impedance)),
            stable: measurement.measurement.stable(),
            raw_packet_id: Some(raw_packet_id),
        })
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

fn measurement_from_packet(parsed: ParsedPacket) -> Option<scalebridge_core::Measurement> {
    match parsed {
        ParsedPacket::T9120Live { measurement, .. }
        | ParsedPacket::T9120HistoryCandidate { measurement, .. } => Some(measurement),
        ParsedPacket::ControlAck { .. } | ParsedPacket::Unknown { .. } => None,
    }
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
