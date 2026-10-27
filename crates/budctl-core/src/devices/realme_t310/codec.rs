//! realme Buds T310 commands, tables and pure decoders. No I/O here, so every function can be
//! tested with bytes from the spec. Each constant cites docs/protocol/realme_t310.md.

use crate::error::{Error, Result};
use crate::reader::Reader;
use crate::status::{
    AncLevel, Battery, Charge, EqBand, EqPreset, Gesture, GestureChoice, NoiseControl, NoiseMode, Switch,
};

/// Spec: realme_t310.md#bitmap
pub(super) const HANDSHAKE: u16 = 0x0100;
/// Spec: realme_t310.md#identity
pub(super) const PRODUCT_ID: u16 = 0x0103;
/// Spec: realme_t310.md#firmware
pub(super) const FIRMWARE: u16 = 0x0105;
/// Spec: realme_t310.md#battery
pub(super) const BATTERY: u16 = 0x0106;
/// Spec: realme_t310.md#gestures
pub(super) const GESTURES: u16 = 0x0108;
/// Spec: realme_t310.md#anc-read
pub(super) const NOISE_CONTROL: u16 = 0x010C;
/// Spec: realme_t310.md#switches
pub(super) const SWITCHES: u16 = 0x010D;
/// Spec: realme_t310.md#anc-write
pub(super) const NOISE_CONTROL_WRITE: u16 = 0x0404;
/// Spec: realme_t310.md#switch-write
pub(super) const SWITCH_WRITE: u16 = 0x0403;
/// Spec: realme_t310.md#eq-write
pub(super) const EQ_SELECT: u16 = 0x0406;
/// Spec: realme_t310.md#find
pub(super) const FIND: u16 = 0x0400;
/// Spec: realme_t310.md#bass-write
pub(super) const BASS_LEVEL_WRITE: u16 = 0x041B;
/// The bass-level range. Spec: realme_t310.md#bass-write
pub(super) const BASS_RANGE: (i8, i8) = (-5, 5);

/// `FB 05 <level s8>`: the range bytes, then the level. Spec: realme_t310.md#bass-write
pub(super) fn bass_write_payload(level: i8) -> Result<[u8; 3]> {
    let (min, max) = BASS_RANGE;
    if !(min..=max).contains(&level) {
        return Err(Error::Usage(format!(
            "the bass level goes from {min} to +{max}; {level} is outside"
        )));
    }
    let [min, max, level] = [min, max, level].map(|v| v.to_le_bytes()[0]);
    Ok([min, max, level])
}
/// Spec: realme_t310.md#gesture-write
pub(super) const GESTURE_WRITE: u16 = 0x0401;

/// `01` starts ringing, `00` stops it. Spec: realme_t310.md#find
pub(super) fn find_payload(ring: bool) -> [u8; 1] {
    [u8::from(ring)]
}
/// Spec: realme_t310.md#eq-read
pub(super) const EQ_PRESET: u16 = 0x010F;
/// Spec: realme_t310.md#codec
pub(super) const CODEC: u16 = 0x0114;
/// Spec: realme_t310.md#custom-eq
pub(super) const CUSTOM_EQ_LIST: u16 = 0x0122;
/// Spec: realme_t310.md#custom-eq
pub(super) const CUSTOM_EQ_WRITE: u16 = 0x0418;
/// The `0x0418` action that saves a whole preset and selects it. Spec: realme_t310.md#custom-eq
const CUSTOM_EQ_SAVE: u8 = 0x02;
/// Spec: realme_t310.md#events-offered
pub(super) const EVENTS_OFFERED: u16 = 0x0200;
/// Spec: realme_t310.md#subscribe
pub(super) const SUBSCRIBE: u16 = 0x0205;
/// Spec: realme_t310.md#fit-test
pub(super) const FIT_TEST: u16 = 0x0405;

/// Spec: realme_t310.md#identity
pub(super) const T310_PRODUCT_ID: u32 = 0x06_3C_12;

/// Payload asking for the current noise-control mode. Spec: realme_t310.md#anc-read
pub(super) const NOISE_MODE_QUERY: [u8; 2] = [0x01, 0x01];
/// Payload asking for the hold's noise-control cycle (one setting for both buds).
/// Spec: realme_t310.md#hold-cycle
pub(super) const HOLD_CYCLE_QUERY: [u8; 2] = [0x02, 0x01];
/// Count, then the T310's switch ids: `05 06 11 1A 1B 1D`. Spec: realme_t310.md#switches
pub(super) const SWITCH_QUERY: [u8; 7] = [6, 0x05, 0x06, 0x11, 0x1A, 0x1B, 0x1D];

/// Event codes to subscribe to; the optional ones only when their command is allowed.
/// Spec: realme_t310.md#subscribe
const EVENTS: [(u8, Option<u16>); 4] = [(0x01, None), (0x02, None), (0x03, None), (0x04, Some(FIT_TEST))];

/// Spec: realme_t310.md#switches
const SWITCH_NAMES: [(u8, &str, &str); 5] = [
    (0x06, "game", "Game mode"),
    (0x11, "dual", "Dual connection"),
    (0x1A, "wind", "Wind-noise reduction"),
    (0x1B, "spatial", "3D sound"),
    (0x1D, "bass-boost", "Bass boost"),
];

/// Noise-control masks: mode and level. Spec: realme_t310.md#anc-read
const NOISE_MODES: [(u32, &str, Option<&str>); 5] = [
    (0x01, "off", None),
    (0x02, "transparency", None),
    (0x04, "anc", Some("light")),
    (0x08, "anc", Some("deep")),
    (0x10, "anc", Some("medium")),
];

/// Bits of the hold cycle. Spec: realme_t310.md#hold-cycle
const CYCLE_MODES: [(u32, &str); 3] = [(0x01, "off"), (0x02, "transparency"), (0x08, "anc")];

/// Built-in EQ presets by id. Spec: realme_t310.md#eq-read
const EQ_PRESETS: [(u8, &str); 4] = [
    (0x00, "pure-raw"),
    (0x01, "deep-bass"),
    (0x02, "pure-vocals"),
    (0x03, "pure-bass"),
];

/// Spec: realme_t310.md#codec
const CODECS: [(u8, &str); 8] = [
    (0x01, "SBC"),
    (0x02, "AAC"),
    (0x03, "LDAC"),
    (0x04, "aptX"),
    (0x05, "aptX HD"),
    (0x06, "aptX Adaptive"),
    (0x07, "LHDC"),
    (0x08, "LHDC V5"),
];

