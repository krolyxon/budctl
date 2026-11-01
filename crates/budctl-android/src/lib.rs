//! budctl-core inside the Android app (docs/android.md).
//!
//! The app's `Core` object calls three functions: `start` the worker, queue a `command`, and
//! `stop` it. The worker calls the app back on its own thread, through the `Platform` object
//! `start` was given (`kotlin.rs`): to list the paired devices, to use the RFCOMM link, and to
//! report events and log lines. Commands and events cross as JSON (`json.rs`). Nothing here knows
//! the buds' protocol.
//!
//! The exports are the second exception to rule 5 (CLAUDE.md): the VM finds a native method by
//! its exported name, and exporting one is an unsafe attribute. Nothing else here is unsafe.

mod json;
mod kotlin;
mod phone;

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use budctl_core::debug;
use budctl_core::worker::{self, Command};
use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jlong};
use jni::{Env, EnvUnowned};

use kotlin::Kotlin;
use phone::{Phone, Reporter};

/// The running worker's inbox. Dropping it ends the worker, which closes any session.
static WORKER: Mutex<Option<Sender<Command>>> = Mutex::new(None);

/// `Core.start(platform, idleMs, debug)`: starts the worker, in place of any running one. Returns
/// null, or why it could not start.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_krolyxon_budctl_Core_start<'caller>(
    mut unowned: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    platform: JObject<'caller>,
    idle_ms: jlong,
    debug: jboolean,
) -> JString<'caller> {
    unowned
        .with_env(|env| -> jni::errors::Result<_> {
            let phone: Arc<dyn Phone> = Arc::new(Kotlin::new(env.new_global_ref(&platform)?));
            if debug {
                debug::enable();
            } else {
                debug::disable();
            }
            let output = phone.clone();
            debug::set_output(Box::new(move |line| output.log(line)));

            let idle = Duration::from_millis(u64::try_from(idle_ms).unwrap_or(0));
            let connect = phone::connect(phone.clone());
            let failure = match worker::start(connect, Box::new(Reporter(phone)), idle) {
                Ok(inbox) => {
                    *WORKER.lock().unwrap_or_else(PoisonError::into_inner) = Some(inbox);
                    None
                }
                Err(e) => Some(format!("could not start the Bluetooth thread: {e}")),
            };
            answer(env, failure)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Core.command(json)`: queues one command for the worker; its event comes back through
/// `Platform.event`. Returns null, or why the command was not queued.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_krolyxon_budctl_Core_command<'caller>(
    mut unowned: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    json: JString<'caller>,
) -> JString<'caller> {
    unowned
        .with_env(|env| -> jni::errors::Result<_> {
            let failure = match json::command(&json.try_to_string(env)?) {
                Ok(command) => queue(command).err(),
                Err(e) => Some(e.to_string()),
            };
            answer(env, failure)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Core.stop()`: ends the worker, which closes any session once the command at hand is done.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_io_github_krolyxon_budctl_Core_stop<'caller>(
    _unowned: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) {
    let inbox = WORKER.lock().unwrap_or_else(PoisonError::into_inner).take();
    drop(inbox);
}

fn queue(command: Command) -> Result<(), String> {
    let worker = WORKER.lock().unwrap_or_else(PoisonError::into_inner);
    let inbox = worker.as_ref().ok_or("the Bluetooth thread is not running")?;
    inbox
        .send(command)
        .map_err(|_| "the Bluetooth thread has stopped".to_owned())
}

/// Null for no failure, else the message, as a Java string.
fn answer<'local>(env: &mut Env<'local>, failure: Option<String>) -> jni::errors::Result<JString<'local>> {
    match failure {
        None => Ok(JString::null()),
        Some(message) => env.new_string(message),
    }
}
