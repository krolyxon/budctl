//! The buds the window shows: made-up ones for `--demo` (roughly what the owner's T310 reported
//! at the last checkpoints), or an empty pair until the real buds answer.

use budctl_core::status::{
    Battery, Charge, EqBand, EqPreset, EqPresets, Gesture, GestureChoice, NoiseControl, Status, Switch, TouchControls,
};

/// The buds the window shows.
pub struct Buds {
    pub name: String,
    pub connected: bool,
    pub status: Status,
    /// The EQ presets, when the buds list them.
    pub eq: Option<EqPresets>,
    /// The touch controls, when the buds report them.
    pub touch: Option<TouchControls>,
    /// The levels the bass level takes, when the buds have one to set.
    pub bass_range: Option<(i8, i8)>,
    /// The bass level last set from this window. The buds do not report it.
    pub bass: Option<i8>,
}

impl Buds {
    /// The name to show for the EQ preset in use.
    pub fn eq_name(&self) -> String {
        eq_name(self.status.eq.as_deref(), self.eq.as_ref())
    }

    /// Records that `now` is the EQ preset in use.
    pub fn set_eq(&mut self, now: String) {
        if let Some(eq) = &mut self.eq {
            eq.current = Some(now.clone());
        }
        self.status.eq = Some(now);
    }

    /// Takes in a custom preset as the buds report it after a write: saved, and in use. A new
    /// one joins the list.
    pub fn set_custom(&mut self, preset: EqPreset) {
        let now = key(&preset);
        if let Some(eq) = self.eq.as_mut() {
            match eq.presets.iter_mut().find(|p| p.custom && p.id == preset.id) {
                Some(shown) => *shown = preset,
                None => eq.presets.push(preset),
            }
        }
        self.set_eq(now);
    }

    /// Takes in the whole preset list, as the buds report it.
    pub fn set_presets(&mut self, eq: EqPresets) {
        self.status.eq.clone_from(&eq.current);
        self.eq = Some(eq);
    }

    /// The custom presets' names, as the buds store them.
    pub fn custom_names(&self) -> impl Iterator<Item = (u8, &str)> {
        self.eq
            .iter()
            .flat_map(|eq| eq.presets.iter())
            .filter(|p| p.custom)
            .map(|p| (p.id, p.name.as_str()))
    }

    /// The preset with this id.
    pub fn preset(&self, id: u8) -> Option<&EqPreset> {
        self.eq.as_ref()?.presets.iter().find(|p| p.id == id)
    }

    /// What a gesture of a bud (`left`, `right`) does now.
    pub fn action(&self, side: &str, gesture: &str) -> Option<&str> {
        self.touch
            .as_ref()?
            .gestures
            .iter()
            .find(|g| g.side == side && g.gesture == gesture && g.group.is_none())
            .map(|g| g.action.as_str())
    }
}

