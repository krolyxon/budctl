//! A scripted transport for tests: it checks every byte budctl sends and replays what the buds
//! would answer, without hardware and without waiting for real time to pass.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::Transport;
use crate::debug;
use crate::error::{Error, Result};

#[derive(Debug)]
pub(crate) enum Step {
    /// budctl must send exactly these bytes next.
    Tx(Vec<u8>),
    /// The buds send these bytes.
    Rx(Vec<u8>),
    /// The buds drop the link.
    Close,
}

/// Cheap to clone: the test keeps one handle to check that the script was used up.
#[derive(Clone, Default)]
pub(crate) struct Script(Arc<Mutex<VecDeque<Step>>>);

impl Script {
    pub(crate) fn new(steps: Vec<Step>) -> Self {
        Script(Arc::new(Mutex::new(steps.into())))
    }

    pub(crate) fn transport(&self) -> Box<dyn Transport> {
        Box::new(self.clone())
    }

    pub(crate) fn assert_done(&self) {
        let left = self.0.lock().unwrap();
        assert!(left.is_empty(), "script not finished, next step: {:?}", left.front());
    }
}

impl Transport for Script {
    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        let mut steps = self.0.lock().unwrap();
        match steps.pop_front() {
            Some(Step::Tx(expected)) => assert_eq!(
                debug::hex(bytes),
                debug::hex(&expected),
                "budctl sent something else than the script expects"
            ),
            other => panic!("unexpected send {} (next step: {other:?})", debug::hex(bytes)),
        }
        Ok(())
    }

    /// Returns the next scripted bytes from the buds. When the script expects budctl to send
    /// next, the buds stay silent: the deadline "passes" at once.
    fn recv(&mut self, _deadline: Instant) -> Result<Option<Vec<u8>>> {
        let mut steps = self.0.lock().unwrap();
        match steps.front() {
            Some(Step::Rx(_)) => match steps.pop_front() {
                Some(Step::Rx(bytes)) => Ok(Some(bytes)),
                _ => unreachable!(),
            },
            Some(Step::Close) => {
                steps.pop_front();
                Err(Error::NotConnected(
                    "the buds closed the control link (script)".to_owned(),
                ))
            }
            _ => Ok(None),
        }
    }
}

/// `"AA 07 00"` → `[0xAA, 0x07, 0x00]`
pub(crate) fn hex(text: &str) -> Vec<u8> {
    text.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect()
}
