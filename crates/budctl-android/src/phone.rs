//! The app's side of the worker, as the worker sees it: the paired devices, the RFCOMM link, and
//! where events and log lines go. `kotlin.rs` provides it over JNI; the tests fake it.

use std::fmt::Display;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use budctl_core::devices;
use budctl_core::transport::Transport;
use budctl_core::worker::{Connect, Event, Report};
use budctl_core::{Error, Found, Result};

use crate::json;

/// What the worker needs from the app. Every method is called on the worker's thread.
pub trait Phone: Send + Sync {
    /// The paired devices, as JSON (read by [`json::paired_devices`]).
    fn paired_devices(&self) -> Result<String>;

    /// Opens the RFCOMM link to `service` on the device at `address`, in place of any link open
    /// before.
    fn connect(&self, address: &str, service: &str) -> Result<()>;

    fn send(&self, bytes: &[u8]) -> Result<()>;

    /// The next bytes from the buds, `None` once `timeout` passes with nothing, or no bytes at all
    /// once the link is closed.
    fn recv(&self, timeout: Duration) -> Result<Option<Vec<u8>>>;

    fn close(&self);

    fn event(&self, json: &str);

    fn log(&self, line: &str);
}

/// An error in the app's side, rather than with the buds: a bug.
pub fn app_error(context: &'static str, detail: impl Display) -> Error {
    Error::Os {
        context,
        source: io::Error::other(detail.to_string()),
    }
}

/// Finds the buds among the paired devices and opens a session, as `budctl` does through BlueZ.
pub fn connect(phone: Arc<dyn Phone>) -> Connect {
    Box::new(move |found| {
        let paired = json::paired_devices(&phone.paired_devices()?)?;
        let found_list = paired
            .into_iter()
            .map(|device| Found {
                model: devices::identify(&device),
                device,
            })
            .collect();
        let (device, model) = budctl_core::select(found_list, None)?;
        found(device.alias.clone());
        budctl_core::ensure_connected(&device)?;

        // The first of the model's services the buds list, as SDP finds it on Linux. Android
        // caches the list; when it has none yet, the first service is tried.
        let address = device.address.to_string();
        let listed = |service: &String| device.uuids.is_empty() || device.uuids.contains(service);
        let service = model
            .rfcomm_uuids()
            .iter()
            .map(ToString::to_string)
            .find(listed)
            .ok_or_else(|| Error::NoService(address.clone()))?;
        phone.connect(&address, &service)?;
        budctl_core::open(model, Box::new(Link(phone.clone())))
    })
}

/// The RFCOMM link the app opened, as a [`Transport`]. Dropping it closes the link.
struct Link(Arc<dyn Phone>);

