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
import { createElement, type Child } from "./dom";

import type { AppState, DashboardView } from "../app/state";
import type {
  DashboardData,
  DeviceRecord,
  MeasurementRecord,
  RawPacketRecord,
  ScanIntervalSettings,
  WatcherStatus,
} from "../lib/types";

const SCAN_INTERVAL_MIN_SECONDS = 1;
const SCAN_INTERVAL_MAX_SECONDS = 3600;

const VIEW_ITEMS: { id: DashboardView; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "history", label: "History" },
  { id: "devices", label: "Devices" },
  { id: "raw-log", label: "Raw log" },
  { id: "settings", label: "Settings" },
];

export interface DashboardHandlers {
  onSelectView: (view: DashboardView) => void;
  onSetAutostartEnabled: (enabled: boolean) => void;
  onSetScanIntervalSettings: (settings: ScanIntervalSettings) => void;
}

export function renderDashboard(state: AppState, handlers: DashboardHandlers): HTMLElement {
  let content = renderLoadingDashboard(state, handlers);

  if (state.data) {
    content = renderLoadedDashboard(state.data, state, handlers);
  }

  return createElement("main", { className: "app-shell" }, [
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
    renderActiveView(data, state, handlers),
    renderLiveMeasurementPopover(data),
  ];
}

function renderLoadingDashboard(state: AppState, handlers: DashboardHandlers): Child[] {
  return [renderLoadingTopbar(state, handlers), renderLoadingActiveView(state.activeView)];
}

function renderBrand(): HTMLElement {
  return createElement("div", { className: "brand" }, [
    createElement("span", { className: "brand-mark", text: "SB" }),
    createElement("div", { className: "brand-copy" }, [
      createElement("strong", { text: "ScaleBridge" }),
      createElement("span", { text: "Local BLE scale monitor" }),
    ]),
  ]);
}

function renderActiveView(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
  switch (state.activeView) {
    case "overview":
      return createElement("section", { className: "view-surface overview-grid" }, [
        renderLatestMeasurement(data),
      ]);
    case "history":
      return createElement("section", { className: "view-surface single-panel-view" }, [
        renderMeasurementsPanel(data.measurements),
      ]);
    case "devices":
      return createElement("section", { className: "view-surface single-panel-view" }, [
        renderDevicesPanel(data.devices),
      ]);
    case "raw-log":
      return createElement("section", { className: "view-surface single-panel-view" }, [
        renderRawPacketsPanel(data.rawPackets),
      ]);
    case "settings":
      return createElement("section", { className: "view-surface settings-grid" }, [
        renderAutostartPanel(data, state, handlers),
        renderScanIntervalPanel(data, state, handlers),
      ]);
  }
}

function renderLoadingActiveView(activeView: DashboardView): HTMLElement {
  switch (activeView) {
    case "overview":
      return createElement("section", { className: "view-surface overview-grid" }, [
        renderSkeletonPanel("Latest measurement"),
      ]);
    case "history":
      return createElement("section", { className: "view-surface single-panel-view" }, [
        renderSkeletonPanel("Measurement history"),
      ]);
    case "devices":
      return createElement("section", { className: "view-surface single-panel-view" }, [
        renderSkeletonPanel("Detected devices"),
      ]);
    case "raw-log":
      return createElement("section", { className: "view-surface single-panel-view" }, [
        renderSkeletonPanel("Raw log"),
      ]);
    case "settings":
      return createElement("section", { className: "view-surface settings-grid" }, [
        renderSkeletonPanel("Autostart"),
        renderSkeletonPanel("Scan interval"),
      ]);
  }
}

function renderViewTabs(activeView: DashboardView, handlers: DashboardHandlers): HTMLElement {
  return createElement(
    "div",
    { ariaLabel: "Dashboard views", className: "view-tabs", role: "tablist" },
    VIEW_ITEMS.map((item) => renderViewTab(item, activeView, handlers)),
  );
}

function renderViewTab(
  item: { id: DashboardView; label: string },
  activeView: DashboardView,
  handlers: DashboardHandlers,
): HTMLButtonElement {
  const selected = item.id === activeView;
  let className = "view-tab";

  if (selected) {
    className = "view-tab selected";
  }

  const button = createElement("button", {
    className,
    text: item.label,
    type: "button",
  });

  button.setAttribute("aria-selected", String(selected));
  button.setAttribute("role", "tab");

  button.addEventListener("click", () => {
    handlers.onSelectView(item.id);
  });

  return button;
}

function renderTopbarTitle(statusLabel: string, statusTone: string): HTMLElement {
  return createElement("div", { className: "topbar-title" }, [
    renderBrand(),
    createElement("div", { className: "topbar-status" }, [
      renderStatusPill(statusLabel, statusTone),
    ]),
  ]);
}

function renderTopbar(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
  return createElement("header", { className: "topbar" }, [
    renderTopbarTitle(
      formatStatusLabel(data.status.watcherStatus),
      toneForStatus(data.status.watcherStatus),
    ),
    createElement("div", { className: "topbar-right" }, [
      renderViewTabs(state.activeView, handlers),
    ]),
  ]);
}

function renderLoadingTopbar(state: AppState, handlers: DashboardHandlers): HTMLElement {
  return createElement("header", { className: "topbar" }, [
    renderTopbarTitle("Loading", "neutral"),
    createElement("div", { className: "topbar-right" }, [
      renderViewTabs(state.activeView, handlers),
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
  let weight = latestRecord?.weight_kg ?? null;

  if (!latestRecord && latestEvent?.measurement.status === "stable") {
    impedance = latestEvent.measurement.impedance;
    measuredAt = latestEvent.measured_at;
    weight = latestEvent.measurement.weight_kg;
  }

  let measurementMeta = "No stable result";

  if (measuredAt) {
    measurementMeta = formatElapsed(measuredAt);
  }

  return createElement("section", { className: "panel latest-panel" }, [
    renderPanelHeader("Latest measurement", measurementMeta),
    createElement("div", { className: "weight-readout", text: formatWeight(weight) }),
    createElement("div", { className: "metric-strip" }, [
      renderMetric("Impedance", formatImpedance(impedance), "neutral"),
      renderMetric("Measured", formatDateTime(measuredAt), "neutral"),
    ]),
  ]);
}

function renderLiveMeasurementPopover(data: DashboardData): HTMLElement {
  const liveMeasurement = data.status.liveMeasurement;

  if (liveMeasurement.phase !== "measuring") {
    return createElement("div", { className: "hidden" });
  }

  let deviceName = "Scale";

  if (liveMeasurement.device) {
    deviceName = formatDeviceName(liveMeasurement.device.name, liveMeasurement.device.address);
  }

  const measuredAt = liveMeasurement.measuredAt ?? liveMeasurement.updatedAt;

  return createElement(
    "aside",
    { ariaLabel: "Live measurement", className: "live-measurement-overlay" },
    [
      createElement("div", { className: "live-measurement-popover" }, [
        createElement("div", { className: "live-measurement-head" }, [
          createElement("strong", { text: "Measuring now" }),
          createElement("span", { text: deviceName }),
        ]),
        createElement("div", {
          className: "live-measurement-readout",
          text: formatWeight(liveMeasurement.measurement?.weight_kg),
        }),
        createElement("div", { className: "live-measurement-details" }, [
          renderMetric(
            "Impedance",
            formatImpedance(liveMeasurement.measurement?.impedance),
            "neutral",
          ),
          renderMetric("Updated", formatDateTime(measuredAt), "neutral"),
        ]),
      ]),
    ],
  );
}

function renderAutostartPanel(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
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
      renderAutostartSwitch({
        disabled: state.saving || !state.backendAvailable,
        enabled: data.autostart.enabled,
        label: autostartLabel,
        onToggle: handlers.onSetAutostartEnabled,
        tone: autostartTone,
      }),
    ]),
  ]);
}

function renderScanIntervalPanel(
  data: DashboardData,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
  return createElement("section", { className: "panel settings-panel scan-interval-panel" }, [
    renderPanelHeader("Scan interval", "BLE rescan cadence"),
    renderScanIntervalForm(data.scanIntervals, state, handlers),
  ]);
}

function renderScanIntervalForm(
  settings: ScanIntervalSettings,
  state: AppState,
  handlers: DashboardHandlers,
): HTMLElement {
  const disabled = state.saving || !state.backendAvailable;
  const windowOpenInput = renderScanIntervalInput(settings.windowOpenSeconds, disabled);
  const backgroundInput = renderScanIntervalInput(settings.backgroundSeconds, disabled);
  const form = createElement("div", { className: "scan-interval-form" }, [
    renderScanIntervalControl(
      "Window open",
      "Scan while Settings or dashboard is visible",
      windowOpenInput,
    ),
    renderScanIntervalControl(
      "Background",
      "Scan while only the tray app is running",
      backgroundInput,
    ),
  ]);
  const apply = (): void => {
    const nextSettings = readScanIntervalSettings(windowOpenInput, backgroundInput);

    if (nextSettings) {
      handlers.onSetScanIntervalSettings(nextSettings);
    }
  };

  windowOpenInput.addEventListener("change", apply);
  backgroundInput.addEventListener("change", apply);

  return form;
}

function renderScanIntervalControl(
  title: string,
  description: string,
  input: HTMLInputElement,
): HTMLLabelElement {
  return createElement("label", { className: "scan-interval-control" }, [
    createElement("span", { className: "setting-copy" }, [
      createElement("strong", { text: title }),
      createElement("span", { text: description }),
    ]),
    createElement("span", { className: "scan-interval-input-shell" }, [
      input,
      createElement("span", { text: "sec" }),
    ]),
  ]);
}

function renderScanIntervalInput(value: number, disabled: boolean): HTMLInputElement {
  const input = createElement("input", { className: "scan-interval-input" });

  input.disabled = disabled;
  input.inputMode = "numeric";
  input.max = String(SCAN_INTERVAL_MAX_SECONDS);
  input.min = String(SCAN_INTERVAL_MIN_SECONDS);
  input.step = "1";
  input.type = "number";
  input.value = String(value);

  input.addEventListener("input", () => {
    input.setCustomValidity("");
  });

  return input;
}

function readScanIntervalSettings(
  windowOpenInput: HTMLInputElement,
  backgroundInput: HTMLInputElement,
): ScanIntervalSettings | null {
  const windowOpenSeconds = readScanIntervalValue(windowOpenInput);
  const backgroundSeconds = readScanIntervalValue(backgroundInput);

  if (windowOpenSeconds === null || backgroundSeconds === null) {
    return null;
  }

  return {
    backgroundSeconds,
    windowOpenSeconds,
  };
}

function readScanIntervalValue(input: HTMLInputElement): number | null {
  const value = Number(input.value);

  if (
    !Number.isInteger(value) ||
    value < SCAN_INTERVAL_MIN_SECONDS ||
    value > SCAN_INTERVAL_MAX_SECONDS
  ) {
    input.setCustomValidity(
      `Enter ${SCAN_INTERVAL_MIN_SECONDS}-${SCAN_INTERVAL_MAX_SECONDS} seconds`,
    );

    input.reportValidity();

    return null;
  }

  input.setCustomValidity("");

  return value;
}

function renderMeasurementsPanel(measurements: MeasurementRecord[]): HTMLElement {
  let body = renderEmptyState("No measurements saved yet");

  if (measurements.length > 0) {
    body = renderMeasurementsTable(measurements);
  }

  return createElement("section", { className: "panel table-panel", id: "measurements" }, [
    renderPanelHeader("Measurement history", formatCount(measurements.length, "record")),
    body,
  ]);
}

function renderMeasurementsTable(measurements: MeasurementRecord[]): HTMLElement {
  return renderTable(
    ["Time", "Weight", "Impedance", "Raw"],
    measurements.map((measurement) => {
      let rawPacketId = "--";

      if (measurement.raw_packet_id) {
        rawPacketId = `#${measurement.raw_packet_id}`;
      }

      return [
        formatDateTime(measurement.measured_at),
        formatWeight(measurement.weight_kg),
        formatImpedance(measurement.impedance),
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

function renderAutostartSwitch(options: {
  disabled: boolean;
  enabled: boolean;
  label: string;
  onToggle: (enabled: boolean) => void;
  tone: string;
}): HTMLButtonElement {
  const button = createElement("button", {
    ariaLabel: "Toggle login launch",
    className: `switch-control tone-${options.tone}`,
    disabled: options.disabled,
    type: "button",
  });

  button.setAttribute("aria-checked", String(options.enabled));
  button.setAttribute("role", "switch");

  button.append(
    createElement("span", { className: "switch-track" }, [
      createElement("span", { className: "switch-knob" }),
    ]),
    createElement("span", { className: "switch-label", text: options.label }),
  );

  button.addEventListener("click", () => {
    options.onToggle(!options.enabled);
  });

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
    case "watching":
      return "good";
    case "connecting":
    case "starting":
    case "stopping":
      return "warn";
    case "stopped":
      return "neutral";
  }
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
