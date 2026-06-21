use scalebridge_core::WatcherStatus;
use scalebridge_storage::{DeviceRecord, MeasurementRecord, RawPacketRecord};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::services;
use crate::state::{AppState, AppStatus, ScanIntervalSettings};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatcherStatusResponse {
    pub status: WatcherStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    pub enabled: bool,
}

#[tauri::command]
pub fn get_current_status(state: State<'_, AppState>) -> Result<AppStatus, String> {
    state.with_lock(|state| Ok(state.status_snapshot()))
}

#[tauri::command]
pub fn list_recent_measurements(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<MeasurementRecord>, String> {
    state.with_lock(|state| {
        state
            .storage
            .list_recent_measurements(limit)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn list_devices(state: State<'_, AppState>) -> Result<Vec<DeviceRecord>, String> {
    state.with_lock(|state| {
        state
            .storage
            .list_devices()
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn list_recent_raw_packets(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<RawPacketRecord>, String> {
    state.with_lock(|state| {
        state
            .storage
            .list_recent_raw_packets(limit)
            .map_err(|error| error.to_string())
    })
}

#[tauri::command]
pub fn start_watcher(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WatcherStatusResponse, String> {
    let status = services::start_watcher(app, state.inner().clone())?;

    Ok(WatcherStatusResponse { status })
}

#[tauri::command]
pub fn stop_watcher(state: State<'_, AppState>) -> Result<WatcherStatusResponse, String> {
    let status = services::stop_watcher(state.inner().clone())?;

    Ok(WatcherStatusResponse { status })
}

#[tauri::command]
pub fn get_autostart_status(app: AppHandle) -> Result<AutostartStatus, String> {
    let enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| error.to_string())?;

    Ok(AutostartStatus { enabled })
}

#[tauri::command]
pub fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<AutostartStatus, String> {
    let autostart = app.autolaunch();

    if enabled {
        autostart.enable().map_err(|error| error.to_string())?;
    } else {
        autostart.disable().map_err(|error| error.to_string())?;
    }

    get_autostart_status(app)
}

#[tauri::command]
pub fn get_scan_interval_settings(
    state: State<'_, AppState>,
) -> Result<ScanIntervalSettings, String> {
    state.with_lock(|state| Ok(state.scan_interval_settings))
}

#[tauri::command]
pub fn set_scan_interval_settings(
    state: State<'_, AppState>,
    settings: ScanIntervalSettings,
) -> Result<ScanIntervalSettings, String> {
    state.with_lock(|state| state.set_scan_interval_settings(settings))
}
