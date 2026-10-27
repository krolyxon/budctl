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
use budctl_core::status::{EqPresets, NoiseControl, NoiseMode, Status, Switch};
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
        let worker = Worker {
            device,
            session: None,
            events,
            last_command: Instant::now(),
        };
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

struct Worker {
    device: Option<BdAddr>,
    session: Option<Box<dyn Session>>,
    events: channel::UnboundedSender<Event>,
    last_command: Instant,
}

impl Worker {
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
        if self.last_command.elapsed() >= IDLE {
            self.session = None;
            if budctl_core::debug::enabled() {
                eprintln!("#        session closed after {} s without a command", IDLE.as_secs());
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
        Ok(Reading { status, eq })
    }

    /// The open session, opening one first if there is none.
    fn session(&mut self) -> Result<&mut Box<dyn Session>> {
        if self.session.is_none() {
            let (device, model) = budctl_core::select(budctl_core::scan()?, self.device)?;
            self.send(Event::Found(device.alias.clone()));
            self.session = Some(budctl_core::connect(&device, model)?);
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
