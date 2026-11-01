//! The connection to the buds, on a thread of its own. Sessions block, and a write with its
//! read-back can take seconds, so a front end never waits on one: it sends [`Command`]s and gets
//! [`Event`]s back, in order, one per command. The desktop app and the Android app both run it.
//!
//! The session opens when a command needs it and closes after an idle time without one, so the
//! control channel is free for other apps (CLAUDE.md, connection behavior). While it is open, the
//! buds' own requests are answered.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::debug;
use crate::devices::Session;
use crate::error::{Error, Kind, Result};
use crate::status::{
    DualDevice, EqBand, EqPreset, EqPresets, Gesture, NoiseControl, NoiseMode, Status, Switch, TouchControls,
};

/// How long the session stays open after the last command, unless the front end says otherwise.
pub const IDLE: Duration = Duration::from_secs(10);
/// How often an open session answers the buds while it waits for a command.
const SERVE_EVERY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub enum Command {
    /// Read everything, connecting first if needed.
    Read,
    Noise(NoiseMode),
    /// A switch's key, and whether to turn it on.
    Switch(&'static str, bool),
    /// Select a built-in EQ preset, by name.
    Eq(String),
    /// Save a custom EQ preset (`custom-<id>`) with these bands changed, which also selects it.
    /// With no bands, it only selects it.
    CustomEq(String, Vec<EqBand>),
    /// Give a bud's gesture an action, by name: `left`, `double-tap`, `play-pause`.
    Touch {
        side: String,
        gesture: String,
        action: String,
    },
    /// Set the noise-control modes a hold steps through.
    HoldCycle(Vec<String>),
    /// Start (`true`) or stop ringing the buds.
    Find(bool),
    /// Set the bass level.
    Bass(i8),
    /// Create a custom EQ preset with this name.
    CreateEq(String),
    /// Give a custom EQ preset (`custom-<id>`) a new name.
    RenameEq(String, String),
    /// Delete a custom EQ preset (`custom-<id>`).
    DeleteEq(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    /// The buds a new session is for, by the name their owner gave them. Answers no command.
    Found(String),
    /// The answer to [`Command::Read`]. Boxed: it is much larger than the other events.
    Read(Box<Reading>),
    /// The answer to [`Command::Noise`]: what the buds report after the write.
    Noise(NoiseControl),
    /// The answer to [`Command::Switch`]: the switch, as read back.
    Switch(Switch),
    /// The answer to [`Command::Eq`]: the preset now in use.
    Eq(String),
    /// The answer to [`Command::CustomEq`]: the preset, as read back.
    CustomEq(EqPreset),
    /// The answer to [`Command::Touch`]: the whole gesture table, as read back.
    Touch(Vec<Gesture>),
    /// The answer to [`Command::HoldCycle`]: the modes, as read back.
    HoldCycle(Vec<String>),
    /// The answer to [`Command::Find`]: whether the buds were told to ring.
    Ringing(bool),
    /// The answer to [`Command::Bass`]: the level the buds took.
    Bass(i8),
    /// The answer to [`Command::CreateEq`] and [`Command::DeleteEq`]: the whole preset list after
    /// it (ids can change).
    Presets(EqPresets),
    /// The answer to a command that failed. The session is closed; the next command opens a
    /// new one.
    Failed { kind: Kind, message: String },
    /// The link dropped while the session waited. Answers no command.
    Lost(String),
}

#[derive(Debug, Clone)]
pub struct Reading {
    pub status: Status,
    /// The EQ presets, when the buds can list them: they name the custom ones.
    pub eq: Option<EqPresets>,
    /// The touch controls, when the buds report them.
    pub touch: Option<TouchControls>,
    /// The levels the bass level takes, when the buds have one to set.
    pub bass_range: Option<(i8, i8)>,
    /// The devices dual connection keeps, when the buds list them.
    pub dual: Option<Vec<DualDevice>>,
}

/// Finds the buds and opens a session, calling `found` with their name once it is known. Each
/// front end brings its own: BlueZ on Linux, the app's Kotlin side on Android.
pub type Connect = Box<dyn FnMut(&dyn Fn(String)) -> Result<Box<dyn Session>> + Send>;

/// The front end's side of the worker.
pub trait Report: Send {
    /// An answer to a command, or [`Event::Found`] or [`Event::Lost`].
    fn event(&self, event: Event);

    /// A line for the front end's log: each command, sessions opening and closing, failures.
    fn log(&self, line: &str);
}

/// Starts the worker on a thread of its own. It takes commands from the sender it returns, and
/// ends, closing any session, once that sender is dropped. `idle` is how long a session stays
/// open after the last command.
pub fn start(connect: Connect, report: Box<dyn Report>, idle: Duration) -> std::io::Result<Sender<Command>> {
    let (commands, inbox) = mpsc::channel();
    let worker = Worker::new(connect, report, idle);
    thread::Builder::new()
        .name("budctl-buds".to_owned())
        .spawn(move || worker.run(&inbox))?;
    Ok(commands)
}

struct Worker {
    connect: Connect,
    session: Option<Box<dyn Session>>,
    report: Box<dyn Report>,
    last_command: Instant,
    /// How long the session stays open after the last command.
    idle: Duration,
    /// The buds once left the dual-connection list unanswered. They are not asked again, so
    /// a reading does not wait for that timeout every time.
    dual_unanswered: bool,
}

impl Worker {
    fn new(connect: Connect, report: Box<dyn Report>, idle: Duration) -> Self {
        Worker {
            connect,
            session: None,
            report,
            last_command: Instant::now(),
            idle,
            dual_unanswered: false,
        }
    }

    /// Runs until the front end drops its sender.
    fn run(mut self, inbox: &Receiver<Command>) {
        loop {
            let next = if self.session.is_some() {
                inbox.recv_timeout(SERVE_EVERY)
            } else {
                inbox.recv().map_err(|_| RecvTimeoutError::Disconnected)
            };
            match next {
                Ok(command) => {
                    self.last_command = Instant::now();
                    self.handle(command);
                }
                Err(RecvTimeoutError::Timeout) => self.wait(),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Between commands: answer the buds, and close the session once it has been idle long
    /// enough.
    fn wait(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if self.last_command.elapsed() >= self.idle {
            self.session = None;
            let message = format!("session closed after {} s without a command", self.idle.as_secs());
            debug::note(|| message.clone());
            self.report.log(&message);
        } else if let Err(error) = session.serve() {
            self.session = None;
            self.report.log(&format!("link lost: {error}"));
            self.report.event(Event::Lost(error.to_string()));
        }
    }

    fn handle(&mut self, command: Command) {
        self.report.log(&format!("{command:?}"));
        let answer = match command {
            Command::Read => self.read().map(|reading| Event::Read(Box::new(reading))),
            Command::Noise(mode) => self
                .session()
                .and_then(|session| session.set_noise_control(mode))
                .map(Event::Noise),
            Command::Switch(key, on) => self
                .session()
                .and_then(|session| session.set_switch(key, on))
                .map(Event::Switch),
            Command::Eq(preset) => self
                .session()
                .and_then(|session| session.set_eq(&preset))
                .map(Event::Eq),
            Command::CustomEq(preset, bands) => self
                .session()
                .and_then(|session| session.set_custom_eq(&preset, &bands))
                .map(Event::CustomEq),
            Command::Touch { side, gesture, action } => self
                .session()
                .and_then(|session| session.set_touch(&side, &gesture, &action, false))
                .map(|change| Event::Touch(change.gestures)),
            Command::CreateEq(name) => self
                .session()
                .and_then(|session| {
                    session.create_custom_eq(&name)?;
                    session.eq_presets()
                })
                .map(Event::Presets),
            Command::DeleteEq(preset) => self
                .session()
                .and_then(|session| {
                    session.delete_custom_eq(&preset)?;
                    session.eq_presets()
                })
                .map(Event::Presets),
            Command::RenameEq(preset, name) => self
                .session()
                .and_then(|session| session.rename_custom_eq(&preset, &name))
                .map(Event::CustomEq),
            Command::Bass(level) => self
                .session()
                .and_then(|session| session.set_bass_level(level))
                .map(|()| Event::Bass(level)),
            Command::Find(ring) => self
                .session()
                .and_then(|session| session.find(ring))
                .map(|()| Event::Ringing(ring)),
            Command::HoldCycle(modes) => {
                let modes: Vec<&str> = modes.iter().map(String::as_str).collect();
                self.session()
                    .and_then(|session| session.set_hold_cycle(&modes))
                    .map(Event::HoldCycle)
            }
        };
        match answer {
            Ok(event) => {
                self.report.log("  done");
                self.report.event(event);
            }
            Err(error) => {
                self.report.log(&format!("  failed ({:?}): {error}", error.kind()));
                self.session = None;
                self.report.event(Event::Failed {
                    kind: error.kind(),
                    message: error.to_string(),
                });
            }
        }
    }

    fn read(&mut self) -> Result<Reading> {
        let ask_dual = !self.dual_unanswered;
        let session = self.session()?;
        let status = session.status()?;
        let eq = session.eq_presets().ok();
        let touch = session.touch_controls().ok();
        let bass_range = session.bass_range();
        let dual = match ask_dual.then(|| session.dual_devices()) {
            Some(Ok(devices)) => Some(devices),
            Some(Err(Error::Timeout(_))) => {
                self.dual_unanswered = true;
                None
            }
            Some(Err(_)) | None => None,
        };
        Ok(Reading {
            status,
            eq,
            touch,
            bass_range,
            dual,
        })
    }

    /// The open session, opening one first if there is none.
    fn session(&mut self) -> Result<&mut Box<dyn Session>> {
        if self.session.is_none() {
            let report = &self.report;
            let found = |name: String| {
                report.log(&format!("  connecting to {name}"));
                report.event(Event::Found(name));
            };
            self.session = Some((self.connect)(&found)?);
            self.report.log("  session open");
        }
        self.session
            .as_mut()
            .ok_or_else(|| Error::NotConnected("the session closed".to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// What the fake sessions report back to the test.
    #[derive(Clone, Default)]
    struct Probe {
        opened: Arc<AtomicUsize>,
        served: Arc<AtomicUsize>,
        closed: Arc<AtomicBool>,
        /// Make `serve` fail, as when the buds drop the link.
        drop_link: Arc<AtomicBool>,
        /// How often the dual-connection list was asked for. The fake buds never answer it.
        dual_asked: Arc<AtomicUsize>,
    }

    struct Fake(Probe);

    impl Session for Fake {
        fn status(&mut self) -> Result<Status> {
            Ok(Status::default())
        }

        fn dual_devices(&mut self) -> Result<Vec<DualDevice>> {
            self.0.dual_asked.fetch_add(1, Ordering::SeqCst);
            Err(Error::Timeout("the dual-connection list".to_owned()))
        }

        fn serve(&mut self) -> Result<()> {
            self.0.served.fetch_add(1, Ordering::SeqCst);
            if self.0.drop_link.load(Ordering::SeqCst) {
                return Err(Error::NotConnected("the buds closed the control link".to_owned()));
            }
            Ok(())
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            self.0.closed.store(true, Ordering::SeqCst);
        }
    }

    /// A front end that only collects the events.
    struct Collect(Sender<Event>);

    impl Report for Collect {
        fn event(&self, event: Event) {
            let _ = self.0.send(event);
        }

        fn log(&self, _line: &str) {}
    }

    /// A worker on its own thread with fake buds, closing idle sessions after 200 ms.
    fn start(probe: &Probe) -> (Sender<Command>, Receiver<Event>, thread::JoinHandle<()>) {
        let fake = probe.clone();
        let connect: Connect = Box::new(move |found| {
            found("Fake buds".to_owned());
            fake.opened.fetch_add(1, Ordering::SeqCst);
            fake.closed.store(false, Ordering::SeqCst);
            Ok(Box::new(Fake(fake.clone())))
        });
        let (events, outbox) = mpsc::channel();
        let (commands, inbox) = mpsc::channel();
        let worker = Worker::new(connect, Box::new(Collect(events)), Duration::from_millis(200));
        let thread = thread::spawn(move || worker.run(&inbox));
        (commands, outbox, thread)
    }

    /// The next event, waiting at most 5 s.
    fn next(events: &Receiver<Event>) -> Option<Event> {
        events.recv_timeout(Duration::from_secs(5)).ok()
    }

    /// Waits, at most 5 s, for `done`. A receive that times out stands in for a sleep.
    fn wait_for(done: impl Fn() -> bool) {
        let (_keep, never) = mpsc::channel::<()>();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(Instant::now() < deadline, "gave up waiting");
            let _ = never.recv_timeout(Duration::from_millis(10));
        }
    }

    #[test]
    fn an_idle_session_closes_and_the_next_command_opens_another() {
        let probe = Probe::default();
        let (commands, events, thread) = start(&probe);

        commands.send(Command::Read).unwrap();
        assert!(matches!(next(&events), Some(Event::Found(name)) if name == "Fake buds"));
        assert!(matches!(next(&events), Some(Event::Read(_))));
        assert_eq!(probe.opened.load(Ordering::SeqCst), 1);

        // While it waits, it answers the buds; then it closes the session.
        wait_for(|| probe.closed.load(Ordering::SeqCst));
        assert!(probe.served.load(Ordering::SeqCst) > 0);

        commands.send(Command::Read).unwrap();
        assert!(matches!(next(&events), Some(Event::Found(_))));
        assert!(matches!(next(&events), Some(Event::Read(_))));
        assert_eq!(probe.opened.load(Ordering::SeqCst), 2);

        // The front end goes away: the worker ends.
        drop(commands);
        thread.join().unwrap();
    }

    #[test]
    fn a_dual_list_left_unanswered_is_not_asked_for_again() {
        let probe = Probe::default();
        let (commands, events, thread) = start(&probe);
        commands.send(Command::Read).unwrap();
        commands.send(Command::Read).unwrap();
        assert!(matches!(next(&events), Some(Event::Found(_))));
        for _ in 0..2 {
            let Some(Event::Read(reading)) = next(&events) else {
                panic!("a reading expected");
            };
            assert!(reading.dual.is_none());
        }
        assert_eq!(probe.dual_asked.load(Ordering::SeqCst), 1);

        drop(commands);
        thread.join().unwrap();
    }

    #[test]
    fn a_dropped_link_is_reported_once() {
        let probe = Probe::default();
        let (commands, events, thread) = start(&probe);
        commands.send(Command::Read).unwrap();
        let _ = next(&events);
        let _ = next(&events);

        probe.drop_link.store(true, Ordering::SeqCst);
        assert!(matches!(next(&events), Some(Event::Lost(_))));
        assert!(probe.closed.load(Ordering::SeqCst));

        drop(commands);
        thread.join().unwrap();
        assert!(events.recv().is_err(), "nothing more after the link dropped");
    }
}