/// Gesture-table names. Spec: realme_t310.md#gestures
const SIDES: [(u8, &str); 3] = [(0x01, "left"), (0x02, "right"), (0x04, "both")];
/// Spec: realme_t310.md#gestures
const MAIN_GROUP: u8 = 0x01;
/// Spec: realme_t310.md#gestures
const ACTS: [(u8, &str); 6] = [
    (0x01, "single-tap"),
    (0x02, "double-tap"),
    (0x03, "triple-tap"),
    (0x04, "hold"),
    (0x05, "slide"),
    (0x06, "long-hold"),
];
/// Spec: realme_t310.md#gestures
const FUNCTIONS: [(u8, &str); 15] = [
    (0x00, "none"),
    (0x01, "play-pause"),
    (0x03, "voice-assistant"),
    (0x05, "previous"),
    (0x06, "next"),
    (0x07, "volume"),
    (0x08, "noise-cycle"),
    (0x09, "favourite-music"),
    (0x0A, "switch-track"),
    (0x0B, "volume-up"),
    (0x0C, "volume-down"),
    (0x0D, "switch-devices"),
    (0x11, "game-mode"),
    (0x1C, "decline-call"),
    (0x1D, "answer-call"),
];

fn name_of<K: PartialEq + Copy>(table: &[(K, &'static str)], key: K) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, name)| *name)
}

/// When a command may be sent. Spec: realme_t310.md#bitmap
enum Gate {
    Always,
    Bit(u8),
}

/// The gate of every command budctl uses. An unlisted command is never sent.
/// Spec: realme_t310.md#bitmap
fn gate(cmd: u16) -> Option<Gate> {
    Some(match cmd {
        0x0100..=0x0104 | 0x0106 | 0x010B | 0x010D | 0x0200 => Gate::Always,
        0x0105 => Gate::Bit(0),
        0x0108 | 0x0401 => Gate::Bit(3),
        0x0109 => Gate::Bit(4),
        0x0400 => Gate::Bit(5),
        0x0403 => Gate::Bit(7),
        0x010C | 0x0404 => Gate::Bit(8),
        0x0405 => Gate::Bit(9),
        0x010F | 0x0406 => Gate::Bit(10),
        0x0114 => Gate::Bit(17),
        0x0205 => Gate::Bit(22),
        0x0122 | 0x0418 => Gate::Bit(34),
        0x0124 | 0x041B => Gate::Bit(38),
        _ => return None,
    })
}

/// The commands these buds accept, from the handshake reply. Spec: realme_t310.md#bitmap
#[derive(Debug)]
pub(super) struct Commands {
    bitmap: Vec<u8>,
}

impl Commands {
    pub(super) fn parse(reply: &[u8]) -> Result<Self> {
        let mut r = ok_reply(reply, "the handshake")?;
        Ok(Self {
            bitmap: r.rest().to_vec(),
        })
    }

    pub(super) fn allows(&self, cmd: u16) -> bool {
        match gate(cmd) {
            Some(Gate::Always) => true,
            Some(Gate::Bit(n)) => self
                .bitmap
                .get(usize::from(n / 8))
                .is_some_and(|byte| byte & (1 << (n % 8)) != 0),
            None => false,
        }
    }
}

/// A reply whose status byte is `00`, positioned after it. Spec: realme_t310.md#seq
fn ok_reply<'a>(reply: &'a [u8], what: &'static str) -> Result<Reader<'a>> {
    let mut r = Reader::new(reply, what);
    match r.u8()? {
        0x00 => Ok(r),
        status => Err(Error::Rejected {
            what: what.to_owned(),
            status,
        }),
    }
}

/// Spec: realme_t310.md#identity
pub(super) fn decode_product_id(reply: &[u8]) -> Result<u32> {
    let mut r = ok_reply(reply, "the product id query")?;
    let [low, middle, high] = [r.u8()?, r.u8()?, r.u8()?];
    Ok(u32::from_le_bytes([low, middle, high, 0]))
}

/// Spec: realme_t310.md#events-offered
pub(super) fn decode_events_offered(reply: &[u8]) -> Result<Vec<u8>> {
    let mut r = ok_reply(reply, "the event list")?;
    let count = r.u8()?;
    Ok(r.take(usize::from(count))?.to_vec())
}

/// The subscribe payload: count, then the wanted codes the buds offer.
/// Spec: realme_t310.md#subscribe
pub(super) fn subscribe_payload(offered: &[u8], commands: &Commands) -> Vec<u8> {
    let codes: Vec<u8> = EVENTS
        .iter()
        .filter(|(code, needs)| offered.contains(code) && needs.is_none_or(|cmd| commands.allows(cmd)))
        .map(|(code, _)| *code)
        .collect();
    let mut payload = vec![u8::try_from(codes.len()).unwrap_or_default()];
    payload.extend(codes);
    payload
}

/// Spec: realme_t310.md#battery
pub(super) fn decode_battery(reply: &[u8]) -> Result<Battery> {
    let mut r = ok_reply(reply, "the battery reply")?;
    let mut battery = Battery::default();
    for _ in 0..r.u8()? {
        let part = r.u8()?;
        let raw = r.u8()?;
        let level = raw & 0x7F;
        if level == 0 {
            continue; // not known
        }
        let charge = Some(Charge {
            level,
            charging: raw & 0x80 != 0,
        });
        match part {
            0x01 => battery.left = charge,
            0x02 => battery.right = charge,
            0x03 => battery.case = charge,
            _ => {}
        }
    }
    Ok(battery)
}

/// The versions of kind 2: plain numbers are joined with dots; full dotted versions (the T310's)
/// are shown once each, joined with ` / `. The raw text when there are none.
/// Spec: realme_t310.md#firmware
pub(super) fn decode_firmware(reply: &[u8]) -> Result<String> {
    let mut r = ok_reply(reply, "the firmware reply")?;
    let _count = r.u8()?;
    let text = String::from_utf8_lossy(r.rest()).into_owned();
    let fields: Vec<&str> = text.split(',').map(str::trim).collect();
    let mut versions: Vec<&str> = fields
        .chunks(3)
        .filter_map(|triple| match triple {
            [_device, "2", version] => Some(*version),
            _ => None,
        })
        .collect();
    if versions.is_empty() {
        return Ok(text);
    }
    if !versions.iter().any(|v| v.contains('.')) {
        return Ok(versions.join("."));
    }
    let mut seen = Vec::new();
    versions.retain(|v| {
        let new = !seen.contains(v);
        seen.push(*v);
        new
    });
    Ok(versions.join(" / "))
}

/// Spec: realme_t310.md#codec
pub(super) fn decode_codec(reply: &[u8]) -> Result<String> {
    let codec = ok_reply(reply, "the codec reply")?.u8()?;
    Ok(name_of(&CODECS, codec).map_or_else(|| format!("unknown (0x{codec:02X})"), str::to_owned))
}

/// Spec: realme_t310.md#anc-read
pub(super) fn decode_noise_mode(reply: &[u8]) -> Result<NoiseControl> {
    let mask = noise_mask(reply, NOISE_MODE_QUERY, "the noise-control reply")?;
    Ok(match NOISE_MODES.iter().find(|(m, _, _)| *m == mask) {
        Some((_, mode, level)) => NoiseControl {
            mode: (*mode).to_owned(),
            level: level.map(str::to_owned),
        },
        None => NoiseControl {
            mode: format!("unknown (0x{mask:02X})"),
            level: None,
        },
    })
}

/// The mask that selects `mode`, as written and as read back. Spec: realme_t310.md#anc-write
pub(super) fn noise_write_mask(mode: NoiseMode) -> u32 {
    match mode {
        NoiseMode::Off => 0x01,
        NoiseMode::Transparency => 0x02,
        NoiseMode::Anc(AncLevel::Light) => 0x04,
        NoiseMode::Anc(AncLevel::Medium) => 0x10,
        // Also plain "noise cancelling on".
        NoiseMode::Anc(AncLevel::Deep) => 0x08,
    }
}

/// `01 01 <mask>`: the noise-mode kind, then the mask in the fewest bytes (all fit in one).
/// Spec: realme_t310.md#anc-write
pub(super) fn noise_write_payload(mode: NoiseMode) -> Vec<u8> {
    let mut payload = NOISE_MODE_QUERY.to_vec();
    payload.extend(fewest_bytes(noise_write_mask(mode)));
    payload
}

/// A mask in the fewest little-endian bytes that hold it, at least one.
/// Spec: realme_t310.md#anc-write, #hold-cycle-write
fn fewest_bytes(mask: u32) -> Vec<u8> {
    let bytes = mask.to_le_bytes();
    let used = bytes.iter().rposition(|&b| b != 0).map_or(1, |last| last + 1);
    bytes.iter().take(used).copied().collect()
}

/// The mask in a noise-control read reply. Spec: realme_t310.md#anc-read
pub(super) fn decode_noise_mask(reply: &[u8]) -> Result<u32> {
    noise_mask(reply, NOISE_MODE_QUERY, "the noise-control reply")
}

/// A write reply: `00` means accepted. Spec: realme_t310.md#writes
pub(super) fn check_write_reply(reply: &[u8], what: &'static str) -> Result<()> {
    ok_reply(reply, what).map(|_| ())
}

/// Spec: realme_t310.md#hold-cycle
pub(super) fn decode_hold_cycle(reply: &[u8]) -> Result<Vec<String>> {
    let mask = decode_hold_cycle_mask(reply)?;
    let mut modes: Vec<String> = CYCLE_MODES
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .map(|(_, name)| (*name).to_owned())
        .collect();
    let known = CYCLE_MODES.iter().fold(0, |all, (bit, _)| all | bit);
    if mask & !known != 0 {
        modes.push(format!("unknown (0x{:02X})", mask & !known));
    }
    Ok(modes)
}

/// The mask for a hold cycle through `modes` (`off`, `transparency`, `anc`): at least two,
/// none twice. The buds keep their own order, so the order given does not matter.
/// Spec: realme_t310.md#hold-cycle-write
pub(super) fn hold_cycle_mask(modes: &[&str]) -> Result<u32> {
    let mut mask = 0;
    for &mode in modes {
        let Some(&(bit, _)) = CYCLE_MODES.iter().find(|(_, name)| *name == mode) else {
            let names: Vec<&str> = CYCLE_MODES.iter().map(|(_, name)| *name).collect();
            return Err(Error::Usage(format!(
                "no mode called {mode} in the hold cycle; use: {}",
                names.join(", ")
            )));
        };
        if mask & bit != 0 {
            return Err(Error::Usage(format!("{mode} is in the hold cycle twice")));
        }
        mask |= bit;
    }
    if mask.count_ones() < 2 {
        return Err(Error::Usage("the hold cycle needs at least two modes".to_owned()));
    }
    Ok(mask)
}

/// `02 01 <mask>`, the mask in the fewest bytes. Spec: realme_t310.md#hold-cycle-write
pub(super) fn hold_cycle_write_payload(mask: u32) -> Vec<u8> {
    let mut payload = HOLD_CYCLE_QUERY.to_vec();
    payload.extend(fewest_bytes(mask));
    payload
}

/// The mask in a hold-cycle read reply. Spec: realme_t310.md#hold-cycle
pub(super) fn decode_hold_cycle_mask(reply: &[u8]) -> Result<u32> {
    noise_mask(reply, HOLD_CYCLE_QUERY, "the hold-cycle reply")
}

/// `00 <echo of the query> <mask, 1-4 bytes LE>`. Spec: realme_t310.md#anc-read
fn noise_mask(reply: &[u8], query: [u8; 2], what: &'static str) -> Result<u32> {
    let mut r = ok_reply(reply, what)?;
    if [r.u8()?, r.u8()?] != query {
        return Err(Error::Protocol(format!("{what} answers a different question")));
    }
    let bytes = r.rest();
    if bytes.is_empty() || bytes.len() > 4 {
        return Err(Error::Protocol(format!("{what} has a {} byte value", bytes.len())));
    }
    Ok(bytes.iter().rev().fold(0, |mask, &b| (mask << 8) | u32::from(b)))
}

/// Spec: realme_t310.md#switches
pub(super) fn decode_switches(reply: &[u8]) -> Result<Vec<Switch>> {
    let mut r = ok_reply(reply, "the switch reply")?;
    let mut switches = Vec::new();
    for _ in 0..r.u8()? {
        let id = r.u8()?;
        let value = r.u8()?;
        if let Some((_, key, label)) = SWITCH_NAMES.iter().find(|(known, _, _)| *known == id) {
            switches.push(Switch {
                key,
                label,
                on: value == 0x01,
            });
        }
    }
    Ok(switches)
}

/// The id of a switch, by its key (`game`…). Spec: realme_t310.md#switches
pub(super) fn switch_id(key: &str) -> Option<u8> {
    SWITCH_NAMES.iter().find(|(_, k, _)| *k == key).map(|(id, _, _)| *id)
}

/// `<id> <01|00>`. Spec: realme_t310.md#switch-write
pub(super) fn switch_write_payload(id: u8, on: bool) -> [u8; 2] {
    [id, u8::from(on)]
}

/// The id of a built-in EQ preset, by name. Spec: realme_t310.md#eq-read
pub(super) fn eq_id(name: &str) -> Option<u8> {
    EQ_PRESETS.iter().find(|(_, n)| *n == name).map(|(id, _)| *id)
}

/// The built-in EQ presets' names, in id order. Spec: realme_t310.md#eq-read
pub(super) fn eq_names() -> Vec<&'static str> {
    EQ_PRESETS.iter().map(|(_, name)| *name).collect()
}

/// Spec: realme_t310.md#eq-read
pub(super) fn decode_eq(reply: &[u8]) -> Result<String> {
    let id = ok_reply(reply, "the EQ reply")?.u8()?;
    Ok(name_of(&EQ_PRESETS, id).map_or_else(|| format!("custom-{id}"), str::to_owned))
}

/// The built-in EQ presets, in id order. Spec: realme_t310.md#eq-read
pub(super) fn builtin_eqs() -> Vec<EqPreset> {
    EQ_PRESETS
        .iter()
        .map(|&(id, name)| EqPreset {
            name: name.to_owned(),
            id,
            custom: false,
            bands: Vec::new(),
            range: None,
        })
        .collect()
}

/// One custom preset as the list holds it. Spec: realme_t310.md#custom-eq
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CustomEq {
    pub(super) selected: bool,
    /// Gain range, dB.
    pub(super) min: i8,
    pub(super) max: i8,
    pub(super) id: u8,
    /// The stored name, byte for byte, so a save writes it back unchanged.
    pub(super) name: Vec<u8>,
    pub(super) bands: Vec<EqBand>,
}

impl CustomEq {
    pub(super) fn preset(&self) -> EqPreset {
        EqPreset {
            name: String::from_utf8_lossy(&self.name).into_owned(),
            id: self.id,
            custom: true,
            bands: self.bands.clone(),
            range: Some((self.min, self.max)),
        }
    }
}

/// `00 <count>`, then per preset `<selected> <min> <max> <id> <nameLen> <name> <bands>
/// (<freq u16 LE> <gain s8>)…`. Spec: realme_t310.md#custom-eq
pub(super) fn decode_custom_eq_list(reply: &[u8]) -> Result<Vec<CustomEq>> {
    let mut r = ok_reply(reply, "the custom EQ list")?;
    let mut presets = Vec::new();
    for _ in 0..r.u8()? {
        let selected = r.u8()? == 0x01;
        let (min, max, id) = (r.i8()?, r.i8()?, r.u8()?);
        let name_len = r.u8()?;
        let name = r.take(usize::from(name_len))?.to_vec();
        let mut bands = Vec::new();
        for _ in 0..r.u8()? {
            bands.push(EqBand {
                hz: r.u16_le()?,
                gain: r.i8()?,
            });
        }
        presets.push(CustomEq {
            selected,
            min,
            max,
            id,
            name,
            bands,
        });
    }
    Ok(presets)
}

/// Spec: realme_t310.md#custom-eq
pub(super) fn decode_custom_eqs(reply: &[u8]) -> Result<Vec<EqPreset>> {
    Ok(decode_custom_eq_list(reply)?.iter().map(CustomEq::preset).collect())
}

/// The id in `custom-<id>`, the name [`decode_eq`] gives a custom preset.
/// Spec: realme_t310.md#eq-read
pub(super) fn custom_eq_id(name: &str) -> Option<u8> {
    name.strip_prefix("custom-")?.parse().ok()
}

/// The preset with the given bands' gains changed, and nothing else. Every band must be one the
/// preset has, at most once, with a gain inside the preset's range.
/// Spec: realme_t310.md#custom-eq
pub(super) fn retune(preset: &CustomEq, changes: &[EqBand]) -> Result<CustomEq> {
    let mut tuned = preset.clone();
    for (i, change) in changes.iter().enumerate() {
        if changes.iter().take(i).any(|earlier| earlier.hz == change.hz) {
            return Err(Error::Usage(format!("the {} Hz band is given twice", change.hz)));
        }
        let Some(band) = tuned.bands.iter_mut().find(|b| b.hz == change.hz) else {
            let have: Vec<String> = preset.bands.iter().map(|b| b.hz.to_string()).collect();
            return Err(Error::Usage(format!(
                "custom-{} has no {} Hz band; its bands are {} Hz",
                preset.id,
                change.hz,
                have.join(", ")
            )));
        };
        if !(preset.min..=preset.max).contains(&change.gain) {
            return Err(Error::Usage(format!(
                "a gain of {:+} dB is outside {:+}..{:+} dB",
                change.gain, preset.min, preset.max
            )));
        }
        band.gain = change.gain;
    }
    Ok(tuned)
}

/// `02 <min> <max> <id> <nameLen> <name> <bands> (<freq u16 LE> <gain s8>)…`: save the whole
/// preset and select it. Spec: realme_t310.md#custom-eq
pub(super) fn custom_eq_save_payload(preset: &CustomEq) -> Result<Vec<u8>> {
    let too_long = |what: &str| Error::Protocol(format!("the preset's {what} does not fit in a byte"));
    let mut payload = vec![CUSTOM_EQ_SAVE];
    payload.extend([preset.min.to_le_bytes()[0], preset.max.to_le_bytes()[0], preset.id]);
    payload.push(u8::try_from(preset.name.len()).map_err(|_| too_long("name"))?);
    payload.extend(&preset.name);
    payload.push(u8::try_from(preset.bands.len()).map_err(|_| too_long("band count"))?);
    for band in &preset.bands {
        payload.extend(band.hz.to_le_bytes());
        payload.push(band.gain.to_le_bytes()[0]);
    }
    Ok(payload)
}

/// One gesture-table entry: `<dev> <btn> <act> <fn>`. Spec: realme_t310.md#gestures
pub(super) type GestureEntry = [u8; 4];

/// The entries of a gesture-table reply, in the order the buds sent them.
/// Spec: realme_t310.md#gestures
pub(super) fn decode_gesture_entries(reply: &[u8]) -> Result<Vec<GestureEntry>> {
    let mut r = ok_reply(reply, "the gesture table")?;
    let mut entries = Vec::new();
    for _ in 0..r.u8()? {
        entries.push([r.u8()?, r.u8()?, r.u8()?, r.u8()?]);
    }
    Ok(entries)
}

/// Spec: realme_t310.md#gestures
pub(super) fn name_gesture(&[side, group, act, function]: &GestureEntry) -> Gesture {
    Gesture {
        side: name_of(&SIDES, side).map_or_else(|| format!("device-0x{side:02X}"), str::to_owned),
        gesture: name_of(&ACTS, act).map_or_else(|| format!("act-0x{act:02X}"), str::to_owned),
        action: name_of(&FUNCTIONS, function).map_or_else(|| format!("fn-0x{function:02X}"), str::to_owned),
        group: (group != MAIN_GROUP).then_some(group),
    }
}

/// Spec: realme_t310.md#gestures
pub(super) fn decode_gestures(reply: &[u8]) -> Result<Vec<Gesture>> {
    Ok(decode_gesture_entries(reply)?.iter().map(name_gesture).collect())
}

/// What `touch <side>` changes: `both` is the left and the right bud's entry.
/// Spec: realme_t310.md#gesture-choices
const TOUCH_SIDES: [(&str, &[u8]); 3] = [("left", &[0x01]), ("right", &[0x02]), ("both", &[0x01, 0x02])];

/// The actions realme Link offers per gesture on the T310, in its order. budctl offers no
/// others. Spec: realme_t310.md#gesture-choices
const TOUCH_CHOICES: [(u8, &[u8]); 3] = [
    // double tap: none, play/pause, next, voice assistant
    (0x02, &[0x00, 0x01, 0x06, 0x03]),
    // triple tap: none, previous, next, voice assistant, volume up, volume down, game mode
    (0x03, &[0x00, 0x05, 0x06, 0x03, 0x0B, 0x0C, 0x11]),
    // hold: none, noise-control cycle, voice assistant, game mode
    (0x04, &[0x00, 0x08, 0x03, 0x11]),
];

/// The actions each gesture can be given, by name, in realme Link's order.
/// Spec: realme_t310.md#gesture-choices
pub(super) fn touch_choices() -> Vec<GestureChoice> {
    TOUCH_CHOICES
        .iter()
        .filter_map(|&(act, functions)| {
            Some(GestureChoice {
                gesture: name_of(&ACTS, act)?.to_owned(),
                actions: functions
                    .iter()
                    .filter_map(|&f| name_of(&FUNCTIONS, f).map(str::to_owned))
                    .collect(),
            })
        })
        .collect()
}

/// The modes a hold cycle can include, by name. Spec: realme_t310.md#hold-cycle-write
pub(super) fn cycle_modes() -> Vec<String> {
    CYCLE_MODES.iter().map(|&(_, name)| name.to_owned()).collect()
}

/// A checked `touch <side> <gesture> <action>`: the bud ids, the act and the fn to write.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct TouchTarget {
    pub(super) devices: &'static [u8],
    pub(super) act: u8,
    pub(super) function: u8,
}

