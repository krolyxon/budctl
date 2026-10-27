//! Paired devices, from BlueZ over the system D-Bus.
//!
//! The only use of D-Bus in budctl is listing paired devices (CLAUDE.md, hard rule 1). budctl
//! connects to the system bus socket at its fixed path itself, so no environment variable can
//! point the connection anywhere else (such as a TCP address).

use std::collections::HashMap;
use std::os::unix::net::UnixStream;

use zbus::blocking::connection::Builder;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::bt::BdAddr;
use crate::error::{Error, Result};

/// The system bus. `DBUS_SYSTEM_BUS_ADDRESS` is deliberately ignored.
const SYSTEM_BUS_SOCKET: &str = "/var/run/dbus/system_bus_socket";

/// A device BlueZ has paired, as BlueZ describes it.
#[derive(Debug, Clone)]
pub struct PairedDevice {
    pub address: BdAddr,
    /// The name the device advertises. Device modules recognize models by this.
    pub name: String,
    /// The name the user gave it (BlueZ falls back to `name`). Shown to people.
    pub alias: String,
    pub connected: bool,
    /// Service UUIDs BlueZ found, lowercase and hyphenated.
    pub uuids: Vec<String>,
}

type Properties = HashMap<String, OwnedValue>;
type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, Properties>>;

fn bluez_error(error: impl std::fmt::Display) -> Error {
    Error::Bluez(error.to_string())
}

/// Every paired device BlueZ knows, sorted by address.
pub fn paired_devices() -> Result<Vec<PairedDevice>> {
    let stream =
        UnixStream::connect(SYSTEM_BUS_SOCKET).map_err(|e| Error::Bluez(format!("{SYSTEM_BUS_SOCKET}: {e}")))?;
    let connection = Builder::async_io_unix_stream(stream).build().map_err(bluez_error)?;
    let reply = connection
        .call_method(
            Some("org.bluez"),
            "/",
            Some("org.freedesktop.DBus.ObjectManager"),
            "GetManagedObjects",
            &(),
        )
        .map_err(bluez_error)?;
    let objects: ManagedObjects = reply.body().deserialize().map_err(bluez_error)?;

    let mut devices: Vec<PairedDevice> = objects
        .values()
        .filter_map(|interfaces| interfaces.get("org.bluez.Device1"))
        .filter_map(device)
        .collect();
    devices.sort_by_key(|d| d.address);
    Ok(devices)
}

/// A paired device from its `org.bluez.Device1` properties; `None` if it is not paired or has
/// no usable address.
fn device(properties: &Properties) -> Option<PairedDevice> {
    if !flag(properties, "Paired") {
        return None;
    }
    let address = text(properties, "Address")?.parse().ok()?;
    let name = text(properties, "Name").unwrap_or_default();
    let alias = text(properties, "Alias").unwrap_or_else(|| name.clone());
    let uuids = properties
        .get("UUIDs")
        .and_then(|v| match &**v {
            Value::Array(array) => Some(
                array
                    .iter()
                    .filter_map(|item| match item {
                        Value::Str(s) => Some(s.as_str().to_ascii_lowercase()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    Some(PairedDevice {
        address,
        name,
        alias,
        connected: flag(properties, "Connected"),
        uuids,
    })
}

fn text(properties: &Properties, key: &str) -> Option<String> {
    match &**properties.get(key)? {
        Value::Str(s) => Some(s.as_str().to_owned()),
        _ => None,
    }
}

fn flag(properties: &Properties, key: &str) -> bool {
    matches!(properties.get(key).map(|v| &**v), Some(Value::Bool(true)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn properties(entries: Vec<(&str, Value<'static>)>) -> Properties {
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), OwnedValue::try_from(v).unwrap()))
            .collect()
    }

    #[test]
    fn paired_device_from_properties() {
        let props = properties(vec![
            ("Address", Value::from("AA:BB:CC:DD:EE:FF")),
            ("Name", Value::from("realme Buds T310")),
            ("Alias", Value::from("my buds")),
            ("Paired", Value::from(true)),
            ("Connected", Value::from(true)),
            ("UUIDs", Value::from(vec!["0000110B-0000-1000-8000-00805F9B34FB"])),
        ]);
        let device = device(&props).unwrap();
        assert_eq!(device.address.to_string(), "AA:BB:CC:DD:EE:FF");
        assert_eq!(device.name, "realme Buds T310");
        assert_eq!(device.alias, "my buds");
        assert!(device.connected);
        assert_eq!(device.uuids, ["0000110b-0000-1000-8000-00805f9b34fb"]);
    }

    #[test]
    fn unpaired_or_unaddressed_devices_are_skipped() {
        let unpaired = properties(vec![
            ("Address", Value::from("AA:BB:CC:DD:EE:FF")),
            ("Paired", Value::from(false)),
        ]);
        assert!(device(&unpaired).is_none());
        let no_address = properties(vec![("Paired", Value::from(true))]);
        assert!(device(&no_address).is_none());
    }

    #[test]
    fn alias_falls_back_to_name() {
        let props = properties(vec![
            ("Address", Value::from("AA:BB:CC:DD:EE:FF")),
            ("Name", Value::from("realme Buds T310")),
            ("Paired", Value::from(true)),
        ]);
        let device = device(&props).unwrap();
        assert_eq!(device.alias, "realme Buds T310");
        assert!(!device.connected);
    }
}
