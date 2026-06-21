use scalebridge_core::{
    ControlAckKind, PacketDirection, ParsedPacket, WatcherEvent, WatcherStatus, WeightStatus,
};

pub fn print_parsed_packet(parsed: &ParsedPacket) {
    match parsed {
        ParsedPacket::T9120Live {
            measurement,
            checksum,
        } => {
            println!("kind=t9120_live");
            println!("weight_kg={:.2}", measurement.weight_kg);
            println!("weight_raw={}", measurement.weight_raw);
            println!("impedance={}", measurement.impedance);
            println!("encrypted_impedance={}", measurement.encrypted_impedance);
            println!("fat_mode={}", measurement.fat_mode);
            println!("status={}", format_status(measurement.status));
            println!("stable={}", measurement.stable());
            println!("checksum={checksum:02x}");
        }
        ParsedPacket::T9120HistoryCandidate {
            measurement,
            timestamp,
            checksum,
        } => {
            println!("kind=t9120_history_candidate");
            println!("weight_kg={:.2}", measurement.weight_kg);
            println!("weight_raw={}", measurement.weight_raw);
            println!("impedance={}", measurement.impedance);
            println!("encrypted_impedance={}", measurement.encrypted_impedance);
            println!("fat_mode={}", measurement.fat_mode);
            println!("status={}", format_status(measurement.status));
            println!("stable={}", measurement.stable());
            println!("checksum={checksum:02x}");
            println!(
                "measured_at={:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                timestamp.year,
                timestamp.month,
                timestamp.day,
                timestamp.hour,
                timestamp.minute,
                timestamp.second,
            );
        }
        ParsedPacket::ControlAck { ack } => {
            println!("kind=control_ack");
            println!("ack={}", format_ack(ack));
        }
        ParsedPacket::Unknown { bytes } => {
            println!("kind=unknown");
            println!("hex={}", hex::encode(bytes));
        }
    }
}

pub fn print_watcher_event(event: WatcherEvent) {
    match event {
        WatcherEvent::StatusChanged { status, message } => {
            print!("event=status status={}", format_watcher_status(status));

            if let Some(message) = message {
                print!(" message={message}");
            }

            println!();
        }
        WatcherEvent::DeviceSeen { device } => {
            println!(
                "event=device_seen id={} name={} profile={:?}",
                device.id,
                device.name.unwrap_or_else(|| "(unknown)".to_string()),
                device.profile.family,
            );
        }
        WatcherEvent::Connected { device } => {
            println!(
                "event=connected id={} name={}",
                device.id,
                device.name.unwrap_or_else(|| "(unknown)".to_string()),
            );
        }
        WatcherEvent::Disconnected { device } => {
            println!(
                "event=disconnected id={} name={}",
                device.id,
                device.name.unwrap_or_else(|| "(unknown)".to_string()),
            );
        }
        WatcherEvent::ServicesDiscovered { device, services } => {
            println!(
                "event=services_discovered id={} service_count={}",
                device.id,
                services.len(),
            );

            for service in services {
                println!(
                    "service={} primary={} characteristics={}",
                    service.uuid,
                    service.primary,
                    service.characteristics.len(),
                );

                for characteristic in service.characteristics {
                    println!(
                        "characteristic={} service={} properties={}",
                        characteristic.uuid,
                        characteristic.service_uuid,
                        characteristic.properties.join(","),
                    );
                }
            }
        }
        WatcherEvent::InitWrite {
            device,
            characteristic_uuid,
            bytes,
        } => {
            println!(
                "event=init_write id={} characteristic={} hex={}",
                device.id,
                characteristic_uuid,
                hex::encode(bytes),
            );
        }
        WatcherEvent::RawPacket { packet } => {
            println!(
                "event=raw_packet id={} direction={} characteristic={} hex={} parser={}",
                packet.device.id,
                format_direction(packet.direction),
                packet
                    .characteristic_uuid
                    .unwrap_or_else(|| "(unknown)".to_string()),
                hex::encode(packet.bytes),
                packet.parser.unwrap_or_else(|| "(none)".to_string()),
            );
        }
        WatcherEvent::Measurement { measurement } => {
            println!(
                "event=measurement id={} weight_kg={:.2} impedance={} status={}",
                measurement.device.id,
                measurement.measurement.weight_kg,
                measurement.measurement.impedance,
                format_status(measurement.measurement.status),
            );
        }
        WatcherEvent::ParseWarning {
            device,
            message,
            bytes,
        } => {
            println!(
                "event=parse_warning id={} message={} hex={}",
                device.id,
                message,
                hex::encode(bytes),
            );
        }
        WatcherEvent::TransportError { message } => {
            println!("event=transport_error message={message}");
        }
    }
}

fn format_status(status: WeightStatus) -> &'static str {
    match status {
        WeightStatus::Stable => "stable",
        WeightStatus::Dynamic => "dynamic",
        WeightStatus::Overload => "overload",
    }
}

fn format_watcher_status(status: WatcherStatus) -> &'static str {
    match status {
        WatcherStatus::Starting => "starting",
        WatcherStatus::Watching => "watching",
        WatcherStatus::Connecting => "connecting",
        WatcherStatus::Connected => "connected",
        WatcherStatus::Subscribed => "subscribed",
        WatcherStatus::Stopping => "stopping",
        WatcherStatus::Stopped => "stopped",
    }
}

fn format_direction(direction: PacketDirection) -> &'static str {
    match direction {
        PacketDirection::Inbound => "inbound",
        PacketDirection::Outbound => "outbound",
    }
}

fn format_ack(ack: &ControlAckKind) -> &'static str {
    match ack {
        ControlAckKind::TimeSync => "time_sync",
        ControlAckKind::HistorySync => "history_sync",
        ControlAckKind::HistoryDelete => "history_delete",
        ControlAckKind::Disconnect => "disconnect",
    }
}