/// Check the names against what the T310 offers. Every failure is a usage error that lists
/// the valid names. Spec: realme_t310.md#gesture-choices
pub(super) fn touch_target(side: &str, gesture: &str, action: &str) -> Result<TouchTarget> {
    let list = |names: Vec<&str>| names.join(", ");
    let Some(&(_, devices)) = TOUCH_SIDES.iter().find(|(name, _)| *name == side) else {
        return Err(Error::Usage(format!(
            "no side called {side}; use one of: {}",
            list(TOUCH_SIDES.iter().map(|(name, _)| *name).collect())
        )));
    };
    let gesture_names = || {
        list(
            TOUCH_CHOICES
                .iter()
                .filter_map(|(act, _)| name_of(&ACTS, *act))
                .collect(),
        )
    };
    let Some(&(act, choices)) = TOUCH_CHOICES
        .iter()
        .find(|(act, _)| name_of(&ACTS, *act) == Some(gesture))
    else {
        return Err(Error::Usage(format!(
            "these buds have no {gesture} gesture to set; use one of: {}",
            gesture_names()
        )));
    };
    let Some(&function) = choices.iter().find(|f| name_of(&FUNCTIONS, **f) == Some(action)) else {
        return Err(Error::Usage(format!(
            "{gesture} cannot be set to {action} on these buds; use one of: {}",
            list(choices.iter().filter_map(|f| name_of(&FUNCTIONS, *f)).collect())
        )));
    };
    Ok(TouchTarget { devices, act, function })
}

