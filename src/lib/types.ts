export type WatcherStatus =
  | "starting"
  | "watching"
  | "connecting"
  | "connected"
  | "subscribed"
  | "stopping"
  | "stopped";

export type WeightStatus = "stable" | "dynamic" | "overload";

export type PacketDirection = "inbound" | "outbound";

export interface DeviceProfile {
  family: string;
  service_uuid: string | null;
  notify_uuid: string | null;
  write_uuid: string | null;
}

export interface DeviceInfo {
  id: string;
  address: string;
  name: string | null;
  service_uuids: string[];
  profile: DeviceProfile;
}

export interface Measurement {
  weight_raw: number;
  weight_kg: number;
  impedance: number;
  encrypted_impedance: number;
  fat_mode: number;
  status: WeightStatus;
}

export interface MeasurementEvent {
  device: DeviceInfo;
  measured_at: string;
  measurement: Measurement;
  raw_bytes: number[];
  characteristic_uuid: string | null;
}

export interface AppStatus {
  watcherStatus: WatcherStatus;
  watcherRunning: boolean;
  latestMeasurement: MeasurementEvent | null;
  lastError: string | null;
}

export interface WatcherStatusResponse {
  status: WatcherStatus;
}

export interface AutostartStatus {
  enabled: boolean;
}

export interface DeviceRecord {
  id: number;
  model: string | null;
  name: string | null;
  address: string | null;
  service_uuids_json: string;
  first_seen_at: string;
  last_seen_at: string;
}

export interface MeasurementRecord {
  id: number;
  device_id: number | null;
  measured_at: string;
  weight_kg: number | null;
  impedance: number | null;
  encrypted_impedance: number | null;
  stable: boolean;
  raw_packet_id: number | null;
}

export interface RawPacketRecord {
  id: number;
  device_id: number | null;
  seen_at: string;
  direction: PacketDirection;
  characteristic_uuid: string | null;
  hex: string;
  parser: string | null;
  parsed_json: string | null;
}

export interface DashboardData {
  autostart: AutostartStatus;
  devices: DeviceRecord[];
  rawPackets: RawPacketRecord[];
  measurements: MeasurementRecord[];
  status: AppStatus;
}
