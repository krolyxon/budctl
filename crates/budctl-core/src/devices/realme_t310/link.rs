//! Requests and replies over one link, for the OPPO/OnePlus/realme protocol family.
//! Spec: realme_t310.md#seq, #buds-requests
//!
//! Holds no T310 tables. If a second model of this family is added, this file moves to
//! `devices/oppo_family/` together with `frame.rs`.

use std::time::{Duration, Instant};

use super::frame::{self, Frame, Reassembler};
use crate::debug;
use crate::error::{Error, Result};
use crate::transport::Transport;

/// A reply's command id is the request's with this bit set. Spec: realme_t310.md#seq
const REPLY: u16 = 0x8000;
/// budctl's sequence numbers. Spec: realme_t310.md#seq
const FIRST_SEQ: u8 = 0x01;
/// Spec: realme_t310.md#seq
const LAST_SEQ: u8 = 0xBF;
/// At least this long between two frames budctl sends. Spec: realme_t310.md#init
const SEND_GAP: Duration = Duration::from_millis(200);
/// How long to wait for a reply.
const REPLY_TIMEOUT: Duration = Duration::from_secs(3);

/// Spec: realme_t310.md#time-request
const TIME_REQUEST: u16 = 0x0500;
/// Spec: realme_t310.md#buds-requests
const APP_LIST_REQUEST: u16 = 0x050C;
/// Spec: realme_t310.md#buds-requests
const APP_LIST_ANSWER: &[u8] = &[0x00, 0x01, 0x00];
/// Spec: realme_t310.md#buds-requests
const CAPABILITIES_REQUEST: u16 = 0x051C;
/// Spec: realme_t310.md#buds-requests
const CAPABILITIES_ANSWER: &[u8] = &[0x00, 0x00, 0x00];
/// Spec: realme_t310.md#buds-requests
const SPATIAL_MODE_REQUEST: u16 = 0x051D;
/// Spec: realme_t310.md#buds-requests
const SPATIAL_MODE_ANSWER: &[u8] = &[0x01];
/// Command groups the buds use; a request outside them gets `UNKNOWN_ANSWER`.
/// Spec: realme_t310.md#seq, #buds-requests
const KNOWN_GROUPS: [u16; 6] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x0F];
/// Spec: realme_t310.md#buds-requests
const UNKNOWN_ANSWER: &[u8] = &[0x01];

pub(super) struct Link {
    transport: Box<dyn Transport>,
    frames: Reassembler,
    next_seq: u8,
    last_send: Option<Instant>,
    /// Unix time in seconds, for the buds' time request. Replaced in tests.
    clock: fn() -> u32,
}

impl Link {
    pub(super) fn new(transport: Box<dyn Transport>, clock: fn() -> u32) -> Self {
        Self {
            transport,
            frames: Reassembler::default(),
            next_seq: FIRST_SEQ,
            last_send: None,
            clock,
        }
    }

    /// Handle whatever the buds send until `deadline`, answering their requests.
    /// This is how budctl waits: never with a plain sleep.
    pub(super) fn pump_until(&mut self, deadline: Instant) -> Result<()> {
        while let Some(frame) = self.next_frame(deadline)? {
            self.handle_unsolicited(frame)?;
        }
        Ok(())
    }

