import type { DashboardData } from "../lib/types";

export type DashboardView = "overview" | "history" | "devices" | "raw-log" | "settings";

export interface AppState {
  activeView: DashboardView;
  backendAvailable: boolean;
  data: DashboardData | null;
  error: string | null;
  loading: boolean;
  saving: boolean;
}

export function createInitialAppState(): AppState {
  return {
    activeView: "overview",
    backendAvailable: true,
    data: null,
    error: null,
    loading: true,
    saving: false,
  };
}
