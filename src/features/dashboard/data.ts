import {
  getAutostartStatus,
  getCurrentStatus,
  getScanIntervalSettings,
  listDevices,
  listRecentMeasurements,
  listRecentRawPackets,
} from "../../lib/tauri";
import { createPreviewData } from "../../stores/preview-data";

import type { DashboardData } from "../../lib/types";

const MEASUREMENT_LIMIT = 12;
const RAW_PACKET_LIMIT = 18;

export interface DashboardLoad {
  backendAvailable: boolean;
  data: DashboardData;
  error: string | null;
}

export async function loadDashboardData(): Promise<DashboardLoad> {
  try {
    const [status, measurements, devices, rawPackets, autostart, scanIntervals] = await Promise.all(
      [
        getCurrentStatus(),
        listRecentMeasurements(MEASUREMENT_LIMIT),
        listDevices(),
        listRecentRawPackets(RAW_PACKET_LIMIT),
        getAutostartStatus(),
        getScanIntervalSettings(),
      ],
    );

    return {
      backendAvailable: true,
      data: {
        autostart,
        devices,
        measurements,
        rawPackets,
        scanIntervals,
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