/// The made-up buds of `--demo`.
pub fn demo() -> Buds {
    let charge = |level| Some(Charge { level, charging: false });
    let switch = |key, label, on| Switch { key, label, on };
    let builtin = |id, name: &str| EqPreset {
        name: name.to_owned(),
        id,
        custom: false,
        bands: Vec::new(),
        range: None,
    };
    let band = |hz, gain| EqBand { hz, gain };
    let gesture = |side: &str, gesture: &str, action: &str| Gesture {
        side: side.to_owned(),
        gesture: gesture.to_owned(),
        action: action.to_owned(),
        group: None,
    };
    let choice = |gesture: &str, actions: &[&str]| GestureChoice {
        gesture: gesture.to_owned(),
        actions: actions.iter().map(|&a| a.to_owned()).collect(),
    };
    let names = |names: &[&str]| names.iter().map(|&n| n.to_owned()).collect::<Vec<_>>();
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
        eq: Some(EqPresets {
            current: Some("custom-4".to_owned()),
            presets: vec![
                builtin(0, "pure-raw"),
                builtin(1, "deep-bass"),
                builtin(2, "pure-vocals"),
                builtin(3, "pure-bass"),
                EqPreset {
                    name: "Custom1".to_owned(),
                    id: 4,
                    custom: true,
                    bands: vec![
                        band(50, 3),
                        band(250, 1),
                        band(1000, 0),
                        band(4000, -1),
                        band(8000, 1),
                        band(16000, 2),
                    ],
                    range: Some((-6, 6)),
                },
            ],
            warnings: Vec::new(),
        }),
        touch: Some(TouchControls {
            gestures: vec![
                gesture("left", "double-tap", "play-pause"),
                gesture("left", "triple-tap", "previous"),
                gesture("left", "hold", "noise-cycle"),
                gesture("right", "double-tap", "play-pause"),
                gesture("right", "triple-tap", "next"),
                gesture("right", "hold", "noise-cycle"),
            ],
            hold_cycle: Some(names(&["off", "anc"])),
            choices: vec![
                choice("double-tap", &["none", "play-pause", "next", "voice-assistant"]),
                choice(
                    "triple-tap",
                    &[
                        "none",
                        "previous",
                        "next",
                        "voice-assistant",
                        "volume-up",
                        "volume-down",
                        "game-mode",
                    ],
                ),
                choice("hold", &["none", "noise-cycle", "voice-assistant", "game-mode"]),
            ],
            cycle_modes: names(&["off", "transparency", "anc"]),
            warnings: Vec::new(),
        }),
        bass_range: Some((-5, 5)),
        bass: None,
    }
}

/// What the window shows before the real buds answer.
pub fn unknown() -> Buds {
    Buds {
        name: "Your buds".to_owned(),
        connected: false,
        status: Status::default(),
        eq: None,
        touch: None,
        bass_range: None,
        bass: None,
    }
}

/// The name a preset is selected by, as in [`Status::eq`]: a built-in one's own name,
/// `custom-<id>` for a custom one.
pub fn key(preset: &EqPreset) -> String {
    if preset.custom {
        format!("custom-{}", preset.id)
    } else {
        preset.name.clone()
    }
}

/// A preset's name to show: a custom one's stored name, a built-in one's in words.
pub fn shown_name(preset: &EqPreset) -> String {
    if preset.custom {
        preset.name.clone()
    } else {
        words(&preset.name)
    }
}

/// The name to show for the EQ preset in use: a custom preset's own name (`custom-4` →
/// "Custom1"), else the preset's name in words (`pure-bass` → "Pure bass").
pub fn eq_name(current: Option<&str>, presets: Option<&EqPresets>) -> String {
    let Some(current) = current else {
        return "Unknown".to_owned();
    };
    presets
        .and_then(|eq| eq.presets.iter().find(|p| key(p) == current))
        .map_or_else(|| words(current), shown_name)
}

/// `pure-bass` → "Pure bass".
pub fn words(name: &str) -> String {
    let words = name.replace('-', " ");
    let mut letters = words.chars();
    letters
        .next()
        .map(|first| first.to_uppercase().chain(letters).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eq_names() {
        let buds = demo();
        assert_eq!(eq_name(Some("custom-4"), buds.eq.as_ref()), "Custom1");
        assert_eq!(eq_name(Some("custom-4"), None), "Custom 4");
        assert_eq!(eq_name(Some("pure-bass"), buds.eq.as_ref()), "Pure bass");
        assert_eq!(eq_name(None, None), "Unknown");
        assert_eq!(buds.eq_name(), "Custom1");
    }

    #[test]
    fn eq_changes_show() {
        let mut buds = demo();
        buds.set_eq("deep-bass".to_owned());
        assert_eq!(buds.eq_name(), "Deep bass");
        assert_eq!(buds.eq.as_ref().unwrap().current.as_deref(), Some("deep-bass"));

        let mut custom = buds.preset(4).unwrap().clone();
        custom.bands[0].gain = -6;
        buds.set_custom(custom);
        assert_eq!(buds.status.eq.as_deref(), Some("custom-4"));
        assert_eq!(buds.preset(4).unwrap().bands[0].gain, -6);
    }
}
