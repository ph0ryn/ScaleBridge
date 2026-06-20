use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use scalebridge_core::{PacketParser, ScaleWatcher, ScaleWatcherConfig};

use crate::hex_input::decode_hex_packet;
use crate::output::{print_parsed_packet, print_watcher_event};
use crate::persistence::{open_storage, persist_parsed_packet, persist_watcher_event};

mod hex_input;
mod output;
mod persistence;

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
        #[arg(long = "db")]
        db_path: Option<PathBuf>,
    },
    Watch {
        #[arg(long = "db")]
        db_path: Option<PathBuf>,
        #[arg(long = "no-db")]
        no_db: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli.command {
        Command::Parse {
            packet_hex,
            db_path,
        } => parse_hex_packet(&packet_hex, db_path.as_deref()),
        Command::Watch { db_path, no_db } => watch(db_path.as_deref(), no_db).await,
    }
}

fn parse_hex_packet(packet_hex: &str, db_path: Option<&Path>) -> Result<(), String> {
    let bytes = decode_hex_packet(packet_hex)?;
    let parsed = PacketParser::parse_notification(&bytes).map_err(|error| error.to_string())?;

    print_parsed_packet(&parsed);

    if let Some(db_path) = db_path {
        let storage = open_storage(db_path)?;
        persist_parsed_packet(&storage, bytes, parsed)?;
        println!("db_path={}", db_path.display());
    }

    Ok(())
}

async fn watch(db_path: Option<&Path>, no_db: bool) -> Result<(), String> {
    if no_db && db_path.is_some() {
        return Err("--db and --no-db cannot be used together".to_string());
    }

    let config = ScaleWatcherConfig::default();
    let storage = if no_db {
        None
    } else if let Some(db_path) = db_path {
        Some(open_storage(db_path)?)
    } else {
        None
    };

    println!("watcher=starting");
    println!("hint=press Ctrl+C to stop");

    if let Some(db_path) = db_path {
        println!("db_path={}", db_path.display());
    }

    tokio::select! {
        result = ScaleWatcher::run(config, move |event| {
            print_watcher_event(event.clone());

            if let Some(storage) = &storage {
                persist_watcher_event(storage, &event);
            }
        }) => {
            result.map_err(|error| error.to_string())
        }
        signal = tokio::signal::ctrl_c() => {
            signal.map_err(|error| format!("failed to listen for Ctrl+C: {error}"))?;
            println!("watcher=stopping");
            Ok(())
        }
    }
}
