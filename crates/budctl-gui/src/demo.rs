//! The buds the window shows: made-up ones for `--demo` (roughly what the owner's T310 reported
//! at the last checkpoint), or an empty pair until the real buds answer.

use budctl_core::status::{Battery, Charge, NoiseControl, Status, Switch};

/// The buds the window shows.
pub struct Buds {
    pub name: String,
    pub connected: bool,
    pub status: Status,
    /// The name to show for the EQ preset in use.
    pub eq_name: String,
}

pub fn buds() -> Buds {
    let charge = |level| Some(Charge { level, charging: false });
    let switch = |key, label, on| Switch { key, label, on };
    Buds {
        name: "realme Buds T310".to_owned(),
        connected: true,
        status: Status {
            battery: Battery {
                left: charge(80),
                right: charge(80),
                case: None,
            },
            noise_control: Some(NoiseControl {
                mode: "anc".to_owned(),
                level: Some("deep".to_owned()),
            }),
            eq: Some("custom-4".to_owned()),
            bass_level: None,
            switches: vec![
                switch("game", "Game mode", false),
                switch("dual", "Dual connection", true),
                switch("wind", "Wind-noise reduction", true),
                switch("spatial", "3D sound", true),
                switch("bass-boost", "Bass boost", true),
            ],
            codec: Some("SBC".to_owned()),
            firmware: Some("1.1.0.43".to_owned()),
            warnings: Vec::new(),
        },
        eq_name: "Custom1".to_owned(),
    }
}

/// What the window shows before the real buds answer.
pub fn unknown() -> Buds {
    Buds {
        name: "Your buds".to_owned(),
        connected: false,
        status: Status::default(),
        eq_name: String::new(),
    }
}
