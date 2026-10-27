//! Device plugins.
//!
//! To support a new pair of buds, add one module here and list it in [`REGISTRY`]. Transport,
//! CLI and GUI code do not change.

use crate::bluez::PairedDevice;
use crate::bt::Uuid;
use crate::error::{Error, Result};
use crate::status::{
    DualDevice, EqBand, EqPreset, EqPresets, NoiseControl, NoiseMode, Status, Switch, TouchChange, TouchControls,
};
use crate::transport::Transport;

pub mod realme_t310;

/// Every model budctl supports.
pub static REGISTRY: &[&dyn Device] = &[&realme_t310::RealmeT310];

/// One supported model.
pub trait Device: Sync {
    /// Model name, e.g. `realme Buds T310`.
    fn name(&self) -> &'static str;

    /// Whether a paired device is this model, judged from BlueZ data alone.
    fn matches(&self, device: &PairedDevice) -> bool;

    /// The control service(s), in the order to try. SDP turns each into an RFCOMM channel.
    fn rfcomm_uuids(&self) -> &'static [Uuid];

    /// Run the model's init sequence on a fresh link and return a ready session.
    fn open(&self, link: Box<dyn Transport>) -> Result<Box<dyn Session>>;
}

/// An open control link to one pair of buds. Dropping it closes the link.
pub trait Session: Send {
    /// Answer what the buds sent since the last call, without waiting. Some buds drop the link
    /// when their own requests (the time request, for one) go unanswered, so a front end that
    /// keeps a session open between commands calls this every so often. An error means the link
    /// is gone.
    fn serve(&mut self) -> Result<()> {
        Ok(())
    }

    /// Battery, noise control, EQ, switches and whatever else the model can read.
    fn status(&mut self) -> Result<Status>;

    /// The touch-control (gesture) settings of each bud.
    fn touch_controls(&mut self) -> Result<TouchControls> {
        Err(Error::Unsupported(
            "these buds have no touch-control settings budctl can read".to_owned(),
        ))
    }

    /// Set the noise-control mode, then read it back. Returns the mode the buds now report.
    fn set_noise_control(&mut self, _mode: NoiseMode) -> Result<NoiseControl> {
        Err(Error::Unsupported(
            "these buds have no noise-control setting budctl can change".to_owned(),
        ))
    }

    /// The EQ presets on the buds, built-in and custom, and the one in use.
    fn eq_presets(&mut self) -> Result<EqPresets> {
        Err(Error::Unsupported(
            "budctl cannot list the EQ presets of these buds".to_owned(),
        ))
    }

    /// Select a built-in EQ preset by name, then read it back. Returns the preset now in use.
    fn set_eq(&mut self, _preset: &str) -> Result<String> {
        Err(Error::Unsupported(
            "budctl cannot change the EQ on these buds".to_owned(),
        ))
    }

    /// Change the gains of some bands of a custom EQ preset (`custom-<id>`), save it and select
    /// it, then read it back. With no bands, it only selects the preset.
    fn set_custom_eq(&mut self, preset: &str, _bands: &[EqBand]) -> Result<EqPreset> {
        Err(Error::Unsupported(format!(
            "budctl cannot change {preset} on these buds"
        )))
    }

    /// The lowest and the highest level [`Session::set_bass_level`] takes, when the buds have a
    /// bass level to set.
    fn bass_range(&self) -> Option<(i8, i8)> {
        None
    }

    /// Create a custom EQ preset called `name`, flat, with the bands of the buds' other custom
    /// presets. Returns it as the buds list it afterwards.
    fn create_custom_eq(&mut self, name: &str) -> Result<EqPreset> {
        Err(Error::Unsupported(format!(
            "budctl cannot create the preset {name} on these buds"
        )))
    }

    /// Give a custom EQ preset (`custom-<id>`) a new name. The buds save it, which also selects
    /// it. Returns it as the buds list it afterwards.
    fn rename_custom_eq(&mut self, preset: &str, _name: &str) -> Result<EqPreset> {
        Err(Error::Unsupported(format!(
            "budctl cannot rename {preset} on these buds"
        )))
    }

    /// Delete a custom EQ preset (`custom-<id>`). Returns the custom presets left, as the buds
    /// list them afterwards: their ids can change.
    fn delete_custom_eq(&mut self, preset: &str) -> Result<Vec<EqPreset>> {
        Err(Error::Unsupported(format!(
            "budctl cannot delete {preset} on these buds"
        )))
    }

    /// Set the bass level. Only the ack can be checked when the buds cannot report the level.
    fn set_bass_level(&mut self, _level: i8) -> Result<()> {
        Err(Error::Unsupported(
            "budctl cannot change the bass level on these buds".to_owned(),
        ))
    }

    /// Start (`true`) or stop ringing the buds so they can be found. Only the ack can be
    /// checked: there is no state to read back.
    fn find(&mut self, _ring: bool) -> Result<()> {
        Err(Error::Unsupported("these buds cannot ring through budctl".to_owned()))
    }

    /// The phones and computers the buds keep for dual connection, and which are connected.
    fn dual_devices(&mut self) -> Result<Vec<DualDevice>> {
        Err(Error::Unsupported(
            "these buds do not list the devices they connect to".to_owned(),
        ))
    }

    /// Turn an on/off setting ([`Switch::key`]) on or off, then read it back.
    fn set_switch(&mut self, key: &str, _on: bool) -> Result<Switch> {
        Err(Error::Unsupported(format!("budctl cannot change {key} on these buds")))
    }

    /// Set the noise-control modes a hold steps through (`off`, `transparency`, `anc`), then
    /// read it back. Returns the modes the buds now report.
    fn set_hold_cycle(&mut self, _modes: &[&str]) -> Result<Vec<String>> {
        Err(Error::Unsupported(
            "budctl cannot change the hold cycle on these buds".to_owned(),
        ))
    }

    /// Give a touch gesture a new action, on `side` (`left`, `right` or `both`), then read the
    /// table back. With `dry_run`, read the table and work out the write, but send nothing.
    fn set_touch(&mut self, _side: &str, _gesture: &str, _action: &str, _dry_run: bool) -> Result<TouchChange> {
        Err(Error::Unsupported(
            "budctl cannot change the touch controls on these buds".to_owned(),
        ))
    }
}

/// The supported model a paired device is, if any.
pub fn identify(device: &PairedDevice) -> Option<&'static dyn Device> {
    REGISTRY.iter().copied().find(|model| model.matches(device))
}
