//! The connection to the buds, on a thread of its own. budctl-core blocks, and a write with its
//! read-back can take seconds, so the window never waits on it: it sends [`Command`]s and gets
//! [`Event`]s back, in order, one per command.
//!
//! The session opens when a command needs it and closes after [`IDLE`] without one, so the
//! control channel is free for other apps (CLAUDE.md, connection behavior). While it is open, the
//! buds' own requests are answered.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use budctl_core::bt::BdAddr;
use budctl_core::devices::Session;
use budctl_core::status::{
    EqBand, EqPreset, EqPresets, Gesture, NoiseControl, NoiseMode, Status, Switch, TouchControls,
};
use budctl_core::{Error, Kind, Result};
use iced::futures::channel::mpsc as channel;
use iced::futures::{SinkExt, Stream, StreamExt};

/// How long the session stays open after the last command.
const IDLE: Duration = Duration::from_secs(10);
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
}

#[derive(Debug, Clone)]
pub enum Event {
    /// The worker takes commands here.
    Ready(Sender<Command>),
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
}

/// The worker, as a stream of events for an iced subscription. `device` picks the buds, as
/// `budctl --device` does.
pub fn run(device: &Option<BdAddr>) -> impl Stream<Item = Event> + use<> {
    let device = *device;
    iced::stream::channel(16, async move |mut output: channel::Sender<Event>| {
        let (commands, inbox) = mpsc::channel();
        let (events, mut outbox) = channel::unbounded();
        if output.send(Event::Ready(commands)).await.is_err() {
            return;
        }
        let worker = Worker::new(bluetooth(device), events, IDLE);
        let started = thread::Builder::new()
            .name("budctl-buds".to_owned())
            .spawn(move || worker.run(&inbox));
        if let Err(e) = started {
            let message = format!("could not start the Bluetooth thread: {e}");
            let _ = output.send(Event::Lost(message)).await;
            return;
        }
        while let Some(event) = outbox.next().await {
            if output.send(event).await.is_err() {
                break;
            }
        }
    })
}

/// Finds the buds and opens a session, calling `found` with their name once it is known.
type Connect = Box<dyn FnMut(&dyn Fn(String)) -> Result<Box<dyn Session>> + Send>;

/// Over Bluetooth, through BlueZ: what the window uses. Tests use a fake.
fn bluetooth(device: Option<BdAddr>) -> Connect {
    Box::new(move |found| {
        let (device, model) = budctl_core::select(budctl_core::scan()?, device)?;
        found(device.alias.clone());
        budctl_core::connect(&device, model)
    })
}

struct Worker {
    connect: Connect,
    session: Option<Box<dyn Session>>,
    events: channel::UnboundedSender<Event>,
    last_command: Instant,
    /// How long the session stays open after the last command.
    idle: Duration,
}

impl Worker {
    fn new(connect: Connect, events: channel::UnboundedSender<Event>, idle: Duration) -> Self {
        Worker {
            connect,
            session: None,
            events,
            last_command: Instant::now(),
            idle,
        }
    }

    /// Runs until the window is gone.
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
            if budctl_core::debug::enabled() {
                eprintln!(
                    "#        session closed after {} s without a command",
                    self.idle.as_secs()
                );
            }
        } else if let Err(error) = session.serve() {
            self.session = None;
            self.send(Event::Lost(error.to_string()));
        }
    }

    fn handle(&mut self, command: Command) {
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
            Command::HoldCycle(modes) => {
                let modes: Vec<&str> = modes.iter().map(String::as_str).collect();
                self.session()
                    .and_then(|session| session.set_hold_cycle(&modes))
                    .map(Event::HoldCycle)
            }
        };
        match answer {
            Ok(event) => self.send(event),
            Err(error) => {
                self.session = None;
                self.send(Event::Failed {
                    kind: error.kind(),
                    message: error.to_string(),
                });
            }
        }
    }

    fn read(&mut self) -> Result<Reading> {
        let session = self.session()?;
        let status = session.status()?;
        let eq = session.eq_presets().ok();
        let touch = session.touch_controls().ok();
        Ok(Reading { status, eq, touch })
    }

    /// The open session, opening one first if there is none.
    fn session(&mut self) -> Result<&mut Box<dyn Session>> {
        if self.session.is_none() {
            let events = self.events.clone();
            let found = move |name| {
                let _ = events.unbounded_send(Event::Found(name));
            };
            self.session = Some((self.connect)(&found)?);
        }
        self.session
            .as_mut()
            .ok_or_else(|| Error::NotConnected("the session closed".to_owned()))
    }

    fn send(&self, event: Event) {
        // An error only means the window is gone; `run` ends on its next receive.
        let _ = self.events.unbounded_send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::executor::block_on;
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
    }

    struct Fake(Probe);

    impl Session for Fake {
        fn status(&mut self) -> Result<Status> {
            Ok(Status::default())
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

    /// A worker on its own thread with fake buds, closing idle sessions after 200 ms.
    fn start(
        probe: &Probe,
    ) -> (
        Sender<Command>,
        channel::UnboundedReceiver<Event>,
        thread::JoinHandle<()>,
    ) {
        let fake = probe.clone();
        let connect: Connect = Box::new(move |found| {
            found("Fake buds".to_owned());
            fake.opened.fetch_add(1, Ordering::SeqCst);
            fake.closed.store(false, Ordering::SeqCst);
            Ok(Box::new(Fake(fake.clone())))
        });
        let (events, outbox) = channel::unbounded();
        let (commands, inbox) = mpsc::channel();
        let worker = Worker::new(connect, events, Duration::from_millis(200));
        let thread = thread::spawn(move || worker.run(&inbox));
        (commands, outbox, thread)
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
        let (commands, mut events, thread) = start(&probe);

        commands.send(Command::Read).unwrap();
        assert!(matches!(block_on(events.next()), Some(Event::Found(name)) if name == "Fake buds"));
        assert!(matches!(block_on(events.next()), Some(Event::Read(_))));
        assert_eq!(probe.opened.load(Ordering::SeqCst), 1);

        // While it waits, it answers the buds; then it closes the session.
        wait_for(|| probe.closed.load(Ordering::SeqCst));
        assert!(probe.served.load(Ordering::SeqCst) > 0);

        commands.send(Command::Read).unwrap();
        assert!(matches!(block_on(events.next()), Some(Event::Found(_))));
        assert!(matches!(block_on(events.next()), Some(Event::Read(_))));
        assert_eq!(probe.opened.load(Ordering::SeqCst), 2);

        // The window goes away: the worker ends.
        drop(commands);
        thread.join().unwrap();
    }

    #[test]
    fn a_dropped_link_is_reported_once() {
        let probe = Probe::default();
        let (commands, mut events, thread) = start(&probe);
        commands.send(Command::Read).unwrap();
        let _ = block_on(events.next());
        let _ = block_on(events.next());

        probe.drop_link.store(true, Ordering::SeqCst);
        assert!(matches!(block_on(events.next()), Some(Event::Lost(_))));
        assert!(probe.closed.load(Ordering::SeqCst));

        drop(commands);
        thread.join().unwrap();
        assert!(block_on(events.next()).is_none(), "nothing more after the link dropped");
    }
}
