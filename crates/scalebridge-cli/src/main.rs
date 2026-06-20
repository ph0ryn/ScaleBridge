use clap::{Parser, Subcommand};
use scalebridge_core::{ControlAckKind, PacketParser, ParsedPacket, WeightStatus};

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
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Parse { packet_hex } => parse_hex_packet(&packet_hex),
    }
}

fn parse_hex_packet(packet_hex: &str) -> Result<(), String> {
    let bytes = decode_hex_packet(packet_hex)?;
    let parsed = PacketParser::parse_notification(&bytes).map_err(|error| error.to_string())?;

    print_parsed_packet(&parsed);

    Ok(())
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

fn format_status(status: WeightStatus) -> &'static str {
    match status {
        WeightStatus::Stable => "stable",
        WeightStatus::Dynamic => "dynamic",
        WeightStatus::Overload => "overload",
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
