//! Framing of the OPPO/OnePlus/realme protocol family. Spec: realme_t310.md#frame
//!
//! Holds no T310 tables. If a second model of this family is added, this file moves to
//! `devices/oppo_family/` (CLAUDE.md, "Device plugin contract").

use crate::error::{Error, Result};
use crate::reader::Reader;

/// Spec: realme_t310.md#frame
const START: u8 = 0xAA;
/// Spec: realme_t310.md#frame
const WHOLE: u8 = 0x00;
/// What the T310 puts in the slice byte of frames it starts itself (requests and pushes). The
/// rest is laid out as a whole message. Spec: realme_t310.md#frame-from-buds
const WHOLE_FROM_BUDS: u8 = 0x08;
/// Spec: realme_t310.md#frame-slices
const FIRST_SLICE: u8 = 0x01;
/// Spec: realme_t310.md#frame-slices
const MIDDLE_SLICE: u8 = 0x02;
/// Spec: realme_t310.md#frame-slices
const LAST_SLICE: u8 = 0x03;
/// Bytes after the length in a whole frame, before the payload: slice, 00, cmd (2), seq,
/// paylen (2). Spec: realme_t310.md#frame-whole
const WHOLE_HEADER: usize = 7;
/// Shortest slice frame: slice, 00, fsn, at least one byte. Spec: realme_t310.md#frame-receive
const MIN_SLICE_LEN: usize = 4;
/// Spec: realme_t310.md#frame-receive
const MAX_LEN: usize = 4096;
/// The length field is at most 3 bytes. Spec: realme_t310.md#frame-receive
const MAX_LEN_BYTES: usize = 3;
/// Senders slice messages that do not fit in this many bytes. budctl never slices, so it
/// refuses to build a bigger frame. Spec: realme_t310.md#frame-slices
const FRAME_BUDGET: usize = 660;

/// One message: a command id, its sequence byte and its payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Frame {
    pub cmd: u16,
    pub seq: u8,
    pub payload: Vec<u8>,
}

/// One whole (unsliced) frame. Spec: realme_t310.md#frame-whole
pub(super) fn encode(cmd: u16, seq: u8, payload: &[u8]) -> Result<Vec<u8>> {
    let too_big = || Error::Protocol(format!("a {} byte payload does not fit in one frame", payload.len()));
    let paylen = u16::try_from(payload.len()).map_err(|_| too_big())?;
    let mut frame = vec![START];
    push_leb128(WHOLE_HEADER + payload.len(), &mut frame);
    frame.extend([WHOLE, 0x00]);
    frame.extend(cmd.to_le_bytes());
    frame.push(seq);
    frame.extend(paylen.to_le_bytes());
    frame.extend(payload);
    if frame.len() > FRAME_BUDGET {
        return Err(too_big());
    }
    Ok(frame)
}

/// 7 bits per byte, low bits first; the top bit says another byte follows.
fn push_leb128(mut value: usize, out: &mut Vec<u8>) {
    loop {
        let low = u8::try_from(value & 0x7F).unwrap_or_default();
        value >>= 7;
        if value == 0 {
            out.push(low);
            return;
        }
        out.push(low | 0x80);
    }
}

enum Length {
    /// The length and how many bytes it took.
    Value(usize, usize),
    /// More bytes are needed to finish the length.
    Incomplete,
    /// Not a valid length: more than three bytes.
    Invalid,
}

fn read_leb128(bytes: &[u8]) -> Length {
    let mut value = 0;
    for (i, &byte) in bytes.iter().take(MAX_LEN_BYTES).enumerate() {
        value |= usize::from(byte & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            return Length::Value(value, i + 1);
        }
    }
    if bytes.len() < MAX_LEN_BYTES {
        Length::Incomplete
    } else {
        Length::Invalid
    }
}

/// Turns the byte stream from the buds back into messages. Spec: realme_t310.md#frame-receive
#[derive(Default)]
pub(super) struct Reassembler {
    buffer: Vec<u8>,
    /// Parts of a sliced message received so far.
    sliced: Option<Vec<u8>>,
    /// Bytes thrown away while looking for a frame start (shown with `--debug`).
    discarded: usize,
}

impl Reassembler {
    pub(super) fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// How many bytes were discarded since the last call.
    pub(super) fn take_discarded(&mut self) -> usize {
        std::mem::take(&mut self.discarded)
    }

