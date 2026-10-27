//! realme Buds T310 commands, tables and pure decoders. No I/O here, so every function can be
//! tested with bytes from the spec. Each constant cites docs/protocol/realme_t310.md.

use crate::error::{Error, Result};
use crate::reader::Reader;
use crate::status::{AncLevel, Battery, Charge, Gesture, NoiseControl, NoiseMode, Switch};

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
/// Spec: realme_t310.md#eq-read
pub(super) const EQ_PRESET: u16 = 0x010F;
/// Spec: realme_t310.md#codec
pub(super) const CODEC: u16 = 0x0114;
/// Not sent: the T310 does not answer it (HW1). Kept for W6. Spec: realme_t310.md#bass-read
#[allow(dead_code)]
pub(super) const BASS_LEVEL: u16 = 0x0124;
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
    let mask = noise_write_mask(mode).to_le_bytes();
    let used = mask.iter().rposition(|&b| b != 0).map_or(1, |last| last + 1);
    payload.extend(mask.iter().take(used));
    payload
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
    let mask = noise_mask(reply, HOLD_CYCLE_QUERY, "the hold-cycle reply")?;
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

/// Spec: realme_t310.md#eq-read
pub(super) fn decode_eq(reply: &[u8]) -> Result<String> {
    let id = ok_reply(reply, "the EQ reply")?.u8()?;
    Ok(name_of(&EQ_PRESETS, id).map_or_else(|| format!("custom-{id}"), str::to_owned))
}

/// `00 <min> <max> <level>`, all signed. Not used until W6. Spec: realme_t310.md#bass-read
#[allow(dead_code)]
pub(super) fn decode_bass_level(reply: &[u8]) -> Result<i8> {
    let mut r = ok_reply(reply, "the bass-level reply")?;
    let (min, max, level) = (r.i8()?, r.i8()?, r.i8()?);
    if !(min..=max).contains(&level) {
        return Err(Error::Protocol(format!("bass level {level} outside {min}..={max}")));
    }
    Ok(level)
}

/// Spec: realme_t310.md#gestures
pub(super) fn decode_gestures(reply: &[u8]) -> Result<Vec<Gesture>> {
    let mut r = ok_reply(reply, "the gesture table")?;
    let mut gestures = Vec::new();
    for _ in 0..r.u8()? {
        let [side, group, act, function] = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
        gestures.push(Gesture {
            side: name_of(&SIDES, side).map_or_else(|| format!("device-0x{side:02X}"), str::to_owned),
            gesture: name_of(&ACTS, act).map_or_else(|| format!("act-0x{act:02X}"), str::to_owned),
            action: name_of(&FUNCTIONS, function).map_or_else(|| format!("fn-0x{function:02X}"), str::to_owned),
            group: (group != MAIN_GROUP).then_some(group),
        });
    }
    Ok(gestures)
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
    fn eq_codec_and_bass_level() {
        assert_eq!(decode_eq(&hex("00 00")).unwrap(), "pure-raw");
        assert_eq!(decode_eq(&hex("00 03")).unwrap(), "pure-bass");
        assert_eq!(decode_eq(&hex("00 05")).unwrap(), "custom-5");
        assert_eq!(decode_codec(&hex("00 02")).unwrap(), "AAC");
        assert_eq!(decode_codec(&hex("00 09")).unwrap(), "unknown (0x09)");
        assert_eq!(decode_bass_level(&hex("00 FB 05 FE")).unwrap(), -2);
        assert!(decode_bass_level(&hex("00 FB 05 07")).is_err());
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
}
