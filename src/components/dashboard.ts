import {
  formatCount,
  formatDateTime,
  formatDeviceName,
  formatElapsed,
  formatImpedance,
  formatStatusLabel,
  formatWeight,
  truncateMiddle,
} from "../lib/format";
import { createElement, createSvgIcon, type Child } from "./dom";

import type { AppState } from "../app/state";
import type {
  AppEventRecord,
  DashboardData,
  DeviceRecord,
  MeasurementRecord,
  RawPacketRecord,
  WatcherStatus,
} from "../lib/types";

const PLAY_ICON = "M8 5v14l11-7-11-7Z";
const STOP_ICON = "M6 6h12v12H6z";
const REFRESH_ICON = "M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6";

export interface DashboardHandlers {
  onRefresh: () => void;
  onStartWatcher: () => void;
  onStopWatcher: () => void;
}

interface ActionButtonOptions {
  disabled: boolean;
  iconPath: string;
  label: string;
  onClick: () => void;
}

export function renderDashboard(state: AppState, handlers: DashboardHandlers): HTMLElement {
  let content = renderLoadingDashboard(handlers);

  if (state.data) {
    content = renderLoadedDashboard(state.data, state, handlers);
  }

  return createElement("main", { className: "app-shell" }, [
    renderSidebar(),
    createElement("div", { className: "workspace" }, content),
  ]);
}

function renderLoadedDashboard(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): Child[] {
  return [
    renderTopbar(data, state, handlers),
    renderAlert(data, state),
    createElement("section", { className: "hero-grid" }, [
      renderLatestMeasurement(data),
      renderConnectionPanel(data, state, handlers),
      renderAutostartPanel(data),
    ]),
    createElement("section", { className: "content-grid" }, [
      renderMeasurementsPanel(data.measurements),
      renderDevicesPanel(data.devices),
      renderRawPacketsPanel(data.rawPackets),
      renderEventsPanel(data.events),
    ]),
  ];
}

function renderLoadingDashboard(handlers: DashboardHandlers): Child[] {
  return [
    renderLoadingTopbar(handlers),
    createElement("section", { className: "hero-grid" }, [
      renderSkeletonPanel("Latest measurement"),
      renderSkeletonPanel("Connection"),
      renderSkeletonPanel("Autostart"),
    ]),
    createElement("section", { className: "content-grid" }, [
      renderSkeletonPanel("Measurement history"),
      renderSkeletonPanel("Detected devices"),
      renderSkeletonPanel("Raw log"),
      renderSkeletonPanel("App events"),
    ]),
  ];
}

function renderSidebar(): HTMLElement {
  return createElement("aside", { ariaLabel: "ScaleBridge sections", className: "sidebar" }, [
    createElement("div", { className: "brand" }, [
      createElement("span", { className: "brand-mark", text: "SB" }),
      createElement("div", { className: "brand-copy" }, [
        createElement("strong", { text: "ScaleBridge" }),
        createElement("span", { text: "Local BLE scale monitor" }),
      ]),
    ]),
    createElement("nav", { className: "nav-list" }, [
      renderNavLink("Overview", "#overview", true),
      renderNavLink("History", "#measurements", false),
      renderNavLink("Devices", "#devices", false),
      renderNavLink("Raw log", "#raw-log", false),
      renderNavLink("Settings", "#settings", false),
    ]),
  ]);
}

function renderNavLink(label: string, href: string, selected: boolean): HTMLElement {
  let className = "nav-link";

  if (selected) {
    className = "nav-link selected";
  }

  const link = createElement("a", {
    className,
    text: label,
  });

  link.href = href;

  return link;
}

