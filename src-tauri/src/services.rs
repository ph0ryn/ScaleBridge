use scalebridge_core::{
    DeviceInfo, Measurement, PacketDirection, ParsedPacket, ProtocolFamily, RawPacketEvent,
    ScaleWatcher, ScaleWatcherConfig, WatcherEvent, WatcherStatus, WeightStatus,
};
use scalebridge_storage::{
    DeviceRecord, DeviceUpsert, MeasurementInsert, PacketDirection as StoragePacketDirection,
    RawPacketInsert,
};
use tauri::{AppHandle, Emitter};
use time::{Duration, OffsetDateTime};

use crate::state::{AppState, BackendState, LiveMeasurementPhase, LiveMeasurementStatus};

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
        state.live_measurement = LiveMeasurementStatus::idle();

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

            if *status == WatcherStatus::Stopped {
                state.live_measurement = LiveMeasurementStatus::idle();
            }
        }
        WatcherEvent::Connected { device } => {
            if device.profile.family == ProtocolFamily::T9120 {
                state.live_measurement =
                    LiveMeasurementStatus::measuring(device.clone(), OffsetDateTime::now_utc());
            }
        }
        WatcherEvent::Disconnected { .. } => {
            state.live_measurement = LiveMeasurementStatus::idle();
        }
        WatcherEvent::Measurement { measurement } => {
            state.latest_measurement = Some(measurement.clone());
            state.live_measurement = live_status_from_measurement(state, measurement);
        }
        WatcherEvent::TransportError { message } => {
            state.last_error = Some(message.clone());
            state.live_measurement = LiveMeasurementStatus::idle();
        }
        WatcherEvent::DeviceSeen { .. }
        | WatcherEvent::ServicesDiscovered { .. }
        | WatcherEvent::InitWrite { .. }
        | WatcherEvent::RawPacket { .. }
        | WatcherEvent::ParseWarning { .. } => {}
    }
}

fn live_status_from_measurement(
    state: &BackendState,
    measurement: &scalebridge_core::MeasurementEvent,
) -> LiveMeasurementStatus {
    let now = OffsetDateTime::now_utc();

    LiveMeasurementStatus {
        phase: live_phase_for_weight_status(measurement.measurement.status),
        device: Some(measurement.device.clone()),
        measurement: Some(measurement.measurement.clone()),
        measured_at: Some(measurement.measured_at),
        started_at: state.live_measurement.started_at.or(Some(now)),
        updated_at: Some(now),
    }
}

fn live_phase_for_weight_status(status: WeightStatus) -> LiveMeasurementPhase {
    match status {
        WeightStatus::Stable => LiveMeasurementPhase::Stable,
        WeightStatus::Dynamic => LiveMeasurementPhase::Measuring,
        WeightStatus::Overload => LiveMeasurementPhase::Overload,
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
        WatcherEvent::Measurement { .. } | WatcherEvent::StatusChanged { .. } => {}
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
        WatcherEvent::ParseWarning { .. } | WatcherEvent::TransportError { .. } => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use scalebridge_core::{DeviceProfile, MeasurementEvent};
    use scalebridge_storage::Storage;

    #[test]
    fn connected_supported_device_marks_live_measurement_as_measuring() {
        let mut state = sample_backend_state();
        let device = sample_device();

        update_state_from_event(
            &mut state,
            &WatcherEvent::Connected {
                device: device.clone(),
            },
        );

        assert_eq!(
            state.live_measurement.phase,
            LiveMeasurementPhase::Measuring
        );
        assert_eq!(
            state
                .live_measurement
                .device
                .as_ref()
                .map(|device| device.id.as_str()),
            Some(device.id.as_str())
        );
        assert!(state.live_measurement.started_at.is_some());
        assert!(state.live_measurement.updated_at.is_some());
        assert!(state.live_measurement.measurement.is_none());
        assert!(state.live_measurement.measured_at.is_none());
    }

    #[test]
    fn measurement_events_update_live_phase_and_value() {
        let mut state = sample_backend_state();

        update_state_from_event(
            &mut state,
            &WatcherEvent::Measurement {
                measurement: sample_measurement_event(WeightStatus::Dynamic),
            },
        );
        assert_eq!(
            state.live_measurement.phase,
            LiveMeasurementPhase::Measuring
        );
        assert_eq!(
            state
                .live_measurement
                .measurement
                .as_ref()
                .map(|value| value.weight_kg),
            Some(53.2)
        );
        assert!(state.live_measurement.measured_at.is_some());

        update_state_from_event(
            &mut state,
            &WatcherEvent::Measurement {
                measurement: sample_measurement_event(WeightStatus::Stable),
            },
        );
        assert_eq!(state.live_measurement.phase, LiveMeasurementPhase::Stable);

        update_state_from_event(
            &mut state,
            &WatcherEvent::Measurement {
                measurement: sample_measurement_event(WeightStatus::Overload),
            },
        );
        assert_eq!(state.live_measurement.phase, LiveMeasurementPhase::Overload);
    }

    #[test]
    fn disconnected_and_stopped_clear_live_measurement() {
        let mut state = sample_backend_state();
        let device = sample_device();

        update_state_from_event(
            &mut state,
            &WatcherEvent::Connected {
                device: device.clone(),
            },
        );
        update_state_from_event(&mut state, &WatcherEvent::Disconnected { device });

        assert_eq!(state.live_measurement.phase, LiveMeasurementPhase::Idle);
        assert!(state.live_measurement.device.is_none());

        update_state_from_event(
            &mut state,
            &WatcherEvent::Measurement {
                measurement: sample_measurement_event(WeightStatus::Dynamic),
            },
        );
        update_state_from_event(
            &mut state,
            &WatcherEvent::StatusChanged {
                status: WatcherStatus::Stopped,
                message: None,
            },
        );

        assert_eq!(state.live_measurement.phase, LiveMeasurementPhase::Idle);
        assert!(state.live_measurement.device.is_none());
    }

    #[test]
    fn transport_error_clears_live_measurement() {
        let mut state = sample_backend_state();

        update_state_from_event(
            &mut state,
            &WatcherEvent::Measurement {
                measurement: sample_measurement_event(WeightStatus::Dynamic),
            },
        );
        update_state_from_event(
            &mut state,
            &WatcherEvent::TransportError {
                message: "lost connection".to_string(),
            },
        );

        assert_eq!(state.live_measurement.phase, LiveMeasurementPhase::Idle);
        assert_eq!(state.last_error.as_deref(), Some("lost connection"));
    }

    fn sample_backend_state() -> BackendState {
        BackendState {
            storage: Storage::open_in_memory().unwrap(),
            watcher: None,
            status: WatcherStatus::Stopped,
            live_measurement: LiveMeasurementStatus::idle(),
            latest_measurement: None,
            last_error: None,
        }
    }

    fn sample_device() -> DeviceInfo {
        DeviceInfo {
            id: "test-device".to_string(),
            address: "test-address".to_string(),
            name: Some("test scale".to_string()),
            service_uuids: Vec::new(),
            profile: DeviceProfile::t9120(),
        }
    }

    fn sample_measurement_event(status: WeightStatus) -> MeasurementEvent {
        MeasurementEvent {
            device: sample_device(),
            measured_at: OffsetDateTime::from_unix_timestamp(1_766_194_280).unwrap(),
            measurement: Measurement {
                weight_raw: 532,
                weight_kg: 53.2,
                impedance: 5880,
                encrypted_impedance: 0,
                fat_mode: 0,
                status,
            },
            raw_bytes: Vec::new(),
            characteristic_uuid: None,
        }
    }
}