    /// The next complete message, if one is buffered.
    pub(super) fn next_frame(&mut self) -> Option<Frame> {
        loop {
            // Throw away everything before the next frame start.
            let start = self.buffer.iter().position(|&b| b == START);
            let skip = start.unwrap_or(self.buffer.len());
            self.discarded += skip;
            self.buffer.drain(..skip);
            if self.buffer.is_empty() {
                return None;
            }

            let (len, len_bytes) = match read_leb128(self.buffer.get(1..).unwrap_or_default()) {
                Length::Value(len, len_bytes) => (len, len_bytes),
                Length::Incomplete => return None,
                Length::Invalid => {
                    self.drop_start();
                    continue;
                }
            };
            let &slice = self.buffer.get(1 + len_bytes)?;
            let min_len = if is_whole(slice) { WHOLE_HEADER } else { MIN_SLICE_LEN };
            if len < min_len || len > MAX_LEN {
                self.drop_start();
                continue;
            }

            let total = 1 + len_bytes + len;
            if self.buffer.len() < total {
                return None;
            }
            let body: Vec<u8> = self.buffer.drain(..total).skip(1 + len_bytes).collect();
            if let Some(frame) = self.body(&body) {
                return Some(frame);
            }
        }
    }

    /// Drop a false frame start and search on.
    fn drop_start(&mut self) {
        self.buffer.drain(..1);
        self.discarded += 1;
    }

    /// Spec: realme_t310.md#frame, #frame-slices
    fn body(&mut self, body: &[u8]) -> Option<Frame> {
        let mut r = Reader::new(body, "frame");
        let slice = r.u8().ok()?;
        let _zero = r.u8().ok()?;
        if is_whole(slice) {
            return parse_message(r.rest());
        }
        let _slice_number = r.u8().ok()?;
        let part = r.rest();
        match slice {
            FIRST_SLICE => self.sliced = Some(part.to_vec()),
            MIDDLE_SLICE => {
                if let Some(joined) = &mut self.sliced {
                    joined.extend_from_slice(part);
                }
            }
            LAST_SLICE => {
                let mut joined = self.sliced.take()?;
                joined.extend_from_slice(part);
                return parse_message(&joined);
            }
            _ => {}
        }
        None
    }
}

/// Spec: realme_t310.md#frame-whole, #frame-from-buds
fn is_whole(slice: u8) -> bool {
    slice == WHOLE || slice == WHOLE_FROM_BUDS
}

