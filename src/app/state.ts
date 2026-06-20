import type { DashboardData } from "../lib/types";

export interface AppState {
  backendAvailable: boolean;
  data: DashboardData | null;
  error: string | null;
  loading: boolean;
  saving: boolean;
}

export function createInitialAppState(): AppState {
  return {
    backendAvailable: true,
    data: null,
    error: null,
    loading: true,
    saving: false,
  };
}
