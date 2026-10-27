//! budctl-core: control wireless earbuds over Bluetooth, with device support as plugins.
//!
//! A front end (CLI, GUI) calls [`scan`], picks a device with [`select`], opens a session with
//! [`connect`] and then reads or changes settings through [`devices::Session`].
//!
//! No network: the only sockets are Bluetooth sockets and the local system D-Bus (BlueZ).
//! See CLAUDE.md, hard rule 1.

pub mod bluez;
pub mod bt;
pub mod debug;
pub mod devices;
mod error;
mod reader;
pub mod status;
pub mod transport;

pub use error::{Error, Kind, Result};

use bluez::PairedDevice;
use bt::BdAddr;
use devices::{Device, Session};
use transport::rfcomm::Rfcomm;
use transport::{Hexdump, Transport, sdp};

/// A paired device and the model budctl recognized it as, if any.
pub struct Found {
    pub device: PairedDevice,
    pub model: Option<&'static dyn Device>,
}

/// Every paired device BlueZ knows, each with its model when budctl supports it.
pub fn scan() -> Result<Vec<Found>> {
    Ok(bluez::paired_devices()?
        .into_iter()
        .map(|device| Found {
            model: devices::identify(&device),
            device,
        })
        .collect())
}

/// Pick the device to talk to: the one at `address`, or else the only connected supported one.
pub fn select(found: Vec<Found>, address: Option<BdAddr>) -> Result<(PairedDevice, &'static dyn Device)> {
    if let Some(address) = address {
        let found = found
            .into_iter()
            .find(|f| f.device.address == address)
            .ok_or_else(|| Error::NoDevice(format!("no paired device has the address {address}")))?;
        let model = found.model.ok_or_else(|| {
            Error::NoDevice(format!(
                "{} ({address}) is not a model budctl supports",
                found.device.alias
            ))
        })?;
        return Ok((found.device, model));
    }

    let supported: Vec<(PairedDevice, &'static dyn Device)> = found
        .into_iter()
        .filter_map(|f| f.model.map(|model| (f.device, model)))
        .collect();
    let (connected, idle): (Vec<_>, Vec<_>) = supported.into_iter().partition(|(d, _)| d.connected);
    let mut connected = connected.into_iter();
    match (connected.next(), connected.next()) {
        (Some(only), None) => Ok(only),
        (Some(first), Some(second)) => {
            let addresses: Vec<String> = [first, second]
                .into_iter()
                .chain(connected)
                .map(|(d, _)| format!("{} ({})", d.address, d.alias))
                .collect();
            Err(Error::Usage(format!(
                "more than one supported device is connected; pick one with --device: {}",
                addresses.join(", ")
            )))
        }
        (None, _) => {
            let mut idle = idle.into_iter();
            match (idle.next(), idle.next()) {
                // One paired pair of buds: let `connect` say plainly that it is not connected.
                (Some(only), None) => Ok(only),
                (Some(_), Some(_)) => Err(Error::NotConnected(
                    "none of your supported buds is connected; connect them in your Bluetooth settings first"
                        .to_owned(),
                )),
                (None, _) => Err(Error::NoDevice(
                    "no paired device budctl supports (pair the buds in your Bluetooth settings first)".to_owned(),
                )),
            }
        }
    }
}

/// Open a control session: find the control service with SDP, connect RFCOMM, run the model's
/// init sequence.
pub fn connect(device: &PairedDevice, model: &'static dyn Device) -> Result<Box<dyn Session>> {
    if !device.connected {
        return Err(Error::NotConnected(format!(
            "{} ({}) is paired but not connected; connect it in your Bluetooth settings first",
            device.alias, device.address
        )));
    }
    for &service in model.rfcomm_uuids() {
        let Some(channel) = sdp::rfcomm_channel(device.address, service)? else {
            debug::note(|| format!("SDP: {} has no service {service}", device.address));
            continue;
        };
        debug::note(|| format!("SDP: service {service} is on RFCOMM channel {channel}"));
        let rfcomm = Box::new(Rfcomm::connect(device.address, channel)?);
        let link: Box<dyn Transport> = if debug::enabled() {
            Box::new(Hexdump::new(rfcomm))
        } else {
            rfcomm
        };
        return model.open(link);
    }
    Err(Error::NoService(device.address.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(address: &str, name: &str, connected: bool) -> Found {
        let device = PairedDevice {
            address: address.parse().unwrap(),
            name: name.to_owned(),
            alias: name.to_owned(),
            connected,
            uuids: Vec::new(),
        };
        Found {
            model: devices::identify(&device),
            device,
        }
    }

    const T310: &str = "realme Buds T310";

    #[test]
    fn picks_the_only_connected_supported_device() {
        let list = vec![
            found("00:00:00:00:00:01", "Some Speaker", true),
            found("00:00:00:00:00:02", T310, true),
            found("00:00:00:00:00:03", T310, false),
        ];
        let (device, model) = select(list, None).unwrap();
        assert_eq!(device.address.to_string(), "00:00:00:00:00:02");
        assert_eq!(model.name(), T310);
    }

    #[test]
    fn several_connected_buds_need_a_choice() {
        let list = || {
            vec![
                found("00:00:00:00:00:02", T310, true),
                found("00:00:00:00:00:03", T310, true),
            ]
        };
        assert_eq!(select(list(), None).err().map(|e| e.kind()), Some(Kind::Usage));
        let (device, _) = select(list(), Some("00:00:00:00:00:03".parse().unwrap())).unwrap();
        assert_eq!(device.address.to_string(), "00:00:00:00:00:03");
    }

    #[test]
    fn nothing_supported_or_unknown_address_is_not_found() {
        let list = || vec![found("00:00:00:00:00:01", "Some Speaker", true)];
        assert!(matches!(select(list(), None), Err(Error::NoDevice(_))));
        assert!(matches!(
            select(list(), Some("00:00:00:00:00:01".parse().unwrap())),
            Err(Error::NoDevice(_))
        ));
        assert!(matches!(
            select(list(), Some("00:00:00:00:00:09".parse().unwrap())),
            Err(Error::NoDevice(_))
        ));
    }

    #[test]
    fn one_idle_pair_is_selected_so_connect_can_explain() {
        let (device, model) = select(vec![found("00:00:00:00:00:02", T310, false)], None).unwrap();
        assert!(matches!(connect(&device, model), Err(Error::NotConnected(_))));
    }
}
