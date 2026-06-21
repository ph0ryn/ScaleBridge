import { renderDashboard } from "../components/dashboard";
import { replaceChildren } from "../components/dom";
import { loadDashboardData } from "../features/dashboard/data";
import { subscribeToDashboardEvents } from "../features/dashboard/subscriptions";
import { setAutostartEnabled, setScanIntervalSettings } from "../lib/tauri";
import { createInitialAppState, type AppState, type DashboardView } from "./state";

import type { ScanIntervalSettings } from "../lib/types";

export function mountApp(root: HTMLElement): void {
  const state = createInitialAppState();
  let refreshTimer: number | null = null;
  let unlistenDashboardEvents = (): void => {};

  const render = (): void => {
    replaceChildren(root, [
      renderDashboard(state, {
        onSelectView: (view: DashboardView) => {
          state.activeView = view;
          render();
        },
        onSetAutostartEnabled: (enabled: boolean) => {
          void runAutostartAction(state, render, enabled);
        },
        onSetScanIntervalSettings: (settings: ScanIntervalSettings) => {
          void runScanIntervalSettingsAction(state, render, settings);
        },
      }),
    ]);
  };

  const scheduleRefresh = (): void => {
    if (refreshTimer !== null) {
      window.clearTimeout(refreshTimer);
    }

    refreshTimer = window.setTimeout(() => {
      refreshTimer = null;
      void refreshDashboard(state, render);
    }, 150);
  };

  render();
  void refreshDashboard(state, render);

  void subscribeToDashboardEvents(scheduleRefresh).then((unlisten) => {
    unlistenDashboardEvents = unlisten;
  });

  window.addEventListener("beforeunload", () => {
    unlistenDashboardEvents();
  });
}

async function refreshDashboard(state: AppState, render: () => void): Promise<void> {
  state.loading = true;
  render();

  const load = await loadDashboardData();

  state.backendAvailable = load.backendAvailable;
  state.data = load.data;
  state.error = load.error;
  state.loading = false;
  state.saving = false;
  render();
}

async function runAutostartAction(
  state: AppState,
  render: () => void,
  enabled: boolean,
): Promise<void> {
  state.saving = true;
  state.error = null;
  render();

  try {
    const autostart = await setAutostartEnabled(enabled);

    if (state.data) {
      state.data.autostart = autostart;
    }
  } catch (error) {
    state.error = String(error);

    if (error instanceof Error) {
      state.error = error.message;
    }

    state.backendAvailable = false;
  }

  await refreshDashboard(state, render);
}

async function runScanIntervalSettingsAction(
  state: AppState,
  render: () => void,
  settings: ScanIntervalSettings,
): Promise<void> {
  state.saving = true;
  state.error = null;
  render();

  try {
    const scanIntervals = await setScanIntervalSettings(settings);

    if (state.data) {
      state.data.scanIntervals = scanIntervals;
    }
  } catch (error) {
    state.error = String(error);

    if (error instanceof Error) {
      state.error = error.message;
    }

    state.backendAvailable = false;
  }

  await refreshDashboard(state, render);
}
