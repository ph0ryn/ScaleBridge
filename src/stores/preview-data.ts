import type { DashboardData } from "../lib/types";

export function createPreviewData(errorMessage: string): DashboardData {
  const now = new Date();
  const previous = new Date(now.valueOf() - 1000 * 60 * 22);
  const older = new Date(now.valueOf() - 1000 * 60 * 60 * 7);

  return {
    autostart: { enabled: false },
    devices: [
      {
        address: "CB-Preview-Scale",
        first_seen_at: older.toISOString(),
        id: 1,
        last_seen_at: previous.toISOString(),
        model: "T9120",
        name: "eufy T9120",
        service_uuids_json: '["0000fff0-0000-1000-8000-00805f9b34fb"]',
      },
    ],
    events: [
      {
        context_json: null,
        created_at: now.toISOString(),
        id: 2,
        level: "warn",
        message: `Backend preview mode: ${errorMessage}`,
      },
      {
        context_json: null,
        created_at: previous.toISOString(),
        id: 1,
        level: "info",
        message: "watcher status changed: scanning",
      },
    ],
    measurements: [
      {
        device_id: 1,
        encrypted_impedance: 169256000,
        id: 2,
        impedance: 4840,
        measured_at: previous.toISOString(),
        raw_packet_id: 2,
        stable: true,
        weight_kg: 53,
      },
      {
        device_id: 1,
        encrypted_impedance: 169253810,
        id: 1,
        impedance: 4818,
        measured_at: older.toISOString(),
        raw_packet_id: 1,
        stable: true,
        weight_kg: 53.2,
      },
    ],
    rawPackets: [
      {
        characteristic_uuid: "0000fff4-0000-1000-8000-00805f9b34fb",
        device_id: 1,
        direction: "inbound",
        hex: "cfe812b414b3b69f00000f",
        id: 2,
        parsed_json: null,
        parser: "t9120_live",
        seen_at: previous.toISOString(),
      },
      {
        characteristic_uuid: "0000fff1-0000-1000-8000-00805f9b34fb",
        device_id: 1,
        direction: "outbound",
        hex: "e60101e6",
        id: 1,
        parsed_json: null,
        parser: null,
        seen_at: older.toISOString(),
      },
    ],
    status: {
      lastError: errorMessage,
      latestMeasurement: {
        characteristic_uuid: "0000fff4-0000-1000-8000-00805f9b34fb",
        device: {
          address: "CB-Preview-Scale",
          id: "preview-scale",
          name: "eufy T9120",
          profile: {
            family: "T9120",
            notify_uuid: "0000fff4-0000-1000-8000-00805f9b34fb",
            service_uuid: "0000fff0-0000-1000-8000-00805f9b34fb",
            write_uuid: "0000fff1-0000-1000-8000-00805f9b34fb",
          },
          service_uuids: ["0000fff0-0000-1000-8000-00805f9b34fb"],
        },
        measured_at: previous.toISOString(),
        measurement: {
          encrypted_impedance: 169256000,
          fat_mode: 0,
          impedance: 4840,
          status: "stable",
          weight_kg: 53,
          weight_raw: 530,
        },
        raw_bytes: [207, 232, 18, 180, 20, 179, 182, 159, 0, 0, 15],
      },
      watcherRunning: false,
      watcherStatus: "stopped",
    },
  };
}