function renderTopbar(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
  const startButton = renderActionButton({
    disabled: data.status.watcherRunning || state.saving || !state.backendAvailable,
    iconPath: PLAY_ICON,
    label: "Start",
    onClick: handlers.onStartWatcher,
  });
  const stopButton = renderActionButton({
    disabled: !data.status.watcherRunning || state.saving || !state.backendAvailable,
    iconPath: STOP_ICON,
    label: "Stop",
    onClick: handlers.onStopWatcher,
  });
  const refreshButton = renderActionButton({
    disabled: state.loading,
    iconPath: REFRESH_ICON,
    label: "Refresh",
    onClick: handlers.onRefresh,
  });

  return createElement("header", { className: "topbar", id: "overview" }, [
    createElement("div", { className: "topbar-copy" }, [
      createElement("h1", { text: "ScaleBridge" }),
      createElement("span", {
        text: `${formatCount(data.measurements.length, "measurement")} loaded`,
      }),
    ]),
    createElement("div", { className: "topbar-actions" }, [
      renderStatusPill(
        formatStatusLabel(data.status.watcherStatus),
        toneForStatus(data.status.watcherStatus),
      ),
      startButton,
      stopButton,
      refreshButton,
    ]),
  ]);
}

function renderLoadingTopbar(handlers: DashboardHandlers): HTMLElement {
  const refreshButton = renderActionButton({
    disabled: true,
    iconPath: REFRESH_ICON,
    label: "Refresh",
    onClick: handlers.onRefresh,
  });

  return createElement("header", { className: "topbar", id: "overview" }, [
    createElement("div", { className: "topbar-copy" }, [
      createElement("h1", { text: "ScaleBridge" }),
      createElement("span", { text: "Loading local status" }),
    ]),
    createElement("div", { className: "topbar-actions" }, [
      renderStatusPill("Loading", "neutral"),
      refreshButton,
    ]),
  ]);
}

function renderAlert(data: DashboardData, state: AppState): HTMLElement {
  if (state.backendAvailable && !data.status.lastError) {
    return createElement("div", { className: "hidden" });
  }

  let message = state.error ?? "Backend is not reachable";
  let title = "Preview mode";

  if (state.backendAvailable) {
    message = data.status.lastError ?? "";
    title = "Watcher error";
  }

  return createElement("section", { className: "alert-panel" }, [
    createElement("strong", { text: title }),
    createElement("span", { text: message }),
  ]);
}

function renderLatestMeasurement(data: DashboardData): HTMLElement {
  const latestRecord = firstMeasurement(data.measurements);
  const latestEvent = data.status.latestMeasurement;
  let impedance = latestRecord?.impedance ?? null;
  let measuredAt = latestRecord?.measured_at ?? null;
  let stable = latestRecord?.stable === true;
  let weight = latestRecord?.weight_kg ?? null;

  if (latestEvent) {
    impedance = latestEvent.measurement.impedance;
    measuredAt = latestEvent.measured_at;
    stable = latestEvent.measurement.status === "stable";
    weight = latestEvent.measurement.weight_kg;
  }

  let measurementMeta = "No data";

  if (measuredAt) {
    measurementMeta = formatElapsed(measuredAt);
  }

  let stableLabel = "Dynamic";
  let stableTone = "warn";

  if (stable) {
    stableLabel = "Stable";
    stableTone = "good";
  }

  return createElement("section", { className: "panel latest-panel" }, [
    renderPanelHeader("Latest measurement", measurementMeta),
    createElement("div", { className: "weight-readout", text: formatWeight(weight) }),
    createElement("div", { className: "metric-strip" }, [
      renderMetric("Stability", stableLabel, stableTone),
      renderMetric("Impedance", formatImpedance(impedance), "neutral"),
      renderMetric("Measured", formatDateTime(measuredAt), "neutral"),
    ]),
  ]);
}

function renderConnectionPanel(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
  const status = data.status.watcherStatus;
  const statusText = formatStatusLabel(status);
  const statusTone = toneForStatus(status);
  let runningText = "Watcher task idle";

  if (data.status.watcherRunning) {
    runningText = "Watcher task active";
  }

  const refreshButton = renderActionButton({
    disabled: state.loading,
    iconPath: REFRESH_ICON,
    label: "Refresh",
    onClick: handlers.onRefresh,
  });

  return createElement("section", { className: "panel connection-panel" }, [
    renderPanelHeader("Connection status", runningText),
    createElement("div", { className: "connection-body" }, [
      renderStatusPill(statusText, statusTone),
      createElement("p", {
        text: data.status.lastError ?? "BLE watcher state is maintained by the Rust backend.",
      }),
    ]),
    createElement("div", { className: "inline-actions" }, [refreshButton]),
  ]);
}