impl Transport for Link {
    fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.0.send(bytes)
    }

    fn recv(&mut self, deadline: Instant) -> Result<Option<Vec<u8>>> {
        match self.0.recv(deadline.saturating_duration_since(Instant::now()))? {
            Some(bytes) if bytes.is_empty() => Err(Error::NotConnected(
                "the buds closed the control link (case lid closed?)".to_owned(),
            )),
            received => Ok(received),
        }
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// The worker's reports, passed on to the app as JSON.
pub struct Reporter(pub Arc<dyn Phone>);

impl Report for Reporter {
    fn event(&self, event: Event) {
        match json::event(&event) {
            Ok(json) => self.0.event(&json),
            Err(e) => self.0.log(&format!("could not write an event as JSON: {e}")),
        }
    }

    fn log(&self, line: &str) {
        self.0.log(line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use budctl_core::Kind;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    const T310: &str = "realme Buds T310";
    const FIRST: &str = "0000079a-d102-11e1-9b23-00025b00a5a5";
    const SECOND: &str = "00001107-d102-11e1-9b23-00025b00a5a5";

    /// A phone with one paired device, whose connect answers as told.
    #[derive(Default)]
    struct Fake {
        devices: String,
        refuse: Option<String>,
        /// What `recv` returns, in order; then nothing.
        incoming: Mutex<VecDeque<Option<Vec<u8>>>>,
        calls: Mutex<Vec<String>>,
    }

    impl Fake {
        fn with(name: &str, connected: bool, uuids: &[&str]) -> Self {
            let device = serde_json::json!([{
                "address": "00:11:22:33:44:55", "name": name, "alias": "My buds",
                "connected": connected, "uuids": uuids,
            }]);
            Fake {
                devices: device.to_string(),
                ..Fake::default()
            }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Phone for Fake {
        fn paired_devices(&self) -> Result<String> {
            Ok(self.devices.clone())
        }

        fn connect(&self, address: &str, service: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("connect {address} {service}"));
            match &self.refuse {
                Some(json) => Err(json::connect_failure(json)),
                None => Ok(()),
            }
        }

        fn send(&self, _bytes: &[u8]) -> Result<()> {
            Ok(())
        }

        fn recv(&self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
            Ok(self.incoming.lock().unwrap().pop_front().flatten())
        }

        fn close(&self) {
            self.calls.lock().unwrap().push("close".to_owned());
        }

        fn event(&self, json: &str) {
            self.calls.lock().unwrap().push(json.to_owned());
        }

        fn log(&self, _line: &str) {}
    }

    /// Runs the Android connect against `phone`, and the names it reported.
    fn open(phone: &Arc<Fake>) -> (Result<()>, Vec<String>) {
        let found = Mutex::new(Vec::new());
        let result = connect(phone.clone())(&|name| found.lock().unwrap().push(name)).map(drop);
        (result, found.into_inner().unwrap())
    }

    #[test]
    fn no_supported_buds_is_not_found() {
        let phone = Arc::new(Fake::with("Some Speaker", true, &[]));
        let (result, found) = open(&phone);
        assert!(matches!(result, Err(Error::NoDevice(_))));
        assert!(found.is_empty());
    }

    #[test]
    fn buds_that_are_not_connected_are_named_but_not_dialled() {
        let phone = Arc::new(Fake::with(T310, false, &[FIRST]));
        let (result, found) = open(&phone);
        assert!(matches!(result, Err(Error::NotConnected(message)) if message.contains("My buds")));
        assert_eq!(found, ["My buds"]);
        assert!(phone.calls().is_empty());
    }

    #[test]
    fn a_refused_connect_keeps_its_kind() {
        let phone = Arc::new(Fake {
            refuse: Some(r#"{"kind": "busy", "message": "refused"}"#.to_owned()),
            ..Fake::with(T310, true, &[FIRST])
        });
        let (result, _) = open(&phone);
        assert_eq!(result.err().map(|e| e.kind()), Some(Kind::Busy));
        assert_eq!(phone.calls(), [format!("connect 00:11:22:33:44:55 {FIRST}")]);
    }

    #[test]
    fn the_first_service_the_buds_list_is_used() {
        let phone = Arc::new(Fake {
            refuse: Some(r#"{"kind": "busy", "message": "refused"}"#.to_owned()),
            ..Fake::with(T310, true, &[SECOND])
        });
        let _ = open(&phone);
        assert_eq!(phone.calls(), [format!("connect 00:11:22:33:44:55 {SECOND}")]);

        let phone = Arc::new(Fake::with(T310, true, &["0000110b-0000-1000-8000-00805f9b34fb"]));
        assert!(matches!(open(&phone).0, Err(Error::NoService(_))));
    }

    #[test]
    fn the_link_reports_silence_and_a_closed_link() {
        let phone = Arc::new(Fake::default());
        phone
            .incoming
            .lock()
            .unwrap()
            .extend([Some(vec![0xAA, 0x07]), None, Some(Vec::new())]);
        let mut link = Link(phone.clone());
        let deadline = Instant::now();
        assert_eq!(link.recv(deadline).unwrap(), Some(vec![0xAA, 0x07]));
        assert_eq!(link.recv(deadline).unwrap(), None);
        assert!(matches!(link.recv(deadline), Err(Error::NotConnected(_))));
        drop(link);
        assert_eq!(phone.calls(), ["close"]);
    }

    #[test]
    fn reports_reach_the_app_as_json() {
        let phone = Arc::new(Fake::default());
        Reporter(phone.clone()).event(Event::Lost("gone".to_owned()));
        assert_eq!(phone.calls(), [r#"{"event":"lost","message":"gone"}"#]);
    }
}
