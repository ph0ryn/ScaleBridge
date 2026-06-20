import { renderDashboard } from "../components/dashboard";
import { replaceChildren } from "../components/dom";
import { loadDashboardData } from "../features/dashboard/data";
import { subscribeToDashboardEvents } from "../features/dashboard/subscriptions";
import { startWatcher, stopWatcher } from "../lib/tauri";
import { createInitialAppState, type AppState } from "./state";

export function mountApp(root: HTMLElement): void {
  const state = createInitialAppState();
  let refreshTimer: number | null = null;
  let unlistenDashboardEvents = (): void => {};

  const render = (): void => {
    replaceChildren(root, [
      renderDashboard(state, {
        onRefresh: () => {
          void refreshDashboard(state, render);
        },
        onStartWatcher: () => {
          void runBackendAction(state, render, startWatcher);
        },
        onStopWatcher: () => {
          void runBackendAction(state, render, stopWatcher);
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

async function runBackendAction(
  state: AppState,
  render: () => void,
  action: () => Promise<unknown>,
): Promise<void> {
  state.saving = true;
  state.error = null;
  render();

  try {
    await action();
  } catch (error) {
    state.error = String(error);

    if (error instanceof Error) {
      state.error = error.message;
    }

    state.backendAvailable = false;
  }

  await refreshDashboard(state, render);
}