function renderAutostartPanel(data: DashboardData): HTMLElement {
  let autostartLabel = "Off";
  let autostartTone = "neutral";

  if (data.autostart.enabled) {
    autostartLabel = "On";
    autostartTone = "good";
  }

  return createElement("section", { className: "panel autostart-panel", id: "settings" }, [
    renderPanelHeader("Autostart", "macOS login launch"),
    createElement("div", { className: "setting-row" }, [
      createElement("div", { className: "setting-copy" }, [
        createElement("strong", { text: "Login launch" }),
        createElement("span", { text: "Managed by the Tauri backend" }),
      ]),
      renderStatusPill(autostartLabel, autostartTone),
    ]),
  ]);
}

function renderMeasurementsPanel(measurements: MeasurementRecord[]): HTMLElement {
  let body = renderEmptyState("No measurements saved yet");

  if (measurements.length > 0) {
    body = renderMeasurementsTable(measurements);
  }

  return createElement("section", { className: "panel table-panel span-two", id: "measurements" }, [
    renderPanelHeader("Measurement history", formatCount(measurements.length, "record")),
    body,
  ]);
}

function renderMeasurementsTable(measurements: MeasurementRecord[]): HTMLElement {
  return renderTable(
    ["Time", "Weight", "Impedance", "Status", "Raw"],
    measurements.map((measurement) => {
      let rawPacketId = "--";
      let stableLabel = "Dynamic";

      if (measurement.raw_packet_id) {
        rawPacketId = `#${measurement.raw_packet_id}`;
      }

      if (measurement.stable) {
        stableLabel = "Stable";
      }

      return [
        formatDateTime(measurement.measured_at),
        formatWeight(measurement.weight_kg),
        formatImpedance(measurement.impedance),
        stableLabel,
        rawPacketId,
      ];
    }),
  );
}

function renderDevicesPanel(devices: DeviceRecord[]): HTMLElement {
  let list = renderEmptyState("No devices detected yet");

  if (devices.length > 0) {
    list = createElement(
      "div",
      { className: "device-list" },
      devices.map((device) => renderDeviceRow(device)),
    );
  }

  return createElement("section", { className: "panel", id: "devices" }, [
    renderPanelHeader("Detected devices", formatCount(devices.length, "device")),
    list,
  ]);
}

function renderDeviceRow(device: DeviceRecord): HTMLElement {
  const serviceCount = countServices(device.service_uuids_json);

  return createElement("article", { className: "device-row" }, [
    createElement("div", { className: "device-main" }, [
      createElement("strong", { text: formatDeviceName(device.name, device.address) }),
      createElement("span", { text: device.model ?? "Unknown profile" }),
    ]),
    createElement("div", { className: "device-meta" }, [
      createElement("span", { text: formatCount(serviceCount, "service") }),
      createElement("span", { text: formatElapsed(device.last_seen_at) }),
    ]),
  ]);
}

function renderRawPacketsPanel(rawPackets: RawPacketRecord[]): HTMLElement {
  let list = renderEmptyState("No raw packets saved yet");

  if (rawPackets.length > 0) {
    list = createElement(
      "div",
      { className: "log-list" },
      rawPackets.map((packet) => renderRawPacketRow(packet)),
    );
  }

  return createElement("section", { className: "panel log-panel", id: "raw-log" }, [
    renderPanelHeader("Raw log", formatCount(rawPackets.length, "packet")),
    list,
  ]);
}

