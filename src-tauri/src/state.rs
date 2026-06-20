use std::sync::{Arc, Mutex};

use scalebridge_core::{MeasurementEvent, ScaleWatcherHandle, WatcherStatus};
use scalebridge_storage::Storage;
use serde::Serialize;

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
    pub latest_measurement: Option<MeasurementEvent>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub watcher_status: WatcherStatus,
    pub watcher_running: bool,
    pub latest_measurement: Option<MeasurementEvent>,
    pub last_error: Option<String>,
}

impl BackendState {
    pub fn status_snapshot(&self) -> AppStatus {
        AppStatus {
            watcher_status: self.status,
            watcher_running: self.watcher.is_some(),
            latest_measurement: self.latest_measurement.clone(),
            last_error: self.last_error.clone(),
        }
    }
}
