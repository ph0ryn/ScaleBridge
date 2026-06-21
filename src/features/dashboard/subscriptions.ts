import { type BackendEventName, listenToBackendEvent } from "../../lib/tauri";

const backendEventNames: BackendEventName[] = [
  "dashboard://refresh-requested",
  "watcher://status-changed",
  "watcher://device-seen",
  "watcher://packet-received",
  "watcher://measurement-created",
  "watcher://error",
];

export async function subscribeToDashboardEvents(onEvent: () => void): Promise<() => void> {
  try {
    const unlistenFunctions = await Promise.all(
      backendEventNames.map((eventName) => listenToBackendEvent(eventName, onEvent)),
    );

    return () => {
      for (const unlisten of unlistenFunctions) {
        unlisten();
      }
    };
  } catch {
    return () => {};
  }
}
