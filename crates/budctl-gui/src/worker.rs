//! The connection to the buds: budctl-core's worker, which owns the session on a thread of its
//! own, run as an iced subscription and connected through BlueZ.

use std::time::Duration;

use budctl_core::bt::BdAddr;
use budctl_core::worker::{self, Connect, Report};
pub use budctl_core::worker::{Command, Event, IDLE, Reading};
use iced::futures::channel::mpsc as channel;
use iced::futures::{SinkExt, Stream, StreamExt};

use crate::{Message, log};

/// The worker, as a stream of messages for an iced subscription: [`Message::Ready`] once it takes
/// commands, then each of its events. `device` picks the buds, as `budctl --device` does; `idle`
/// is how long a session stays open after the last command.
pub fn run(&(device, idle): &(Option<BdAddr>, Duration)) -> impl Stream<Item = Message> + use<> {
    iced::stream::channel(16, async move |mut output: channel::Sender<Message>| {
        let (events, mut outbox) = channel::unbounded();
        let commands = match worker::start(bluetooth(device), Box::new(Window(events)), idle) {
            Ok(commands) => commands,
            Err(e) => {
                let message = format!("could not start the Bluetooth thread: {e}");
                let _ = output.send(Message::Worker(Event::Lost(message))).await;
                return;
            }
        };
        if output.send(Message::Ready(commands)).await.is_err() {
            return;
        }
        while let Some(event) = outbox.next().await {
            if output.send(Message::Worker(event)).await.is_err() {
                break;
            }
        }
    })
}

/// The window's side of the worker: events go to the subscription, lines to the GUI's log and,
/// with `--debug`, between the frames on stderr.
struct Window(channel::UnboundedSender<Event>);

impl Report for Window {
    fn event(&self, event: Event) {
        // An error only means the window is gone; the worker ends once its sender is dropped.
        let _ = self.0.unbounded_send(event);
    }

    fn log(&self, line: &str) {
        if budctl_core::debug::enabled() {
            eprintln!("#        {line}");
        }
        log::line(line);
    }
}

/// Over Bluetooth, through BlueZ: what the window uses.
fn bluetooth(device: Option<BdAddr>) -> Connect {
    Box::new(move |found| {
        let (device, model) = budctl_core::select(budctl_core::scan()?, device)?;
        found(device.alias.clone());
        budctl_core::connect(&device, model)
    })
}
