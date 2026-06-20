use scalebridge_core::{
    DeviceInfo, Measurement, PacketDirection, ParsedPacket, RawPacketEvent, ScaleWatcher,
    ScaleWatcherConfig, WatcherEvent, WatcherStatus,
};
use scalebridge_storage::{
    AppEventInsert, DeviceRecord, DeviceUpsert, MeasurementInsert,
    PacketDirection as StoragePacketDirection, RawPacketInsert,
};
use tauri::{AppHandle, Emitter};
use time::{Duration, OffsetDateTime};

use crate::state::{AppState, BackendState};

struct ParsedMeasurement {
    measured_at: OffsetDateTime,
    measurement: Measurement,
}

pub fn start_watcher(app: AppHandle, app_state: AppState) -> Result<WatcherStatus, String> {
    app_state.with_lock(|state| {
        if state.watcher.is_some() {
            return Ok(state.status);
        }

        state.status = WatcherStatus::Starting;
        let event_app = app.clone();
        let event_state = app_state.clone();
        let handle = ScaleWatcher::spawn(ScaleWatcherConfig::default(), move |event| {
            if let Err(error) = handle_watcher_event(&event_app, &event_state, event) {
                eprintln!("watcher event handling failed: {error}");
            }
        });

        state.watcher = Some(handle);

        Ok(state.status)
    })
}

pub fn stop_watcher(app_state: AppState) -> Result<WatcherStatus, String> {
    app_state.with_lock(|state| {
        if let Some(handle) = state.watcher.take() {
            handle.stop();
        }

        state.status = WatcherStatus::Stopping;

        Ok(state.status)
    })
}

fn handle_watcher_event(
    app: &AppHandle,
    app_state: &AppState,
    event: WatcherEvent,
) -> Result<(), String> {
    app_state.with_lock(|state| {
        update_state_from_event(state, &event);
        persist_watcher_event(state, &event)?;

        Ok(())
    })?;
    emit_watcher_event(app, &event);

    Ok(())
}

fn update_state_from_event(state: &mut BackendState, event: &WatcherEvent) {
    match event {
        WatcherEvent::StatusChanged { status, .. } => {
            state.status = *status;
        }
        WatcherEvent::Measurement { measurement } => {
            state.latest_measurement = Some(measurement.clone());
        }
        WatcherEvent::TransportError { message } => {
            state.last_error = Some(message.clone());
        }
        WatcherEvent::DeviceSeen { .. }
        | WatcherEvent::Connected { .. }
        | WatcherEvent::Disconnected { .. }
        | WatcherEvent::ServicesDiscovered { .. }
        | WatcherEvent::InitWrite { .. }
        | WatcherEvent::RawPacket { .. }
        | WatcherEvent::ParseWarning { .. } => {}
    }
}

fn persist_watcher_event(state: &mut BackendState, event: &WatcherEvent) -> Result<(), String> {
    match event {
        WatcherEvent::DeviceSeen { device }
        | WatcherEvent::Connected { device }
        | WatcherEvent::Disconnected { device } => {
            upsert_device(state, device)?;
        }
        WatcherEvent::ServicesDiscovered { device, .. } => {
            upsert_device(state, device)?;
        }
        WatcherEvent::RawPacket { packet } => {
            let raw_packet_id = persist_raw_packet_event(state, packet)?;
            persist_measurement_from_raw_packet(state, packet, raw_packet_id)?;
        }
        WatcherEvent::Measurement { .. } => {}
        WatcherEvent::StatusChanged { status, message } => {
            persist_app_event(
                state,
                "info",
                &format!("watcher status changed: {}", format_watcher_status(*status)),
                message
                    .as_ref()
                    .map(|message| serde_json::json!({ "message": message }).to_string()),
            )?;
        }
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

            persist_raw_packet_event(state, &raw_packet)?;
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

            persist_app_event(state, "warn", message, Some(context))?;
        }
        WatcherEvent::TransportError { message } => {
            persist_app_event(state, "error", message, None)?;
        }
    }

    Ok(())
}

fn persist_raw_packet_event(
    state: &mut BackendState,
    packet: &RawPacketEvent,
) -> Result<i64, String> {
    let device = upsert_device(state, &packet.device)?;
    let parsed_json = packet
        .parsed
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())?;

    state
        .storage
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
    state: &mut BackendState,
    packet: &RawPacketEvent,
    raw_packet_id: i64,
) -> Result<(), String> {
    let Some(parsed) = &packet.parsed else {
        return Ok(());
    };
    let Some(parsed_measurement) = measurement_from_packet(parsed, packet.seen_at) else {
        return Ok(());
    };
    let device = upsert_device(state, &packet.device)?;

    insert_measurement(
        state,
        Some(device.id),
        Some(raw_packet_id),
        parsed_measurement,
    )
}

fn insert_measurement(
    state: &mut BackendState,
    device_id: Option<i64>,
    raw_packet_id: Option<i64>,
    parsed_measurement: ParsedMeasurement,
) -> Result<(), String> {
    state
        .storage
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
    state: &mut BackendState,
    level: &str,
    message: &str,
    context_json: Option<String>,
) -> Result<(), String> {
    state
        .storage
        .insert_app_event(&AppEventInsert {
            created_at: OffsetDateTime::now_utc(),
            level: level.to_string(),
            message: message.to_string(),
            context_json,
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn upsert_device(state: &mut BackendState, device: &DeviceInfo) -> Result<DeviceRecord, String> {
    let service_uuids_json =
        serde_json::to_string(&device.service_uuids).map_err(|error| error.to_string())?;

    state
        .storage
        .upsert_device(&DeviceUpsert {
            model: Some(format!("{:?}", device.profile.family)),
            name: device.name.clone(),
            address: Some(device.address.clone()),
            service_uuids_json,
            seen_at: OffsetDateTime::now_utc(),
        })
        .map_err(|error| error.to_string())
}

fn emit_watcher_event(app: &AppHandle, event: &WatcherEvent) {
    let event_name = match event {
        WatcherEvent::StatusChanged { .. } => "watcher://status-changed",
        WatcherEvent::DeviceSeen { .. } => "watcher://device-seen",
        WatcherEvent::Connected { .. } | WatcherEvent::Disconnected { .. } => {
            "watcher://status-changed"
        }
        WatcherEvent::ServicesDiscovered { .. } => "watcher://device-seen",
        WatcherEvent::InitWrite { .. } | WatcherEvent::RawPacket { .. } => {
            "watcher://packet-received"
        }
        WatcherEvent::Measurement { .. } => "watcher://measurement-created",
        WatcherEvent::ParseWarning { .. } | WatcherEvent::TransportError { .. } => {
            "watcher://error"
        }
    };

    let _ = app.emit(event_name, event);
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