    /// Send a request and return the payload of its reply.
    pub(super) fn request(&mut self, cmd: u16, payload: &[u8]) -> Result<Vec<u8>> {
        let seq = self.send(cmd, payload)?;
        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            match self.next_frame(deadline)? {
                None => return Err(Error::Timeout(format!("no reply to command 0x{cmd:04X}"))),
                Some(frame) if frame.cmd == cmd | REPLY && frame.seq == seq => return Ok(frame.payload),
                Some(frame) => self.handle_unsolicited(frame)?,
            }
        }
    }

    /// Send a request without waiting for its reply. A reply that comes later is skipped.
    pub(super) fn send(&mut self, cmd: u16, payload: &[u8]) -> Result<u8> {
        if let Some(last) = self.last_send {
            self.pump_until(last + SEND_GAP)?;
        }
        let seq = self.next_seq;
        self.next_seq = if seq >= LAST_SEQ { FIRST_SEQ } else { seq + 1 };
        debug::note(|| format!("send 0x{cmd:04X} seq {seq:02X} [{}]", debug::hex(payload)));
        self.transport.send(&frame::encode(cmd, seq, payload)?)?;
        self.last_send = Some(Instant::now());
        Ok(seq)
    }

    fn next_frame(&mut self, deadline: Instant) -> Result<Option<Frame>> {
        loop {
            if let Some(frame) = self.frames.next_frame() {
                debug::note(|| {
                    format!(
                        "recv 0x{:04X} seq {:02X} [{}]",
                        frame.cmd,
                        frame.seq,
                        debug::hex(&frame.payload)
                    )
                });
                return Ok(Some(frame));
            }
            let discarded = self.frames.take_discarded();
            if discarded > 0 {
                debug::note(|| format!("discarded {discarded} bytes that were not a frame"));
            }
            match self.transport.recv(deadline)? {
                Some(bytes) => self.frames.push(&bytes),
                None => return Ok(None),
            }
        }
    }

    /// A frame nobody is waiting for: answer it if the buds expect an answer, else skip it.
    /// Spec: realme_t310.md#buds-requests, #pushes
    fn handle_unsolicited(&mut self, frame: Frame) -> Result<()> {
        if frame.cmd & REPLY != 0 {
            debug::note(|| format!("skipping late reply 0x{:04X}", frame.cmd));
            return Ok(());
        }
        let answer = match frame.cmd {
            TIME_REQUEST => {
                let mut answer = vec![0x00];
                answer.extend((self.clock)().to_le_bytes());
                answer
            }
            APP_LIST_REQUEST => APP_LIST_ANSWER.to_vec(),
            CAPABILITIES_REQUEST => CAPABILITIES_ANSWER.to_vec(),
            SPATIAL_MODE_REQUEST => SPATIAL_MODE_ANSWER.to_vec(),
            cmd if !KNOWN_GROUPS.contains(&(cmd >> 8)) => UNKNOWN_ANSWER.to_vec(),
            _ => return Ok(()), // pushes and messages that need no answer
        };
        debug::note(|| format!("answer 0x{:04X} with [{}]", frame.cmd, debug::hex(&answer)));
        // Answered at once: some firmware drops the link when this takes too long.
        self.transport
            .send(&frame::encode(frame.cmd | REPLY, frame.seq, &answer)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::mock::{Script, Step, hex};

    fn fixed_clock() -> u32 {
        1_750_000_000
    }

    #[test]
    fn answers_the_time_request_while_waiting() {
        // Vector: realme_t310.md#buds-requests
        let script = Script::new(vec![
            Step::Tx(hex("AA 07 00 00 06 01 01 00 00")),
            Step::Rx(hex("AA 07 00 00 00 05 02 00 00")),
            Step::Tx(hex("AA 0C 00 00 00 85 02 05 00 00 80 E1 4E 68")),
            Step::Rx(hex("AA 0A 00 00 06 81 01 03 00 00 01 01")),
        ]);
        let mut link = Link::new(script.transport(), fixed_clock);
        assert_eq!(link.request(0x0106, &[]).unwrap(), hex("00 01 01"));
        script.assert_done();
    }

    #[test]
    fn skips_pushes_and_stale_replies() {
        let script = Script::new(vec![
            Step::Tx(hex("AA 07 00 00 06 01 01 00 00")),
            Step::Rx(hex("AA 0C 00 00 04 02 FF 05 00 03 01 01 08 00")),
            Step::Rx(hex("AA 08 00 00 06 81 77 01 00 00")),
            Step::Rx(hex("AA 08 00 00 06 81 01 01 00 00")),
        ]);
        let mut link = Link::new(script.transport(), fixed_clock);
        assert_eq!(link.request(0x0106, &[]).unwrap(), hex("00"));
        script.assert_done();
    }

    #[test]
    fn unknown_requests_get_the_unknown_answer() {
        let script = Script::new(vec![
            Step::Rx(hex("AA 07 00 00 34 12 09 00 00")),
            Step::Tx(hex("AA 08 00 00 34 92 09 01 00 01")),
        ]);
        let mut link = Link::new(script.transport(), fixed_clock);
        link.pump_until(Instant::now()).unwrap();
        script.assert_done();
    }

    #[test]
    fn silence_is_a_timeout_and_a_drop_is_not_connected() {
        let script = Script::new(vec![Step::Tx(hex("AA 07 00 00 06 01 01 00 00"))]);
        let mut link = Link::new(script.transport(), fixed_clock);
        assert!(matches!(link.request(0x0106, &[]), Err(Error::Timeout(_))));

        let script = Script::new(vec![Step::Tx(hex("AA 07 00 00 06 01 01 00 00")), Step::Close]);
        let mut link = Link::new(script.transport(), fixed_clock);
        assert!(matches!(link.request(0x0106, &[]), Err(Error::NotConnected(_))));
    }

    #[test]
    fn sequence_numbers_wrap_inside_budctls_range() {
        let mut steps = Vec::new();
        for seq in (FIRST_SEQ..=LAST_SEQ).chain([FIRST_SEQ]) {
            steps.push(Step::Tx(frame::encode(0x0100, seq, &[]).unwrap()));
        }
        let script = Script::new(steps);
        let mut link = Link::new(script.transport(), fixed_clock);
        for _ in FIRST_SEQ..=LAST_SEQ + 1 {
            link.send(0x0100, &[]).unwrap();
        }
        script.assert_done();
    }
}