/// The table with the target's entries changed, and nothing else. Every targeted entry must
/// already be in the table: budctl never adds entries. Spec: realme_t310.md#gesture-write
pub(super) fn retarget_gestures(table: &[GestureEntry], target: &TouchTarget) -> Result<Vec<GestureEntry>> {
    for &device in target.devices {
        if !table
            .iter()
            .any(|&[d, group, act, _]| d == device && group == MAIN_GROUP && act == target.act)
        {
            let entry = name_gesture(&[device, MAIN_GROUP, target.act, 0]);
            return Err(Error::Unsupported(format!(
                "the buds report no {} {} entry, so budctl will not write one",
                entry.side, entry.gesture
            )));
        }
    }
    Ok(table
        .iter()
        .map(|&[device, group, act, function]| {
            if target.devices.contains(&device) && group == MAIN_GROUP && act == target.act {
                [device, group, act, target.function]
            } else {
                [device, group, act, function]
            }
        })
        .collect())
}

/// `<count> (<dev> <btn> <act> <fn>)…`: the whole table. Spec: realme_t310.md#gesture-write
pub(super) fn gesture_write_payload(table: &[GestureEntry]) -> Result<Vec<u8>> {
    let count = u8::try_from(table.len())
        .map_err(|_| Error::Protocol(format!("a gesture table of {} entries does not fit", table.len())))?;
    let mut payload = vec![count];
    payload.extend(table.iter().flatten());
    Ok(payload)
}

