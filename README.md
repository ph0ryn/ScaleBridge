# ScaleBridge

ScaleBridge is a lightweight macOS menu bar app for collecting BLE scale
measurements locally.

It runs in the background, watches for supported scales, connects when a scale
wakes up, stores measurements in SQLite, and opens the UI only when the menu bar
window is requested.

## Features

- Background BLE watcher for supported eufy scale profiles.
- On-demand menu bar window instead of an always-running WebView.
- Local SQLite storage for measurements, devices, and raw packets.
- Rust core shared by the desktop app and the debugging CLI.
- Raw packet capture for future device support and parser fixes.
- No cloud login or server sync in the core workflow.

## Architecture

```text
src/           frontend display layer
src-tauri/     Tauri shell, tray, autostart, and native commands
crates/        Rust BLE core, storage, and CLI crates
docs/          protocol and implementation notes
```

The frontend displays state through Tauri commands. BLE scanning, packet
parsing, database writes, and macOS integration stay on the Rust side.

## CLI

```text
scalebridge watch
```

The CLI uses the same Rust core as the desktop app and is intended for live
debugging, raw packet dumps, and device compatibility work.

## Privacy

ScaleBridge is local-first. Measurement data and raw BLE packets are stored on
the user's Mac by default.
