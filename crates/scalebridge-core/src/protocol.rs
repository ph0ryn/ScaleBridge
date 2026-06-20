use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

pub mod uuids {
    pub const T9120_SERVICE: &str = "0000fff0-0000-1000-8000-00805f9b34fb";
    pub const T9120_NOTIFY: &str = "0000fff4-0000-1000-8000-00805f9b34fb";
    pub const T9120_WRITE: &str = "0000fff1-0000-1000-8000-00805f9b34fb";

    pub const T9140_V1_SERVICE: &str = "0000ffb0-0000-1000-8000-00805f9b34fb";
    pub const T9140_V1_NOTIFY: &str = "0000ffb2-0000-1000-8000-00805f9b34fb";
    pub const T9140_V1_WRITE: &str = "0000ffb1-0000-1000-8000-00805f9b34fb";

    pub const T9140_V2_SERVICE: &str = "4143f6b0-5300-4900-4700-414943415245";
    pub const T9140_V2_NOTIFY: &str = "4143f6b2-5300-4900-4700-414943415245";
    pub const T9140_V2_WRITE: &str = "4143f6b1-5300-4900-4700-414943415245";

    pub const T9140_V3_SERVICE: &str = "4143f7b0-5300-4900-4700-414943415245";
    pub const T9140_V3_NOTIFY: &str = "4143f7b2-5300-4900-4700-414943415245";
    pub const T9140_V3_WRITE: &str = "4143f7b1-5300-4900-4700-414943415245";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolFamily {
    T9120,
    T9140V1,
    T9140V2,
    T9140V3,
    T9148OrT9149,
    T9150OrT9130,
    Fff0Unknown,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceAdvertisement {
    pub name: Option<String>,
    pub service_uuids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub family: ProtocolFamily,
    pub service_uuid: Option<String>,
    pub notify_uuid: Option<String>,
    pub write_uuid: Option<String>,
}

impl DeviceProfile {
    #[must_use]
    pub fn detect(advertisement: &DeviceAdvertisement, discovered_services: &[String]) -> Self {
        let name = advertisement.name.as_deref().unwrap_or_default();
        let advertised = advertisement.service_uuids.iter().map(String::as_str);
        let discovered = discovered_services.iter().map(String::as_str);
        let services: Vec<_> = advertised.chain(discovered).map(normalize_uuid).collect();

        if is_t9120_name(name) {
            return Self::t9120();
        }

        if name == "eufy T9140"
            && services
                .iter()
                .any(|service| service == uuids::T9140_V1_SERVICE)
        {
            return Self::t9140_v1();
        }

        if name == "eufy T9140"
            && services
                .iter()
                .any(|service| service == uuids::T9140_V2_SERVICE)
        {
            return Self::t9140_v2();
        }

        if name == "eufy T9140"
            && services
                .iter()
                .any(|service| service == uuids::T9140_V3_SERVICE)
        {
            return Self::t9140_v3();
        }

        if matches!(name, "eufy T9148" | "eufy T9149") {
            return Self {
                family: ProtocolFamily::T9148OrT9149,
                service_uuid: Some(uuids::T9120_SERVICE.to_string()),
                notify_uuid: Some(uuids::T9120_NOTIFY.to_string()),
                write_uuid: Some(uuids::T9120_WRITE.to_string()),
            };
        }

        if matches!(name, "eufy T9150" | "eufy T9130") {
            return Self {
                family: ProtocolFamily::T9150OrT9130,
                service_uuid: Some(uuids::T9120_SERVICE.to_string()),
                notify_uuid: None,
                write_uuid: None,
            };
        }

        if services
            .iter()
            .any(|service| service == uuids::T9120_SERVICE)
        {
            return Self {
                family: ProtocolFamily::Fff0Unknown,
                service_uuid: Some(uuids::T9120_SERVICE.to_string()),
                notify_uuid: None,
                write_uuid: None,
            };
        }

        Self {
            family: ProtocolFamily::Unknown,
            service_uuid: None,
            notify_uuid: None,
            write_uuid: None,
        }
    }

    #[must_use]
    pub fn t9120() -> Self {
        Self {
            family: ProtocolFamily::T9120,
            service_uuid: Some(uuids::T9120_SERVICE.to_string()),
            notify_uuid: Some(uuids::T9120_NOTIFY.to_string()),
            write_uuid: Some(uuids::T9120_WRITE.to_string()),
        }
    }

    #[must_use]
    pub fn t9140_v1() -> Self {
        Self {
            family: ProtocolFamily::T9140V1,
            service_uuid: Some(uuids::T9140_V1_SERVICE.to_string()),
            notify_uuid: Some(uuids::T9140_V1_NOTIFY.to_string()),
            write_uuid: Some(uuids::T9140_V1_WRITE.to_string()),
        }
    }

    #[must_use]
    pub fn t9140_v2() -> Self {
        Self {
            family: ProtocolFamily::T9140V2,
            service_uuid: Some(uuids::T9140_V2_SERVICE.to_string()),
            notify_uuid: Some(uuids::T9140_V2_NOTIFY.to_string()),
            write_uuid: Some(uuids::T9140_V2_WRITE.to_string()),
        }
    }

    #[must_use]
    pub fn t9140_v3() -> Self {
        Self {
            family: ProtocolFamily::T9140V3,
            service_uuid: Some(uuids::T9140_V3_SERVICE.to_string()),
            notify_uuid: Some(uuids::T9140_V3_NOTIFY.to_string()),
            write_uuid: Some(uuids::T9140_V3_WRITE.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightStatus {
    Stable,
    Dynamic,
    Overload,
}

impl WeightStatus {
    fn from_byte(byte: u8) -> Result<Self, ParseError> {
        match byte {
            0 => Ok(Self::Stable),
            1 => Ok(Self::Dynamic),
            2 => Ok(Self::Overload),
            status => Err(ParseError::InvalidStatus(status)),
        }
    }

    #[must_use]
    pub fn is_stable(self) -> bool {
        matches!(self, Self::Stable)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub weight_raw: u16,
    pub weight_kg: f64,
    pub impedance: u16,
    pub encrypted_impedance: u32,
    pub fat_mode: u8,
    pub status: WeightStatus,
}

impl Measurement {
    #[must_use]
    pub fn stable(&self) -> bool {
        self.status.is_stable()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryTimestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl HistoryTimestamp {
    #[must_use]
    pub fn to_offset_date_time(&self, offset: UtcOffset) -> Option<OffsetDateTime> {
        let month = Month::try_from(self.month).ok()?;
        let date = Date::from_calendar_date(i32::from(self.year), month, self.day).ok()?;
        let time = Time::from_hms(self.hour, self.minute, self.second).ok()?;

        Some(PrimitiveDateTime::new(date, time).assume_offset(offset))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAckKind {
    TimeSync,
    HistorySync,
    HistoryDelete,
    Disconnect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParsedPacket {
    T9120Live {
        measurement: Measurement,
        checksum: u8,
    },
    T9120HistoryCandidate {
        measurement: Measurement,
        timestamp: HistoryTimestamp,
        checksum: u8,
    },
    ControlAck {
        ack: ControlAckKind,
    },
    Unknown {
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("empty notification")]
    Empty,
    #[error("invalid T9120 packet length {actual}; expected {expected:?}")]
    InvalidLength { actual: usize, expected: Vec<usize> },
    #[error("invalid checksum {actual:#04x}; expected {expected:#04x}")]
    InvalidChecksum { actual: u8, expected: u8 },
    #[error("invalid weight status {0}")]
    InvalidStatus(u8),
}

pub struct PacketParser;

impl PacketParser {
    pub fn parse_notification(bytes: &[u8]) -> Result<ParsedPacket, ParseError> {
        match bytes {
            [] => Err(ParseError::Empty),
            [0xf1, 0x00] => Ok(ParsedPacket::ControlAck {
                ack: ControlAckKind::TimeSync,
            }),
            [0xf2, 0x00] => Ok(ParsedPacket::ControlAck {
                ack: ControlAckKind::HistorySync,
            }),
            [0xf2, 0x01] => Ok(ParsedPacket::ControlAck {
                ack: ControlAckKind::HistoryDelete,
            }),
            [0xf3, 0x00] => Ok(ParsedPacket::ControlAck {
                ack: ControlAckKind::Disconnect,
            }),
            [0xcf, ..] => parse_t9120_cf_packet(bytes),
            _ => Ok(ParsedPacket::Unknown {
                bytes: bytes.to_vec(),
            }),
        }
    }
}

#[must_use]
pub fn build_unit_command(unit: u8) -> [u8; 11] {
    let mut command = [0_u8; 11];
    command[0] = 0xfd;
    command[1] = unit;
    command[10] = xor_checksum(&command[..10]);
    command
}

#[must_use]
pub fn build_time_sync_command(now: OffsetDateTime) -> [u8; 8] {
    let year = now.year() as u16;
    let [year_hi, year_lo] = year.to_be_bytes();

    [
        0xf1,
        year_hi,
        year_lo,
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    ]
}

#[must_use]
pub fn xor_checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0, |checksum, byte| checksum ^ byte)
}

fn parse_t9120_cf_packet(bytes: &[u8]) -> Result<ParsedPacket, ParseError> {
    match bytes.len() {
        11 => {
            let measurement = parse_t9120_measurement(bytes)?;
            let checksum = bytes[10];

            Ok(ParsedPacket::T9120Live {
                measurement,
                checksum,
            })
        }
        18 => {
            let measurement = parse_t9120_measurement(bytes)?;
            let timestamp = HistoryTimestamp {
                year: u16::from_be_bytes([bytes[11], bytes[12]]),
                month: bytes[13],
                day: bytes[14],
                hour: bytes[15],
                minute: bytes[16],
                second: bytes[17],
            };

            Ok(ParsedPacket::T9120HistoryCandidate {
                measurement,
                timestamp,
                checksum: bytes[10],
            })
        }
        actual => Err(ParseError::InvalidLength {
            actual,
            expected: vec![11, 18],
        }),
    }
}

fn parse_t9120_measurement(bytes: &[u8]) -> Result<Measurement, ParseError> {
    let expected = xor_checksum(&bytes[..10]);
    let actual = bytes[10];

    if actual != expected {
        return Err(ParseError::InvalidChecksum { actual, expected });
    }

    let impedance = u16::from_le_bytes([bytes[1], bytes[2]]);
    let weight_raw = u16::from_le_bytes([bytes[3], bytes[4]]);
    let encrypted_impedance =
        u32::from(bytes[5]) | (u32::from(bytes[6]) << 8) | (u32::from(bytes[7]) << 16);
    let fat_mode = bytes[8] >> 4;
    let status = WeightStatus::from_byte(bytes[9])?;

    Ok(Measurement {
        weight_raw,
        weight_kg: f64::from(weight_raw) / 100.0,
        impedance,
        encrypted_impedance,
        fat_mode,
        status,
    })
}

fn is_t9120_name(name: &str) -> bool {
    matches!(
        name,
        "eufy T9120" | "eufy T9146" | "eufy T9146 C1" | "eufy T9147"
    )
}

fn normalize_uuid(uuid: &str) -> String {
    uuid.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const STABLE_SAMPLE: [u8; 11] = [
        0xcf, 0xe8, 0x12, 0xb4, 0x14, 0xb3, 0xb6, 0x9f, 0x00, 0x00, 0x0f,
    ];

    #[test]
    fn parses_confirmed_t9120_stable_packet() {
        let parsed = PacketParser::parse_notification(&STABLE_SAMPLE).unwrap();

        let ParsedPacket::T9120Live {
            measurement,
            checksum,
        } = parsed
        else {
            panic!("expected live packet");
        };

        assert_eq!(checksum, 0x0f);
        assert_eq!(measurement.status, WeightStatus::Stable);
        assert!(measurement.stable());
        assert_eq!(measurement.weight_raw, 5300);
        assert_eq!(measurement.weight_kg, 53.0);
        assert_eq!(measurement.impedance, 4840);
        assert_eq!(measurement.encrypted_impedance, 10_466_995);
        assert_eq!(measurement.fat_mode, 0);
    }

    #[test]
    fn rejects_checksum_mismatch() {
        let mut packet = STABLE_SAMPLE;
        packet[10] = 0x00;

        assert_eq!(
            PacketParser::parse_notification(&packet),
            Err(ParseError::InvalidChecksum {
                actual: 0x00,
                expected: 0x0f,
            }),
        );
    }

    #[test]
    fn rejects_invalid_t9120_length() {
        assert_eq!(
            PacketParser::parse_notification(&[0xcf, 0x00]),
            Err(ParseError::InvalidLength {
                actual: 2,
                expected: vec![11, 18],
            }),
        );
    }

    #[test]
    fn parses_dynamic_and_overload_status() {
        let mut dynamic = STABLE_SAMPLE;
        dynamic[9] = 1;
        dynamic[10] = xor_checksum(&dynamic[..10]);

        let ParsedPacket::T9120Live { measurement, .. } =
            PacketParser::parse_notification(&dynamic).unwrap()
        else {
            panic!("expected dynamic packet");
        };

        assert_eq!(measurement.status, WeightStatus::Dynamic);
        assert!(!measurement.stable());

        let mut overload = STABLE_SAMPLE;
        overload[9] = 2;
        overload[10] = xor_checksum(&overload[..10]);

        let ParsedPacket::T9120Live { measurement, .. } =
            PacketParser::parse_notification(&overload).unwrap()
        else {
            panic!("expected overload packet");
        };

        assert_eq!(measurement.status, WeightStatus::Overload);
    }

    #[test]
    fn parses_history_candidate_separately() {
        let mut packet = [0_u8; 18];
        packet[..11].copy_from_slice(&STABLE_SAMPLE);
        packet[11..13].copy_from_slice(&2026_u16.to_be_bytes());
        packet[13] = 6;
        packet[14] = 20;
        packet[15] = 23;
        packet[16] = 51;
        packet[17] = 20;

        let parsed = PacketParser::parse_notification(&packet).unwrap();

        let ParsedPacket::T9120HistoryCandidate {
            measurement,
            timestamp,
            checksum,
        } = parsed
        else {
            panic!("expected history candidate");
        };

        assert_eq!(measurement.weight_raw, 5300);
        assert_eq!(checksum, 0x0f);
        assert_eq!(
            timestamp,
            HistoryTimestamp {
                year: 2026,
                month: 6,
                day: 20,
                hour: 23,
                minute: 51,
                second: 20,
            },
        );
    }

    #[test]
    fn converts_history_timestamp_to_offset_date_time() {
        let timestamp = HistoryTimestamp {
            year: 2026,
            month: 6,
            day: 21,
            hour: 5,
            minute: 4,
            second: 48,
        };
        let measured_at = timestamp
            .to_offset_date_time(UtcOffset::from_hms(9, 0, 0).unwrap())
            .unwrap();

        assert_eq!(measured_at.year(), 2026);
        assert_eq!(measured_at.month() as u8, 6);
        assert_eq!(measured_at.day(), 21);
        assert_eq!(measured_at.hour(), 5);
        assert_eq!(measured_at.minute(), 4);
        assert_eq!(measured_at.second(), 48);
        assert_eq!(measured_at.offset(), UtcOffset::from_hms(9, 0, 0).unwrap());
    }

    #[test]
    fn does_not_confirm_t9120_from_fff0_service_only() {
        let profile = DeviceProfile::detect(
            &DeviceAdvertisement {
                name: None,
                service_uuids: vec![uuids::T9120_SERVICE.to_string()],
            },
            &[],
        );

        assert_eq!(profile.family, ProtocolFamily::Fff0Unknown);
    }

    #[test]
    fn confirms_t9120_from_known_name() {
        let profile = DeviceProfile::detect(
            &DeviceAdvertisement {
                name: Some("eufy T9120".to_string()),
                service_uuids: vec![uuids::T9120_SERVICE.to_string()],
            },
            &[],
        );

        assert_eq!(profile, DeviceProfile::t9120());
    }

    #[test]
    fn builds_unit_init_command_with_xor_checksum() {
        assert_eq!(
            build_unit_command(0),
            [0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xfd],
        );

        let command = build_unit_command(1);

        assert_eq!(command[0], 0xfd);
        assert_eq!(command[1], 1);
        assert_eq!(command[10], xor_checksum(&command[..10]));
    }

    #[test]
    fn builds_time_sync_command() {
        let now = OffsetDateTime::from_unix_timestamp(1_766_194_280).unwrap();
        let command = build_time_sync_command(now);

        assert_eq!(command[0], 0xf1);
        assert_eq!(
            u16::from_be_bytes([command[1], command[2]]),
            now.year() as u16
        );
        assert_eq!(command[3], now.month() as u8);
        assert_eq!(command[4], now.day());
        assert_eq!(command[5], now.hour());
        assert_eq!(command[6], now.minute());
        assert_eq!(command[7], now.second());
    }
}
