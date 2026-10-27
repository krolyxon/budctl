//! Bluetooth addresses and UUIDs.

use std::fmt;
use std::str::FromStr;

use serde::{Serialize, Serializer};

use crate::error::{Error, Result};

/// A Bluetooth device address, kept in the order people write it (`AA:BB:CC:DD:EE:FF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BdAddr([u8; 6]);

impl BdAddr {
    /// The address as the kernel's `bdaddr_t` stores it: reversed.
    /// Spec: transport.md#sockets
    pub(crate) fn kernel_order(self) -> [u8; 6] {
        let mut bytes = self.0;
        bytes.reverse();
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrParseError(String);

impl fmt::Display for AddrParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} is not a Bluetooth address like AA:BB:CC:DD:EE:FF", self.0)
    }
}

impl std::error::Error for AddrParseError {}

impl FromStr for BdAddr {
    type Err = AddrParseError;

    fn from_str(text: &str) -> std::result::Result<Self, Self::Err> {
        let error = || AddrParseError(text.to_owned());
        let mut bytes = [0u8; 6];
        let mut parts = text.split(':');
        for byte in &mut bytes {
            let part = parts.next().ok_or_else(error)?;
            if part.len() != 2 {
                return Err(error());
            }
            *byte = u8::from_str_radix(part, 16).map_err(|_| error())?;
        }
        if parts.next().is_some() {
            return Err(error());
        }
        Ok(BdAddr(bytes))
    }
}

impl fmt::Display for BdAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02X}:{b:02X}:{c:02X}:{d:02X}:{e:02X}:{g:02X}")
    }
}

impl Serialize for BdAddr {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// A 128-bit Bluetooth UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uuid(u128);

/// `00000000-0000-1000-8000-00805F9B34FB`; short UUIDs are offsets into it.
/// Spec: transport.md#sdp-data-elements
const BASE_UUID: u128 = 0x0000_0000_0000_1000_8000_0080_5F9B_34FB;

impl Uuid {
    pub const fn from_u128(value: u128) -> Self {
        Uuid(value)
    }

    /// A 16- or 32-bit UUID, expanded with the Bluetooth base UUID.
    pub const fn from_u32(short: u32) -> Self {
        Uuid(BASE_UUID | ((short as u128) << 96))
    }

    /// RFCOMM's protocol UUID (`0x0003`).
    /// Spec: transport.md#sdp-rfcomm-channel
    pub const RFCOMM: Uuid = Uuid::from_u32(0x0003);

    pub fn to_be_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    /// A UUID as SDP encodes it: 2, 4 or 16 big-endian bytes.
    /// Spec: transport.md#sdp-data-elements
    pub(crate) fn from_sdp(bytes: &[u8]) -> Result<Self> {
        match *bytes {
            [a, b] => Ok(Uuid::from_u32(u32::from(u16::from_be_bytes([a, b])))),
            [a, b, c, d] => Ok(Uuid::from_u32(u32::from_be_bytes([a, b, c, d]))),
            _ => {
                let long: [u8; 16] = bytes
                    .try_into()
                    .map_err(|_| Error::Protocol(format!("SDP UUID of {} bytes", bytes.len())))?;
                Ok(Uuid(u128::from_be_bytes(long)))
            }
        }
    }
}

impl fmt::Display for Uuid {
    /// Lowercase and hyphenated, as BlueZ writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = format!("{:032x}", self.0);
        let part = |range: std::ops::Range<usize>| hex.get(range).unwrap_or_default();
        write!(
            f,
            "{}-{}-{}-{}-{}",
            part(0..8),
            part(8..12),
            part(12..16),
            part(16..20),
            part(20..32)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_round_trip_and_kernel_order() {
        let addr: BdAddr = "AA:bb:CC:01:02:03".parse().unwrap();
        assert_eq!(addr.to_string(), "AA:BB:CC:01:02:03");
        assert_eq!(addr.kernel_order(), [0x03, 0x02, 0x01, 0xCC, 0xBB, 0xAA]);
    }

    #[test]
    fn bad_addresses_are_rejected() {
        for text in [
            "",
            "AA:BB:CC:DD:EE",
            "AA:BB:CC:DD:EE:FF:00",
            "AA:BB:CC:DD:EE:F",
            "GG:BB:CC:DD:EE:FF",
        ] {
            assert!(text.parse::<BdAddr>().is_err(), "{text}");
        }
    }

    #[test]
    fn uuid_display_matches_bluez() {
        let uuid = Uuid::from_u128(0x0000_079A_D102_11E1_9B23_0002_5B00_A5A5);
        assert_eq!(uuid.to_string(), "0000079a-d102-11e1-9b23-00025b00a5a5");
        assert_eq!(Uuid::RFCOMM.to_string(), "00000003-0000-1000-8000-00805f9b34fb");
    }

    #[test]
    fn short_sdp_uuids_expand_with_the_base() {
        assert_eq!(Uuid::from_sdp(&[0x00, 0x03]).unwrap(), Uuid::RFCOMM);
        assert_eq!(Uuid::from_sdp(&[0x00, 0x00, 0x00, 0x03]).unwrap(), Uuid::RFCOMM);
        assert_eq!(Uuid::from_sdp(&Uuid::RFCOMM.to_be_bytes()).unwrap(), Uuid::RFCOMM);
        assert!(Uuid::from_sdp(&[1, 2, 3]).is_err());
    }
}
