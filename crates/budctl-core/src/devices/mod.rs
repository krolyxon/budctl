//! Device plugins.
//!
//! To support a new pair of buds, add one module here and list it in [`REGISTRY`]. Transport,
//! CLI and GUI code do not change.

use crate::bluez::PairedDevice;
use crate::bt::Uuid;
use crate::error::{Error, Result};
use crate::status::{NoiseControl, NoiseMode, Status, TouchControls};
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
}

/// The supported model a paired device is, if any.
pub fn identify(device: &PairedDevice) -> Option<&'static dyn Device> {
    REGISTRY.iter().copied().find(|model| model.matches(device))
}
