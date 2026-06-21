use std::sync::{Arc, Mutex};
use std::time::Duration;

use scalebridge_core::{
    DeviceInfo, Measurement, MeasurementEvent, ScaleWatcherHandle, ScanCadence, WatcherStatus,
};
use scalebridge_storage::{Storage, StorageError};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

const SCAN_WINDOW_OPEN_SECONDS_KEY: &str = "scan.window_open_seconds";
const SCAN_BACKGROUND_SECONDS_KEY: &str = "scan.background_seconds";
const SCAN_WINDOW_OPEN_CONTINUOUS_KEY: &str = "scan.window_open_continuous_scan";
const DEFAULT_WINDOW_OPEN_SCAN_SECONDS: u64 = 2;
const DEFAULT_BACKGROUND_SCAN_SECONDS: u64 = 10;
const DEFAULT_WINDOW_OPEN_CONTINUOUS_SCAN: bool = true;
pub const MIN_SCAN_INTERVAL_SECONDS: u64 = 1;
pub const MAX_SCAN_INTERVAL_SECONDS: u64 = 3600;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Mutex<BackendState>>,
}

impl AppState {
    pub fn new(storage: Storage) -> Result<Self, StorageError> {
        let scan_interval_settings = ScanIntervalSettings::load(&storage)?;

        Ok(Self {
            inner: Arc::new(Mutex::new(BackendState {
                storage,
                watcher: None,
                status: WatcherStatus::Stopped,
                live_measurement: LiveMeasurementStatus::idle(),
                latest_measurement: None,
                last_error: None,
                scan_interval_settings,
            })),
        })
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
    pub scan_interval_settings: ScanIntervalSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIntervalSettings {
    pub window_open_seconds: u64,
    pub background_seconds: u64,
    pub window_open_continuous_scan: bool,
}

impl Default for ScanIntervalSettings {
    fn default() -> Self {
        Self {
            window_open_seconds: DEFAULT_WINDOW_OPEN_SCAN_SECONDS,
            background_seconds: DEFAULT_BACKGROUND_SCAN_SECONDS,
            window_open_continuous_scan: DEFAULT_WINDOW_OPEN_CONTINUOUS_SCAN,
        }
    }
}

impl ScanIntervalSettings {
    #[must_use]
    pub fn cadence_for_window_open(self, window_open: bool) -> ScanCadence {
        if window_open && self.window_open_continuous_scan {
            return ScanCadence::Continuous {
                rescan_delay: Duration::from_secs(self.background_seconds),
            };
        }

        if window_open {
            return ScanCadence::Timed {
                rescan_delay: Duration::from_secs(self.window_open_seconds),
            };
        }

        ScanCadence::Timed {
            rescan_delay: Duration::from_secs(self.background_seconds),
        }
    }

    pub fn validate(self) -> Result<Self, String> {
        validate_interval_seconds("window open scan interval", self.window_open_seconds)?;
        validate_interval_seconds("background scan interval", self.background_seconds)?;

        Ok(self)
    }

    fn load(storage: &Storage) -> Result<Self, StorageError> {
        Ok(Self {
            window_open_seconds: load_interval_seconds(
                storage,
                SCAN_WINDOW_OPEN_SECONDS_KEY,
                DEFAULT_WINDOW_OPEN_SCAN_SECONDS,
            )?,
            background_seconds: load_interval_seconds(
                storage,
                SCAN_BACKGROUND_SECONDS_KEY,
                DEFAULT_BACKGROUND_SCAN_SECONDS,
            )?,
            window_open_continuous_scan: load_bool_setting(
                storage,
                SCAN_WINDOW_OPEN_CONTINUOUS_KEY,
                DEFAULT_WINDOW_OPEN_CONTINUOUS_SCAN,
            )?,
        })
    }

    fn save(self, storage: &Storage) -> Result<(), StorageError> {
        storage.set_setting_i64(
            SCAN_WINDOW_OPEN_SECONDS_KEY,
            i64::try_from(self.window_open_seconds).expect("scan interval fits i64"),
        )?;
        storage.set_setting_i64(
            SCAN_BACKGROUND_SECONDS_KEY,
            i64::try_from(self.background_seconds).expect("scan interval fits i64"),
        )?;
        storage.set_setting_i64(
            SCAN_WINDOW_OPEN_CONTINUOUS_KEY,
            if self.window_open_continuous_scan {
                1
            } else {
                0
            },
        )
    }
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

    pub fn set_scan_interval_settings(
        &mut self,
        settings: ScanIntervalSettings,
    ) -> Result<ScanIntervalSettings, String> {
        let settings = settings.validate()?;

        settings
            .save(&self.storage)
            .map_err(|error| error.to_string())?;
        self.scan_interval_settings = settings;

        Ok(settings)
    }

    #[must_use]
    pub fn scan_cadence_for_window_open(&self, window_open: bool) -> ScanCadence {
        self.scan_interval_settings
            .cadence_for_window_open(window_open)
    }
}

fn load_interval_seconds(
    storage: &Storage,
    key: &str,
    default_value: u64,
) -> Result<u64, StorageError> {
    let Some(value) = storage.get_setting_i64(key)? else {
        return Ok(default_value);
    };
    let Ok(seconds) = u64::try_from(value) else {
        return Ok(default_value);
    };

    if valid_interval_seconds(seconds) {
        return Ok(seconds);
    }

    Ok(default_value)
}

fn validate_interval_seconds(label: &str, value: u64) -> Result<(), String> {
    if valid_interval_seconds(value) {
        return Ok(());
    }

    Err(format!(
        "{label} must be between {MIN_SCAN_INTERVAL_SECONDS} and {MAX_SCAN_INTERVAL_SECONDS} seconds"
    ))
}

fn load_bool_setting(
    storage: &Storage,
    key: &str,
    default_value: bool,
) -> Result<bool, StorageError> {
    let Some(value) = storage.get_setting_i64(key)? else {
        return Ok(default_value);
    };

    Ok(value != 0)
}

fn valid_interval_seconds(value: u64) -> bool {
    (MIN_SCAN_INTERVAL_SECONDS..=MAX_SCAN_INTERVAL_SECONDS).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_default_scan_interval_settings() {
        let storage = Storage::open_in_memory().unwrap();
        let state = AppState::new(storage).unwrap();

        let settings = state
            .with_lock(|state| Ok(state.scan_interval_settings))
            .unwrap();

        assert_eq!(settings.window_open_seconds, 2);
        assert_eq!(settings.background_seconds, 10);
        assert!(settings.window_open_continuous_scan);
    }

    #[test]
    fn validates_scan_interval_settings() {
        assert!(
            ScanIntervalSettings {
                window_open_seconds: 2,
                background_seconds: 10,
                window_open_continuous_scan: true,
            }
            .validate()
            .is_ok()
        );
        assert!(
            ScanIntervalSettings {
                window_open_seconds: 0,
                background_seconds: 10,
                window_open_continuous_scan: true,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn saves_continuous_scan_setting() {
        let storage = Storage::open_in_memory().unwrap();
        let state = AppState::new(storage).unwrap();
        let settings = ScanIntervalSettings {
            window_open_seconds: 4,
            background_seconds: 12,
            window_open_continuous_scan: false,
        };

        state
            .with_lock(|state| state.set_scan_interval_settings(settings))
            .unwrap();
        let stored = state
            .with_lock(|state| {
                ScanIntervalSettings::load(&state.storage).map_err(|error| error.to_string())
            })
            .unwrap();

        assert_eq!(stored, settings);
    }

    #[test]
    fn returns_continuous_cadence_for_open_window_when_enabled() {
        let storage = Storage::open_in_memory().unwrap();
        let state = AppState::new(storage).unwrap();

        let open_window_cadence = state
            .with_lock(|state| Ok(state.scan_cadence_for_window_open(true)))
            .unwrap();
        let background_cadence = state
            .with_lock(|state| Ok(state.scan_cadence_for_window_open(false)))
            .unwrap();

        assert_eq!(
            open_window_cadence,
            ScanCadence::Continuous {
                rescan_delay: Duration::from_secs(10),
            }
        );
        assert_eq!(
            background_cadence,
            ScanCadence::Timed {
                rescan_delay: Duration::from_secs(10),
            }
        );
    }

    #[test]
    fn returns_timed_window_cadence_when_continuous_is_disabled() {
        let storage = Storage::open_in_memory().unwrap();
        let state = AppState::new(storage).unwrap();

        state
            .with_lock(|state| {
                state.set_scan_interval_settings(ScanIntervalSettings {
                    window_open_seconds: 3,
                    background_seconds: 10,
                    window_open_continuous_scan: false,
                })?;

                Ok(())
            })
            .unwrap();

        let cadence = state
            .with_lock(|state| Ok(state.scan_cadence_for_window_open(true)))
            .unwrap();

        assert_eq!(
            cadence,
            ScanCadence::Timed {
                rescan_delay: Duration::from_secs(3),
            }
        );
    }
}
