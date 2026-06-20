import {
  getAutostartStatus,
  getCurrentStatus,
  listDevices,
  listRecentEvents,
  listRecentMeasurements,
  listRecentRawPackets,
} from "../../lib/tauri";
import { createPreviewData } from "../../stores/preview-data";

import type { DashboardData } from "../../lib/types";

const MEASUREMENT_LIMIT = 12;
const EVENT_LIMIT = 16;
const RAW_PACKET_LIMIT = 18;

export interface DashboardLoad {
  backendAvailable: boolean;
  data: DashboardData;
  error: string | null;
}

export async function loadDashboardData(): Promise<DashboardLoad> {
  try {
    const [status, measurements, devices, events, rawPackets, autostart] = await Promise.all([
      getCurrentStatus(),
      listRecentMeasurements(MEASUREMENT_LIMIT),
      listDevices(),
      listRecentEvents(EVENT_LIMIT),
      listRecentRawPackets(RAW_PACKET_LIMIT),
      getAutostartStatus(),
    ]);

    return {
      backendAvailable: true,
      data: {
        autostart,
        devices,
        events,
        measurements,
        rawPackets,
        status,
      },
      error: null,
    };
  } catch (error) {
    let message = String(error);

    if (error instanceof Error) {
      message = error.message;
    }

    return {
      backendAvailable: false,
      data: createPreviewData(message),
      error: message,
    };
  }
}