function renderRawPacketRow(packet: RawPacketRecord): HTMLElement {
  let packetTone = "neutral";

  if (packet.direction === "inbound") {
    packetTone = "good";
  }

  return createElement("article", { className: "log-row" }, [
    createElement("div", { className: "log-row-head" }, [
      renderStatusPill(packet.direction, packetTone),
      createElement("span", { text: formatElapsed(packet.seen_at) }),
      createElement("span", { text: packet.parser ?? "unparsed" }),
    ]),
    createElement("code", { text: truncateMiddle(packet.hex, 54) }),
  ]);
}

function renderEventsPanel(events: AppEventRecord[]): HTMLElement {
  let list = renderEmptyState("No app events saved yet");

  if (events.length > 0) {
    list = createElement(
      "div",
      { className: "event-list" },
      events.map((event) => renderEventRow(event)),
    );
  }

  return createElement("section", { className: "panel" }, [
    renderPanelHeader("App events", formatCount(events.length, "event")),
    list,
  ]);
}

function renderEventRow(event: AppEventRecord): HTMLElement {
  return createElement("article", { className: "event-row" }, [
    renderStatusPill(event.level, toneForEventLevel(event.level)),
    createElement("div", { className: "event-copy" }, [
      createElement("strong", { text: event.message }),
      createElement("span", { text: formatDateTime(event.created_at) }),
    ]),
  ]);
}

function renderPanelHeader(title: string, meta: string): HTMLElement {
  return createElement("div", { className: "panel-header" }, [
    createElement("h2", { text: title }),
    createElement("span", { text: meta }),
  ]);
}

function renderMetric(label: string, value: string, tone: string): HTMLElement {
  return createElement("div", { className: "metric" }, [
    createElement("span", { text: label }),
    createElement("strong", { className: `tone-${tone}`, text: value }),
  ]);
}

function renderStatusPill(label: string, tone: string): HTMLElement {
  return createElement("span", { className: `status-pill tone-${tone}`, text: label });
}

function renderActionButton(options: ActionButtonOptions): HTMLButtonElement {
  const button = createElement("button", {
    className: "action-button",
    disabled: options.disabled,
    type: "button",
  });

  button.append(
    createSvgIcon(options.iconPath, options.label),
    document.createTextNode(options.label),
  );

  button.addEventListener("click", options.onClick);

  return button;
}

function renderTable(headers: string[], rows: string[][]): HTMLElement {
  const table = createElement("table", { className: "data-table" });
  const headRow = createElement(
    "tr",
    {},
    headers.map((header) => createElement("th", { text: header })),
  );
  const bodyRows = rows.map((row) =>
    createElement(
      "tr",
      {},
      row.map((cell) => createElement("td", { text: cell })),
    ),
  );

  table.append(createElement("thead", {}, [headRow]), createElement("tbody", {}, bodyRows));

  return table;
}

function renderEmptyState(message: string): HTMLElement {
  return createElement("div", { className: "empty-state", text: message });
}

function renderSkeletonPanel(title: string): HTMLElement {
  return createElement("section", { className: "panel skeleton-panel" }, [
    renderPanelHeader(title, "Loading"),
    createElement("div", { className: "skeleton-line wide" }),
    createElement("div", { className: "skeleton-line" }),
    createElement("div", { className: "skeleton-line short" }),
  ]);
}

function toneForStatus(status: WatcherStatus): string {
  switch (status) {
    case "connected":
    case "subscribed":
      return "good";
    case "connecting":
    case "scanning":
    case "starting":
    case "stopping":
      return "warn";
    case "idle":
    case "stopped":
      return "neutral";
  }
}

function toneForEventLevel(level: string): string {
  if (level === "error") {
    return "danger";
  }

  if (level === "warn") {
    return "warn";
  }

  return "neutral";
}

function countServices(serviceUuidsJson: string): number {
  try {
    const parsed = JSON.parse(serviceUuidsJson) as unknown;

    if (Array.isArray(parsed)) {
      return parsed.length;
    }

    return 0;
  } catch {
    return 0;
  }
}

function firstMeasurement(measurements: MeasurementRecord[]): MeasurementRecord | null {
  if (measurements.length === 0) {
    return null;
  }

  return measurements[0] ?? null;
}
