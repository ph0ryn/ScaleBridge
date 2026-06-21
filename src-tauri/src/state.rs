use std::sync::{Arc, Mutex};

use scalebridge_core::{
    DeviceInfo, Measurement, MeasurementEvent, ScaleWatcherHandle, WatcherStatus,
};
use scalebridge_storage::Storage;
use serde::Serialize;
use time::OffsetDateTime;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<BackendState>>,
}

impl AppState {
    pub fn new(storage: Storage) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BackendState {
                storage,
                watcher: None,
                status: WatcherStatus::Stopped,
                live_measurement: LiveMeasurementStatus::idle(),
                latest_measurement: None,
                last_error: None,
            })),
        }
    }

    pub fn with_lock<T>(
        &self,
        action: impl FnOnce(&mut BackendState) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut state = self
            .inner
            .lock()
            .map_err(|error| format!("backend state lock poisoned: {error}"))?;

        action(&mut state)
    }
}

pub struct BackendState {
    pub storage: Storage,
    pub watcher: Option<ScaleWatcherHandle>,
    pub status: WatcherStatus,
    pub live_measurement: LiveMeasurementStatus,
    pub latest_measurement: Option<MeasurementEvent>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveMeasurementPhase {
    Idle,
    Measuring,
    Stable,
    Overload,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveMeasurementStatus {
    pub phase: LiveMeasurementPhase,
    pub device: Option<DeviceInfo>,
    pub measurement: Option<Measurement>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub measured_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

impl LiveMeasurementStatus {
    #[must_use]
    pub fn idle() -> Self {
        Self {
            phase: LiveMeasurementPhase::Idle,
            device: None,
            measurement: None,
            measured_at: None,
            started_at: None,
            updated_at: None,
        }
    }

    #[must_use]
    pub fn measuring(device: DeviceInfo, now: OffsetDateTime) -> Self {
        Self {
            phase: LiveMeasurementPhase::Measuring,
            device: Some(device),
            measurement: None,
            measured_at: None,
            started_at: Some(now),
            updated_at: Some(now),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub watcher_status: WatcherStatus,
    pub watcher_running: bool,
    pub live_measurement: LiveMeasurementStatus,
    pub latest_measurement: Option<MeasurementEvent>,
    pub last_error: Option<String>,
}

impl BackendState {
    pub fn status_snapshot(&self) -> AppStatus {
        AppStatus {
            watcher_status: self.status,
            watcher_running: self.watcher.is_some(),
            live_measurement: self.live_measurement.clone(),
            latest_measurement: self.latest_measurement.clone(),
            last_error: self.last_error.clone(),
        }
    }
}
