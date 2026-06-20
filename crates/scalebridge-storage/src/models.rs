use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub id: i64,
    pub model: Option<String>,
    pub name: Option<String>,
    pub address: Option<String>,
    pub service_uuids_json: String,
    pub first_seen_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceUpsert {
    pub model: Option<String>,
    pub name: Option<String>,
    pub address: Option<String>,
    pub service_uuids_json: String,
    pub seen_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketDirection {
    Inbound,
    Outbound,
}

impl PacketDirection {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "outbound" => Self::Outbound,
            _ => Self::Inbound,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawPacketRecord {
    pub id: i64,
    pub device_id: Option<i64>,
    pub seen_at: OffsetDateTime,
    pub direction: PacketDirection,
    pub characteristic_uuid: Option<String>,
    pub hex: String,
    pub parser: Option<String>,
    pub parsed_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacketInsert {
    pub device_id: Option<i64>,
    pub seen_at: OffsetDateTime,
    pub direction: PacketDirection,
    pub characteristic_uuid: Option<String>,
    pub hex: String,
    pub parser: Option<String>,
    pub parsed_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementRecord {
    pub id: i64,
    pub device_id: Option<i64>,
    pub measured_at: OffsetDateTime,
    pub weight_kg: Option<f64>,
    pub impedance: Option<i64>,
    pub encrypted_impedance: Option<i64>,
    pub stable: bool,
    pub raw_packet_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementInsert {
    pub device_id: Option<i64>,
    pub measured_at: OffsetDateTime,
    pub weight_kg: Option<f64>,
    pub impedance: Option<i64>,
    pub encrypted_impedance: Option<i64>,
    pub stable: bool,
    pub raw_packet_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEventRecord {
    pub id: i64,
    pub created_at: OffsetDateTime,
    pub level: String,
    pub message: String,
    pub context_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppEventInsert {
    pub created_at: OffsetDateTime,
    pub level: String,
    pub message: String,
    pub context_json: Option<String>,
}