/// The handshake reply of the owner's T310 (HW1). Spec: realme_t310.md#bitmap-t310
#[cfg(test)]
pub(super) const T310_BITMAP_REPLY: &str = "00 BF 1F 5A 20 45 04 40 00";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::hex;

    #[test]
    fn t310_bitmap_allows_its_commands_and_nothing_else() {
        let commands = Commands::parse(&hex(T310_BITMAP_REPLY)).unwrap();
        // Every gated command in the T310's logged command set (realme_t310.md#bitmap-t310).
        for cmd in [
            0x0100, 0x0103, 0x0105, 0x0106, 0x0108, 0x0109, 0x010C, 0x010D, 0x010F, 0x0114, 0x0122, 0x0124, 0x0205,
            0x0400, 0x0401, 0x0403, 0x0404, 0x0405, 0x0406, 0x0418, 0x041B, 0x0200,
        ] {
            assert!(commands.allows(cmd), "0x{cmd:04X} should be allowed");
        }
        // Commands the T310 lacks, and commands budctl has no gate for.
        for cmd in [0x012F, 0x041E, 0x0413, 0x0130, 0x0427, 0x0423, 0x012A, 0x0422, 0x9999] {
            assert!(!commands.allows(cmd), "0x{cmd:04X} should be refused");
        }
    }

    #[test]
    fn a_short_bitmap_allows_only_the_always_commands() {
        let commands = Commands::parse(&hex("00")).unwrap();
        assert!(commands.allows(BATTERY) && commands.allows(SWITCHES) && commands.allows(HANDSHAKE));
        assert!(!commands.allows(FIRMWARE) && !commands.allows(NOISE_CONTROL));
        assert!(matches!(
            Commands::parse(&hex("01 FF")),
            Err(Error::Rejected { status: 1, .. })
        ));
    }

    #[test]
    fn product_id_is_three_bytes_little_endian() {
        assert_eq!(decode_product_id(&hex("00 12 3C 06")).unwrap(), T310_PRODUCT_ID);
        assert_eq!(decode_product_id(&hex("00 14 54 06")).unwrap(), 0x06_54_14);
        assert!(decode_product_id(&hex("00 12 3C")).is_err());
    }

    #[test]
    fn subscribes_to_offered_events_only() {
        let commands = Commands::parse(&hex(T310_BITMAP_REPLY)).unwrap();
        let buds4_offer = hex("00 09 01 02 03 04 08 0B F1 F2 F3");
        let offered = decode_events_offered(&buds4_offer).unwrap();
        assert_eq!(subscribe_payload(&offered, &commands), hex("04 01 02 03 04"));
        assert_eq!(subscribe_payload(&[0x02, 0x01], &commands), hex("02 01 02"));
        let no_fit_test = Commands::parse(&hex("00 BD 0D")).unwrap();
        assert_eq!(subscribe_payload(&offered, &no_fit_test), hex("03 01 02 03"));
    }

    #[test]
    fn battery_levels_charging_and_unknown_parts() {
        let battery = decode_battery(&hex("00 03 01 64 02 E4 03 00")).unwrap();
        assert_eq!(
            battery.left,
            Some(Charge {
                level: 100,
                charging: false
            })
        );
        assert_eq!(
            battery.right,
            Some(Charge {
                level: 100,
                charging: true
            })
        );
        assert_eq!(battery.case, None);
        assert_eq!(decode_battery(&hex("00 00")).unwrap(), Battery::default());
        assert!(decode_battery(&hex("00 03 01 64")).is_err());
    }

    #[test]
    fn firmware_shows_kind_two_versions() {
        let mut reply = hex("00 04");
        reply.extend(b"1,2,138,2,2,138,3,1,01,3,2,105");
        assert_eq!(decode_firmware(&reply).unwrap(), "138.138.105");
        // The owner's T310 (HW1): both buds report the same full version.
        let mut t310 = hex("00 02");
        t310.extend(b"1,2,1.1.0.43,2,2,1.1.0.43");
        assert_eq!(decode_firmware(&t310).unwrap(), "1.1.0.43");
        let mut differ = hex("00 02");
        differ.extend(b"1,2,1.1.0.43,2,2,1.1.0.44");
        assert_eq!(decode_firmware(&differ).unwrap(), "1.1.0.43 / 1.1.0.44");
        let mut odd = hex("00 01");
        odd.extend(b"V1.0.2");
        assert_eq!(decode_firmware(&odd).unwrap(), "V1.0.2");
    }

    #[test]
    fn noise_control_modes() {
        let mode = |reply: &str| decode_noise_mode(&hex(reply)).unwrap();
        assert_eq!(mode("00 01 01 01").mode, "off");
        assert_eq!(mode("00 01 01 02 00").mode, "transparency");
        let deep = mode("00 01 01 08");
        assert_eq!((deep.mode.as_str(), deep.level.as_deref()), ("anc", Some("deep")));
        assert_eq!(mode("00 01 01 10").level.as_deref(), Some("medium"));
        assert_eq!(mode("00 01 01 00 08").mode, "unknown (0x800)");
        assert!(decode_noise_mode(&hex("00 02 01 08")).is_err());
        assert!(decode_noise_mode(&hex("00 01 01")).is_err());
        assert!(matches!(
            decode_noise_mode(&hex("0E 01 01 08")),
            Err(Error::Rejected { status: 0x0E, .. })
        ));
    }

    #[test]
    fn noise_control_write_payloads() {
        assert_eq!(noise_write_payload(NoiseMode::Off), hex("01 01 01"));
        assert_eq!(noise_write_payload(NoiseMode::Transparency), hex("01 01 02"));
        assert_eq!(noise_write_payload(NoiseMode::Anc(AncLevel::Light)), hex("01 01 04"));
        assert_eq!(noise_write_payload(NoiseMode::Anc(AncLevel::Medium)), hex("01 01 10"));
        assert_eq!(noise_write_payload(NoiseMode::Anc(AncLevel::Deep)), hex("01 01 08"));
        // What is written is what a read reports: every write mask decodes to its own mode.
        for level in [AncLevel::Light, AncLevel::Medium, AncLevel::Deep] {
            let mut reply = hex("00 01 01");
            reply.extend(noise_write_payload(NoiseMode::Anc(level)).iter().skip(2));
            let read = decode_noise_mode(&reply).unwrap();
            assert_eq!((read.mode.as_str(), read.level.as_deref()), ("anc", Some(level.name())));
        }
        // The family's documented frame for "off" (realme_t310.md#frame-whole).
        let frame =
            crate::devices::realme_t310::frame::encode(NOISE_CONTROL_WRITE, 0x06, &noise_write_payload(NoiseMode::Off))
                .unwrap();
        assert_eq!(frame, hex("AA 0A 00 00 04 04 06 03 00 01 01 01"));
    }

    #[test]
    fn switch_writes() {
        assert_eq!(switch_id("game"), Some(0x06));
        assert_eq!(switch_id("dual"), Some(0x11));
        assert_eq!(switch_id("turbo"), None);
        assert_eq!(switch_write_payload(0x06, true), [0x06, 0x01]);
        assert_eq!(switch_write_payload(0x06, false), [0x06, 0x00]);
    }

    #[test]
    fn eq_presets_by_name() {
        assert_eq!(eq_id("pure-raw"), Some(0x00));
        assert_eq!(eq_id("pure-bass"), Some(0x03));
        assert_eq!(eq_id("custom-7"), None);
        assert_eq!(eq_names(), ["pure-raw", "deep-bass", "pure-vocals", "pure-bass"]);
        // Every preset reads back under the name it was selected by.
        for name in eq_names() {
            let reply = [0x00, eq_id(name).unwrap()];
            assert_eq!(decode_eq(&reply).unwrap(), name);
        }
    }

    #[test]
    fn find_payloads() {
        assert_eq!(find_payload(true), [0x01]);
        assert_eq!(find_payload(false), [0x00]);
    }

    #[test]
    fn write_replies() {
        assert!(check_write_reply(&hex("00"), "the write").is_ok());
        assert!(matches!(
            check_write_reply(&hex("0E"), "the write"),
            Err(Error::Rejected { status: 0x0E, .. })
        ));
        assert!(check_write_reply(&[], "the write").is_err());
    }

    #[test]
    fn hold_cycle_lists_modes() {
        assert_eq!(decode_hold_cycle(&hex("00 02 01 0A")).unwrap(), ["transparency", "anc"]);
        assert_eq!(
            decode_hold_cycle(&hex("00 02 01 0B 00")).unwrap(),
            ["off", "transparency", "anc"]
        );
        assert_eq!(decode_hold_cycle(&hex("00 02 01 14")).unwrap(), ["unknown (0x14)"]);
    }

    #[test]
    fn switches_from_report_22_values() {
        let switches = decode_switches(&hex("00 06 05 00 06 00 11 01 1A 00 1B 01 1D 01")).unwrap();
        let on: Vec<(&str, bool)> = switches.iter().map(|s| (s.key, s.on)).collect();
        assert_eq!(
            on,
            [
                ("game", false),
                ("dual", true),
                ("wind", false),
                ("spatial", true),
                ("bass-boost", true)
            ]
        );
        assert!(decode_switches(&hex("00 02 06 01")).is_err());
    }

    #[test]
    fn eq_and_codec() {
        assert_eq!(decode_eq(&hex("00 00")).unwrap(), "pure-raw");
        assert_eq!(decode_eq(&hex("00 03")).unwrap(), "pure-bass");
        assert_eq!(decode_eq(&hex("00 05")).unwrap(), "custom-5");
        assert_eq!(decode_codec(&hex("00 02")).unwrap(), "AAC");
        assert_eq!(decode_codec(&hex("00 09")).unwrap(), "unknown (0x09)");
    }

    #[test]
    fn gesture_table_names_known_values() {
        let gestures = decode_gestures(&hex("00 03 01 01 02 01 02 01 04 08 04 06 02 1D")).unwrap();
        let rows: Vec<(&str, &str, &str, Option<u8>)> = gestures
            .iter()
            .map(|g| (g.side.as_str(), g.gesture.as_str(), g.action.as_str(), g.group))
            .collect();
        assert_eq!(
            rows,
            [
                ("left", "double-tap", "play-pause", None),
                ("right", "hold", "noise-cycle", None),
                ("both", "double-tap", "answer-call", Some(6)),
            ]
        );
        let odd = decode_gestures(&hex("00 01 09 01 07 42")).unwrap();
        assert_eq!(odd.first().map(|g| g.action.as_str()), Some("fn-0x42"));
        assert!(decode_gestures(&hex("00 02 01 01 02 01")).is_err());
    }

    /// The owner's table (HW1): per bud, double tap play/pause, triple tap next, hold cycle.
    const HW1_GESTURES: &str = "00 06 01 01 02 01 01 01 03 06 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04 08";

    #[test]
    fn touch_targets_follow_the_t310_choices() {
        assert_eq!(
            touch_target("left", "triple-tap", "volume-up").unwrap(),
            TouchTarget {
                devices: &[0x01],
                act: 0x03,
                function: 0x0B
            }
        );
        assert_eq!(touch_target("both", "hold", "none").unwrap().devices, [0x01, 0x02]);
        let usage = |side, gesture, action| match touch_target(side, gesture, action) {
            Err(Error::Usage(message)) => message,
            other => panic!("expected a usage error, got {other:?}"),
        };
        assert!(usage("middle", "hold", "none").contains("left, right, both"));
        assert!(usage("left", "single-tap", "none").contains("double-tap, triple-tap, hold"));
        // Offered by the family, but not by realme Link for this gesture.
        assert!(usage("left", "double-tap", "volume-up").contains("none, play-pause, next, voice-assistant"));
        assert!(usage("left", "hold", "previous").contains("none, noise-cycle, voice-assistant, game-mode"));
    }

    #[test]
    fn retargeting_changes_only_the_targeted_entries() {
        let table = decode_gesture_entries(&hex(HW1_GESTURES)).unwrap();
        let left = touch_target("left", "triple-tap", "volume-up").unwrap();
        let payload = gesture_write_payload(&retarget_gestures(&table, &left).unwrap()).unwrap();
        assert_eq!(
            payload,
            hex("06 01 01 02 01 01 01 03 0B 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04 08")
        );
        // The frame the spec gives for this change (realme_t310.md#gesture-write).
        let frame = crate::devices::realme_t310::frame::encode(GESTURE_WRITE, 0x06, &payload).unwrap();
        assert_eq!(
            crate::debug::hex(&frame),
            "AA 20 00 00 01 04 06 19 00 06 01 01 02 01 01 01 03 0B 01 01 04 08 02 01 02 01 02 01 03 06 02 01 04 08"
        );
        let both = touch_target("both", "hold", "game-mode").unwrap();
        let payload = gesture_write_payload(&retarget_gestures(&table, &both).unwrap()).unwrap();
        assert_eq!(
            payload,
            hex("06 01 01 02 01 01 01 03 06 01 01 04 11 02 01 02 01 02 01 03 06 02 01 04 11")
        );
        // Unchanged, the payload is the reply without its status byte.
        let same = touch_target("right", "double-tap", "play-pause").unwrap();
        let payload = gesture_write_payload(&retarget_gestures(&table, &same).unwrap()).unwrap();
        assert_eq!(payload, hex(HW1_GESTURES).get(1..).unwrap());
    }

    #[test]
    fn entries_the_buds_did_not_report_are_never_written() {
        // Only the left bud's entries.
        let table = decode_gesture_entries(&hex("00 03 01 01 02 01 01 01 03 06 01 01 04 08")).unwrap();
        let both = touch_target("both", "hold", "none").unwrap();
        assert!(matches!(retarget_gestures(&table, &both), Err(Error::Unsupported(_))));
        // An on-call row (group 06) is not a main-group entry.
        let table = decode_gesture_entries(&hex("00 01 01 06 02 1D")).unwrap();
        let left = touch_target("left", "double-tap", "none").unwrap();
        assert!(matches!(retarget_gestures(&table, &left), Err(Error::Unsupported(_))));
    }

    #[test]
    fn hold_cycle_writes() {
        assert_eq!(hold_cycle_mask(&["transparency", "anc"]).unwrap(), 0x0A);
        assert_eq!(hold_cycle_mask(&["anc", "off", "transparency"]).unwrap(), 0x0B);
        assert_eq!(hold_cycle_write_payload(0x0B), hex("02 01 0B"));
        assert_eq!(hold_cycle_write_payload(0x09), hex("02 01 09"));
        let usage = |modes: &[&str]| matches!(hold_cycle_mask(modes), Err(Error::Usage(_)));
        assert!(usage(&["anc"]));
        assert!(usage(&[]));
        assert!(usage(&["anc", "anc"]));
        assert!(usage(&["anc", "deep"]));
        // What is written reads back as the same modes.
        let mut reply = hex("00");
        reply.extend(hold_cycle_write_payload(0x0B));
        assert_eq!(decode_hold_cycle_mask(&reply).unwrap(), 0x0B);
        assert_eq!(decode_hold_cycle(&reply).unwrap(), ["off", "transparency", "anc"]);
        // The frame the spec gives (realme_t310.md#hold-cycle-write).
        let frame =
            crate::devices::realme_t310::frame::encode(NOISE_CONTROL_WRITE, 0x06, &hold_cycle_write_payload(0x0B))
                .unwrap();
        assert_eq!(crate::debug::hex(&frame), "AA 0A 00 00 04 04 06 03 00 02 01 0B");
    }

    #[test]
    fn custom_eq_list() {
        assert!(decode_custom_eqs(&hex("00 00")).unwrap().is_empty());
        // One preset "Me", selected, range FA 06, two bands: 62 Hz +3, 16 kHz -2.
        let reply = hex("00 01 01 FA 06 04 02 4D 65 02 3E 00 03 80 3E FE");
        let presets = decode_custom_eqs(&reply).unwrap();
        assert_eq!(
            presets,
            [EqPreset {
                name: "Me".to_owned(),
                id: 0x04,
                custom: true,
                bands: vec![EqBand { hz: 62, gain: 3 }, EqBand { hz: 16000, gain: -2 }],
                range: Some((-6, 6)),
            }]
        );
        // Cut short inside the bands.
        assert!(decode_custom_eqs(&hex("00 01 01 FA 06 04 02 4D 65 02 3E 00")).is_err());
        assert_eq!(builtin_eqs().len(), 4);
    }

    #[test]
    fn bass_level_payloads() {
        assert_eq!(bass_write_payload(0).unwrap(), [0xFB, 0x05, 0x00]);
        assert_eq!(bass_write_payload(5).unwrap(), [0xFB, 0x05, 0x05]);
        assert_eq!(bass_write_payload(-2).unwrap(), [0xFB, 0x05, 0xFE]);
        assert_eq!(bass_write_payload(-5).unwrap(), [0xFB, 0x05, 0xFB]);
        assert!(matches!(bass_write_payload(6), Err(Error::Usage(_))));
        assert!(matches!(bass_write_payload(-6), Err(Error::Usage(_))));
        // The frame the spec gives (realme_t310.md#bass-write).
        let frame = crate::devices::realme_t310::frame::encode(BASS_LEVEL_WRITE, 0x05, &bass_write_payload(3).unwrap())
            .unwrap();
        assert_eq!(crate::debug::hex(&frame), "AA 0A 00 00 1B 04 05 03 00 FB 05 03");
    }

    /// The owner's custom EQ list (W11a): "Custom1", id 04, 6 flat bands from 50 Hz.
    const OWNER_CUSTOM_EQS: &str =
        "00 01 01 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00";

    #[test]
    fn retunes_the_owners_custom_preset() {
        let list = decode_custom_eq_list(&hex(OWNER_CUSTOM_EQS)).unwrap();
        let custom1 = list.first().unwrap();
        assert!(custom1.selected);
        assert_eq!((custom1.min, custom1.max, custom1.id), (-6, 6, 4));
        assert_eq!(custom_eq_id("custom-4"), Some(4));
        assert_eq!(custom_eq_id("pure-bass"), None);
        assert_eq!(custom_eq_id("custom-x"), None);

        // Unchanged, the save carries the preset as listed.
        assert_eq!(
            custom_eq_save_payload(&retune(custom1, &[]).unwrap()).unwrap(),
            hex("02 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 00 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E 00")
        );
        let tuned = retune(custom1, &[EqBand { hz: 50, gain: 3 }, EqBand { hz: 16000, gain: -2 }]).unwrap();
        let payload = custom_eq_save_payload(&tuned).unwrap();
        assert_eq!(
            payload,
            hex("02 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 03 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E FE")
        );
        // The frame the spec gives (realme_t310.md#custom-eq).
        let frame = crate::devices::realme_t310::frame::encode(CUSTOM_EQ_WRITE, 0x06, &payload).unwrap();
        assert_eq!(
            crate::debug::hex(&frame),
            "AA 26 00 00 18 04 06 1F 00 02 FA 06 04 07 43 75 73 74 6F 6D 31 06 32 00 03 FA 00 00 E8 03 00 A0 0F 00 40 1F 00 80 3E FE"
        );

        let usage = |changes: &[EqBand]| matches!(retune(custom1, changes), Err(Error::Usage(_)));
        assert!(usage(&[EqBand { hz: 62, gain: 1 }]));
        assert!(usage(&[EqBand { hz: 50, gain: 7 }]));
        assert!(usage(&[EqBand { hz: 50, gain: -7 }]));
        assert!(usage(&[EqBand { hz: 50, gain: 1 }, EqBand { hz: 50, gain: 2 }]));
    }
}
