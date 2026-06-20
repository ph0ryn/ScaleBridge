use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{
    AppEventInsert, AppEventRecord, DeviceRecord, DeviceUpsert, MeasurementInsert,
    MeasurementRecord, PacketDirection, RawPacketInsert, RawPacketRecord,
};

const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("time format error: {0}")]
    TimeFormat(#[from] time::error::Format),
    #[error("time parse error: {0}")]
    TimeParse(#[from] time::error::Parse),
}

pub struct Storage {
    connection: Connection,
}

impl Storage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        let storage = Self { connection };
        storage.migrate()?;

        Ok(storage)
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        let storage = Self { connection };
        storage.migrate()?;

        Ok(storage)
    }

    pub fn migrate(&self) -> Result<(), StorageError> {
        self.connection.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS devices (
              id INTEGER PRIMARY KEY,
              model TEXT,
              name TEXT,
              address TEXT,
              service_uuids_json TEXT NOT NULL DEFAULT '[]',
              first_seen_at TEXT NOT NULL,
              last_seen_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS raw_packets (
              id INTEGER PRIMARY KEY,
              device_id INTEGER,
              seen_at TEXT NOT NULL,
              direction TEXT NOT NULL,
              characteristic_uuid TEXT,
              hex TEXT NOT NULL,
              parser TEXT,
              parsed_json TEXT,
              FOREIGN KEY(device_id) REFERENCES devices(id)
            );

            CREATE TABLE IF NOT EXISTS measurements (
              id INTEGER PRIMARY KEY,
              device_id INTEGER,
              measured_at TEXT NOT NULL,
              weight_kg REAL,
              impedance INTEGER,
              encrypted_impedance INTEGER,
              stable INTEGER NOT NULL,
              raw_packet_id INTEGER,
              FOREIGN KEY(device_id) REFERENCES devices(id),
              FOREIGN KEY(raw_packet_id) REFERENCES raw_packets(id)
            );

            CREATE TABLE IF NOT EXISTS app_events (
              id INTEGER PRIMARY KEY,
              created_at TEXT NOT NULL,
              level TEXT NOT NULL,
              message TEXT NOT NULL,
              context_json TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_devices_address ON devices(address);
            CREATE INDEX IF NOT EXISTS idx_raw_packets_seen_at ON raw_packets(seen_at);
            CREATE INDEX IF NOT EXISTS idx_measurements_measured_at ON measurements(measured_at);
            CREATE INDEX IF NOT EXISTS idx_app_events_created_at ON app_events(created_at);
            PRAGMA user_version = 1;
            ",
        )?;

        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64, StorageError> {
        Ok(self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?)
    }

    pub fn upsert_device(&self, device: &DeviceUpsert) -> Result<DeviceRecord, StorageError> {
        let seen_at = format_time(device.seen_at)?;

        if let Some(address) = &device.address {
            let existing_id = self
                .connection
                .query_row(
                    "SELECT id FROM devices WHERE address = ?1",
                    [address],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;

            if let Some(id) = existing_id {
                self.connection.execute(
                    "
                    UPDATE devices
                    SET model = ?1,
                        name = ?2,
                        service_uuids_json = ?3,
                        last_seen_at = ?4
                    WHERE id = ?5
                    ",
                    params![
                        device.model.as_deref(),
                        device.name.as_deref(),
                        device.service_uuids_json.as_str(),
                        seen_at,
                        id
                    ],
                )?;

                return self.get_device(id);
            }
        }

        self.connection.execute(
            "
            INSERT INTO devices (
              model,
              name,
              address,
              service_uuids_json,
              first_seen_at,
              last_seen_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                device.model.as_deref(),
                device.name.as_deref(),
                device.address.as_deref(),
                device.service_uuids_json.as_str(),
                seen_at,
                seen_at
            ],
        )?;

        self.get_device(self.connection.last_insert_rowid())
    }

    pub fn insert_raw_packet(&self, packet: &RawPacketInsert) -> Result<i64, StorageError> {
        let seen_at = format_time(packet.seen_at)?;

        self.connection.execute(
            "
            INSERT INTO raw_packets (
              device_id,
              seen_at,
              direction,
              characteristic_uuid,
              hex,
              parser,
              parsed_json
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                packet.device_id,
                seen_at,
                packet.direction.as_str(),
                packet.characteristic_uuid.as_deref(),
                packet.hex.as_str(),
                packet.parser.as_deref(),
                packet.parsed_json.as_deref()
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn insert_measurement(&self, measurement: &MeasurementInsert) -> Result<i64, StorageError> {
        let measured_at = format_time(measurement.measured_at)?;

        self.connection.execute(
            "
            INSERT INTO measurements (
              device_id,
              measured_at,
              weight_kg,
              impedance,
              encrypted_impedance,
              stable,
              raw_packet_id
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                measurement.device_id,
                measured_at,
                measurement.weight_kg,
                measurement.impedance,
                measurement.encrypted_impedance,
                if measurement.stable { 1_i64 } else { 0_i64 },
                measurement.raw_packet_id
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn insert_app_event(&self, event: &AppEventInsert) -> Result<i64, StorageError> {
        let created_at = format_time(event.created_at)?;

        self.connection.execute(
            "
            INSERT INTO app_events (
              created_at,
              level,
              message,
              context_json
            )
            VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                created_at,
                event.level.as_str(),
                event.message.as_str(),
                event.context_json.as_deref()
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn list_recent_measurements(
        &self,
        limit: u32,
    ) -> Result<Vec<MeasurementRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
              id,
              device_id,
              measured_at,
              weight_kg,
              impedance,
              encrypted_impedance,
              stable,
              raw_packet_id
            FROM measurements
            ORDER BY measured_at DESC, id DESC
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map([i64::from(limit)], read_measurement)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn list_devices(&self) -> Result<Vec<DeviceRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
              id,
              model,
              name,
              address,
              service_uuids_json,
              first_seen_at,
              last_seen_at
            FROM devices
            ORDER BY last_seen_at DESC, id DESC
            ",
        )?;
        let rows = statement.query_map([], read_device)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn list_recent_events(&self, limit: u32) -> Result<Vec<AppEventRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
              id,
              created_at,
              level,
              message,
              context_json
            FROM app_events
            ORDER BY created_at DESC, id DESC
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map([i64::from(limit)], read_app_event)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    pub fn list_recent_raw_packets(
        &self,
        limit: u32,
    ) -> Result<Vec<RawPacketRecord>, StorageError> {
        let mut statement = self.connection.prepare(
            "
            SELECT
              id,
              device_id,
              seen_at,
              direction,
              characteristic_uuid,
              hex,
              parser,
              parsed_json
            FROM raw_packets
            ORDER BY seen_at DESC, id DESC
            LIMIT ?1
            ",
        )?;
        let rows = statement.query_map([i64::from(limit)], read_raw_packet)?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StorageError::from)
    }

    fn get_device(&self, id: i64) -> Result<DeviceRecord, StorageError> {
        Ok(self.connection.query_row(
            "
            SELECT
              id,
              model,
              name,
              address,
              service_uuids_json,
              first_seen_at,
              last_seen_at
            FROM devices
            WHERE id = ?1
            ",
            [id],
            read_device,
        )?)
    }
}

fn read_device(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeviceRecord> {
    Ok(DeviceRecord {
        id: row.get(0)?,
        model: row.get(1)?,
        name: row.get(2)?,
        address: row.get(3)?,
        service_uuids_json: row.get(4)?,
        first_seen_at: parse_time_from_row(row, 5)?,
        last_seen_at: parse_time_from_row(row, 6)?,
    })
}

fn read_raw_packet(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawPacketRecord> {
    let direction: String = row.get(3)?;

    Ok(RawPacketRecord {
        id: row.get(0)?,
        device_id: row.get(1)?,
        seen_at: parse_time_from_row(row, 2)?,
        direction: PacketDirection::from_str(&direction),
        characteristic_uuid: row.get(4)?,
        hex: row.get(5)?,
        parser: row.get(6)?,
        parsed_json: row.get(7)?,
    })
}

fn read_measurement(row: &rusqlite::Row<'_>) -> rusqlite::Result<MeasurementRecord> {
    let stable: i64 = row.get(6)?;

    Ok(MeasurementRecord {
        id: row.get(0)?,
        device_id: row.get(1)?,
        measured_at: parse_time_from_row(row, 2)?,
        weight_kg: row.get(3)?,
        impedance: row.get(4)?,
        encrypted_impedance: row.get(5)?,
        stable: stable != 0,
        raw_packet_id: row.get(7)?,
    })
}

fn read_app_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<AppEventRecord> {
    Ok(AppEventRecord {
        id: row.get(0)?,
        created_at: parse_time_from_row(row, 1)?,
        level: row.get(2)?,
        message: row.get(3)?,
        context_json: row.get(4)?,
    })
}

fn format_time(value: OffsetDateTime) -> Result<String, StorageError> {
    Ok(value.format(&time::format_description::well_known::Rfc3339)?)
}

fn parse_time(value: &str) -> rusqlite::Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn parse_time_from_row(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<OffsetDateTime> {
    let value: String = row.get(index)?;

    parse_time(&value)
}

pub fn schema_version() -> i64 {
    SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_schema_idempotently() {
        let storage = Storage::open_in_memory().unwrap();
        storage.migrate().unwrap();

        assert_eq!(storage.schema_version().unwrap(), schema_version());
    }

    #[test]
    fn upserts_device_by_address() {
        let storage = Storage::open_in_memory().unwrap();
        let first = storage.upsert_device(&sample_device("first")).unwrap();
        let second = storage.upsert_device(&sample_device("second")).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(second.name.as_deref(), Some("second"));
        assert_eq!(storage.list_devices().unwrap().len(), 1);
    }

    #[test]
    fn inserts_raw_packet_and_measurement() {
        let storage = Storage::open_in_memory().unwrap();
        let device = storage.upsert_device(&sample_device("scale")).unwrap();
        let raw_packet_id = storage
            .insert_raw_packet(&RawPacketInsert {
                device_id: Some(device.id),
                seen_at: sample_time(),
                direction: PacketDirection::Inbound,
                characteristic_uuid: Some("0000fff4-0000-1000-8000-00805f9b34fb".to_string()),
                hex: "cfe812b414b3b69f00000f".to_string(),
                parser: Some("t9120_live".to_string()),
                parsed_json: Some(r#"{"kind":"t9120_live"}"#.to_string()),
            })
            .unwrap();
        let measurement_id = storage
            .insert_measurement(&MeasurementInsert {
                device_id: Some(device.id),
                measured_at: sample_time(),
                weight_kg: Some(53.0),
                impedance: Some(4840),
                encrypted_impedance: Some(10_466_995),
                stable: true,
                raw_packet_id: Some(raw_packet_id),
            })
            .unwrap();

        let raw_packets = storage.list_recent_raw_packets(10).unwrap();
        let measurements = storage.list_recent_measurements(10).unwrap();

        assert_eq!(raw_packets[0].id, raw_packet_id);
        assert_eq!(raw_packets[0].direction, PacketDirection::Inbound);
        assert_eq!(measurements[0].id, measurement_id);
        assert_eq!(measurements[0].raw_packet_id, Some(raw_packet_id));
        assert_eq!(measurements[0].weight_kg, Some(53.0));
        assert!(measurements[0].stable);
    }

    #[test]
    fn lists_recent_measurements_newest_first() {
        let storage = Storage::open_in_memory().unwrap();
        storage
            .insert_measurement(&MeasurementInsert {
                device_id: None,
                measured_at: sample_time(),
                weight_kg: Some(52.0),
                impedance: None,
                encrypted_impedance: None,
                stable: false,
                raw_packet_id: None,
            })
            .unwrap();
        storage
            .insert_measurement(&MeasurementInsert {
                device_id: None,
                measured_at: sample_time() + time::Duration::seconds(1),
                weight_kg: Some(53.0),
                impedance: None,
                encrypted_impedance: None,
                stable: true,
                raw_packet_id: None,
            })
            .unwrap();

        let measurements = storage.list_recent_measurements(1).unwrap();

        assert_eq!(measurements.len(), 1);
        assert_eq!(measurements[0].weight_kg, Some(53.0));
    }

    #[test]
    fn inserts_app_event() {
        let storage = Storage::open_in_memory().unwrap();
        let event_id = storage
            .insert_app_event(&AppEventInsert {
                created_at: sample_time(),
                level: "info".to_string(),
                message: "watcher started".to_string(),
                context_json: Some(r#"{"source":"test"}"#.to_string()),
            })
            .unwrap();

        let events = storage.list_recent_events(10).unwrap();

        assert_eq!(events[0].id, event_id);
        assert_eq!(events[0].level, "info");
        assert_eq!(events[0].message, "watcher started");
    }

    fn sample_device(name: &str) -> DeviceUpsert {
        DeviceUpsert {
            model: Some("T9120".to_string()),
            name: Some(name.to_string()),
            address: Some("corebluetooth-id".to_string()),
            service_uuids_json: serde_json::json!(["0000fff0-0000-1000-8000-00805f9b34fb"])
                .to_string(),
            seen_at: sample_time(),
        }
    }

    fn sample_time() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_766_194_280).unwrap()
    }
}
