import { invoke } from "@tauri-apps/api/core";
import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  AppStatus,
  AutostartStatus,
  DeviceRecord,
  MeasurementRecord,
  RawPacketRecord,
  ScanIntervalSettings,
  WatcherStatusResponse,
} from "./types";

export type BackendEventName =
  | "dashboard://refresh-requested"
  | "watcher://status-changed"
  | "watcher://device-seen"
  | "watcher://packet-received"
  | "watcher://measurement-created"
  | "watcher://error";

export async function getCurrentStatus(): Promise<AppStatus> {
  return invoke<AppStatus>("get_current_status");
}

export async function listRecentMeasurements(limit: number): Promise<MeasurementRecord[]> {
  return invoke<MeasurementRecord[]>("list_recent_measurements", { limit });
}

export async function listDevices(): Promise<DeviceRecord[]> {
  return invoke<DeviceRecord[]>("list_devices");
}

export async function listRecentRawPackets(limit: number): Promise<RawPacketRecord[]> {
  return invoke<RawPacketRecord[]>("list_recent_raw_packets", { limit });
}

export async function startWatcher(): Promise<WatcherStatusResponse> {
  return invoke<WatcherStatusResponse>("start_watcher");
}

export async function stopWatcher(): Promise<WatcherStatusResponse> {
  return invoke<WatcherStatusResponse>("stop_watcher");
}

export async function getAutostartStatus(): Promise<AutostartStatus> {
  return invoke<AutostartStatus>("get_autostart_status");
}

export async function setAutostartEnabled(enabled: boolean): Promise<AutostartStatus> {
  return invoke<AutostartStatus>("set_autostart_enabled", { enabled });
}

export async function getScanIntervalSettings(): Promise<ScanIntervalSettings> {
  return invoke<ScanIntervalSettings>("get_scan_interval_settings");
}

export async function setScanIntervalSettings(
  settings: ScanIntervalSettings,
): Promise<ScanIntervalSettings> {
  return invoke<ScanIntervalSettings>("set_scan_interval_settings", { settings });
}

export async function listenToBackendEvent(
  eventName: BackendEventName,
  handler: (event: Event<unknown>) => void,
): Promise<UnlistenFn> {
  return listen(eventName, handler);
}
