use clap::{Parser, Subcommand};
use scalebridge_core::{
    ControlAckKind, PacketDirection, PacketParser, ParsedPacket, ScaleWatcher, ScaleWatcherConfig,
    WatcherEvent, WatcherStatus, WeightStatus,
};

#[derive(Debug, Parser)]
#[command(name = "scalebridge")]
#[command(about = "ScaleBridge debugging CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Parse {
        #[arg(long = "hex")]
        packet_hex: String,
    },
    Watch,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Parse { packet_hex } => parse_hex_packet(&packet_hex),
        Command::Watch => watch().await,
    }
}

fn parse_hex_packet(packet_hex: &str) -> Result<(), String> {
    let bytes = decode_hex_packet(packet_hex)?;
    let parsed = PacketParser::parse_notification(&bytes).map_err(|error| error.to_string())?;

    print_parsed_packet(&parsed);

    Ok(())
}

async fn watch() -> Result<(), String> {
    let config = ScaleWatcherConfig::default();

    println!("watcher=starting");
    println!("hint=press Ctrl+C to stop");

    tokio::select! {
        result = ScaleWatcher::run(config, print_watcher_event) => {
            result.map_err(|error| error.to_string())
        }
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(|error| format!("failed to listen for Ctrl+C: {error}"))?;
            println!("watcher=stopping");
            Ok(())
        }
    }
}

fn decode_hex_packet(packet_hex: &str) -> Result<Vec<u8>, String> {
    let mut cleaned = String::new();

    for token in packet_hex.split(|character: char| {
        character.is_ascii_whitespace() || matches!(character, ',' | ':' | '-')
    }) {
        let normalized = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
            .unwrap_or(token);
        cleaned.push_str(normalized);
    }

    if cleaned.is_empty() {
        return Err("packet hex must not be empty".to_string());
    }

    if cleaned.len() % 2 != 0 {
        return Err("packet hex must contain an even number of digits".to_string());
    }

    hex::decode(cleaned).map_err(|error| format!("invalid packet hex: {error}"))
}

fn print_parsed_packet(parsed: &ParsedPacket) {
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

fn print_watcher_event(event: WatcherEvent) {
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
        WatcherStatus::Scanning => "scanning",
        WatcherStatus::Connecting => "connecting",
        WatcherStatus::Connected => "connected",
        WatcherStatus::Subscribed => "subscribed",
        WatcherStatus::Idle => "idle",
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