/// `cmd(2, LE) seq paylen(2, LE) payload`. A payload shorter than `paylen` is kept as it is.
/// Spec: realme_t310.md#frame-receive
fn parse_message(bytes: &[u8]) -> Option<Frame> {
    let mut r = Reader::new(bytes, "message");
    let cmd = r.u16_le().ok()?;
    let seq = r.u8().ok()?;
    let paylen = usize::from(r.u16_le().ok()?);
    let rest = r.rest();
    let payload = rest.get(..paylen).unwrap_or(rest).to_vec();
    Some(Frame { cmd, seq, payload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::hex;

    fn frames(input: &[u8]) -> Vec<Frame> {
        let mut r = Reassembler::default();
        r.push(input);
        std::iter::from_fn(|| r.next_frame()).collect()
    }

    fn frame(cmd: u16, seq: u8, payload: &str) -> Frame {
        Frame {
            cmd,
            seq,
            payload: hex(payload),
        }
    }

    // Vectors: realme_t310.md#frame-whole and #frame-long.

    #[test]
    fn encodes_the_documented_examples() {
        assert_eq!(encode(0x0100, 0x01, &[]).unwrap(), hex("AA 07 00 00 00 01 01 00 00"));
        assert_eq!(
            encode(0x0404, 0x06, &hex("01 01 01")).unwrap(),
            hex("AA 0A 00 00 04 04 06 03 00 01 01 01")
        );
    }

    #[test]
    fn long_lengths_use_leb128() {
        let payload = vec![0u8; 124];
        let encoded = encode(0x0418, 0x03, &payload).unwrap();
        assert_eq!(encoded.get(..3), Some(&hex("AA 83 01")[..]));
        assert_eq!(
            frames(&encoded),
            vec![Frame {
                cmd: 0x0418,
                seq: 0x03,
                payload
            }]
        );

        let mut reply = hex("AA 81 02 00 00 22 81 05 FA 00");
        reply.extend(vec![0x11; 250]);
        let decoded = frames(&reply);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded.first().map(|f| f.payload.len()), Some(250));
    }

    #[test]
    fn refuses_frames_that_would_need_slicing() {
        assert!(encode(0x0418, 1, &[0; 700]).is_err());
    }

    #[test]
    fn round_trips_every_payload_size() {
        for len in 0..=640 {
            let payload: Vec<u8> = (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect();
            let encoded = encode(0x0106, 0x42, &payload).unwrap();
            assert_eq!(
                frames(&encoded),
                vec![Frame {
                    cmd: 0x0106,
                    seq: 0x42,
                    payload
                }]
            );
        }
    }

    #[test]
    fn decodes_a_push() {
        assert_eq!(
            frames(&hex("AA 0C 00 00 04 02 FF 05 00 03 01 01 08 00")),
            vec![frame(0x0204, 0xFF, "03 01 01 08 00")]
        );
    }

    /// Captured on the owner's T310 (realme_t310.md#frame-from-buds): a reply and a request
    /// from the buds in one read, then an ack and a battery push in another.
    #[test]
    fn decodes_frames_the_buds_start() {
        assert_eq!(
            frames(&hex(
                "AA 10 00 00 00 81 01 09 00 00 BF 1F 5A 20 45 04 40 00 AA 07 08 00 01 05 0F 00 00"
            )),
            vec![
                frame(0x8100, 0x01, "00 BF 1F 5A 20 45 04 40 00"),
                frame(0x0501, 0x0F, "")
            ]
        );
        assert_eq!(
            frames(&hex(
                "AA 11 00 00 05 82 04 0A 00 00 04 01 00 02 00 03 00 04 00 AA 0F 08 00 04 02 10 08 00 01 03 02 64 01 64 03 00"
            )),
            vec![
                frame(0x8205, 0x04, "00 04 01 00 02 00 03 00 04 00"),
                frame(0x0204, 0x10, "01 03 02 64 01 64 03 00")
            ]
        );
    }

    #[test]
    fn splits_and_joins_reads() {
        let two =
            hex("AA 0D 00 00 04 02 FF 06 00 01 02 01 64 02 64 AA 0F 00 00 04 02 0C 08 00 02 03 01 07 02 07 03 04");
        assert_eq!(
            frames(&two),
            vec![
                frame(0x0204, 0xFF, "01 02 01 64 02 64"),
                frame(0x0204, 0x0C, "02 03 01 07 02 07 03 04")
            ]
        );

        // The same bytes, one at a time.
        let mut r = Reassembler::default();
        let mut out = Vec::new();
        for byte in &two {
            r.push(&[*byte]);
            out.extend(std::iter::from_fn(|| r.next_frame()));
        }
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn skips_noise_and_bad_lengths() {
        let real = "AA 0D 00 00 04 02 FF 06 00 01 02 01 64 02 64";
        let mut r = Reassembler::default();
        r.push(&hex(&format!("13 37 {real}")));
        assert_eq!(r.next_frame(), Some(frame(0x0204, 0xFF, "01 02 01 64 02 64")));
        assert_eq!(r.take_discarded(), 2);

        assert_eq!(frames(&hex(&format!("AA 80 80 80 {real}"))).len(), 1);
        assert!(frames(&hex("AA 00")).is_empty());
        assert!(frames(&hex("AA 05 00 00 23 81 00")).is_empty());
    }

    #[test]
    fn short_payload_is_cut_to_what_is_present() {
        assert_eq!(
            frames(&hex("AA 09 00 00 06 81 01 09 00 07 07")),
            vec![frame(0x8106, 0x01, "07 07")]
        );
    }

    #[test]
    fn joins_slices_and_drops_orphans() {
        // cmd 0x8108, seq 05, paylen 4, payload 00 01 02 03, split in three slices.
        let sliced = hex("AA 07 01 00 00 08 81 05 04 AA 05 02 00 01 00 00 AA 06 03 00 02 01 02 03");
        assert_eq!(frames(&sliced), vec![frame(0x8108, 0x05, "00 01 02 03")]);

        // A last slice with no first slice is dropped; the next whole frame still decodes.
        let orphan = hex("AA 06 03 00 02 00 02 03 AA 07 00 00 00 01 01 00 00");
        assert_eq!(frames(&orphan), vec![frame(0x0100, 0x01, "")]);
    }
}
