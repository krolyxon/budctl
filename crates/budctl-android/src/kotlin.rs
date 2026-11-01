//! [`Phone`] over JNI: calls into the app's `Platform` object (`Platform.kt`) from the worker's
//! thread. The thread attaches to the VM on its first call and stays attached until it ends;
//! each call gets a JNI frame of its own, so no local reference outlives it.

use std::time::Duration;

use budctl_core::{Error, Result};
use jni::objects::{JByteArray, JObject, JString};
use jni::refs::Global;
use jni::strings::JNIStr;
use jni::vm::JavaVM;
use jni::{Env, JValue, jni_sig, jni_str};

use crate::json;
use crate::phone::{Phone, app_error};

pub struct Kotlin {
    platform: Global<JObject<'static>>,
}

impl Kotlin {
    pub fn new(platform: Global<JObject<'static>>) -> Self {
        Kotlin { platform }
    }

    /// Runs `call` on this thread's JNI environment. A Java exception or a JNI failure becomes an
    /// app error about `what`.
    fn call<T>(
        &self,
        what: &'static str,
        call: impl FnOnce(&mut Env, &JObject<'static>) -> jni::errors::Result<T>,
    ) -> Result<T> {
        let vm = JavaVM::singleton().map_err(|e| app_error(what, e))?;
        vm.attach_current_thread(|env| call(env, &self.platform))
            .map_err(|e| app_error(what, e))
    }

    /// Calls a `(String) -> Unit` method, for reports: a failure has nowhere to go.
    fn tell(&self, method: &'static JNIStr, text: &str) {
        let _ = self.call("reporting to the app", |env, platform| {
            let text = env.new_string(text)?;
            env.call_method(
                platform,
                method,
                jni_sig!((text: java.lang.String) -> void),
                &[JValue::Object(&text)],
            )?
            .v()
        });
    }
}

/// A Kotlin `String?` as a Rust string.
fn string(env: &mut Env, object: JObject) -> jni::errors::Result<Option<String>> {
    if object.is_null() {
        return Ok(None);
    }
    let string = env.cast_local::<JString>(object)?;
    Ok(Some(string.try_to_string(env)?))
}

impl Phone for Kotlin {
    fn paired_devices(&self) -> Result<String> {
        let list = self.call("listing the paired devices", |env, platform| {
            let list = env
                .call_method(
                    platform,
                    jni_str!("pairedDevices"),
                    jni_sig!(() -> java.lang.String),
                    &[],
                )?
                .l()?;
            string(env, list)
        })?;
        Ok(list.unwrap_or_default())
    }

    fn connect(&self, address: &str, service: &str) -> Result<()> {
        let failure = self.call("connecting to the control channel", |env, platform| {
            let address = env.new_string(address)?;
            let service = env.new_string(service)?;
            let failure = env
                .call_method(
                    platform,
                    jni_str!("connect"),
                    jni_sig!((address: java.lang.String, service: java.lang.String) -> java.lang.String),
                    &[JValue::Object(&address), JValue::Object(&service)],
                )?
                .l()?;
            string(env, failure)
        })?;
        match failure {
            None => Ok(()),
            Some(json) => Err(json::connect_failure(&json)),
        }
    }

    fn send(&self, bytes: &[u8]) -> Result<()> {
        let failure = self.call("sending to the buds", |env, platform| {
            let bytes = env.byte_array_from_slice(bytes)?;
            let failure = env
                .call_method(
                    platform,
                    jni_str!("send"),
                    jni_sig!((bytes: jbyte[]) -> java.lang.String),
                    &[JValue::Object(&bytes)],
                )?
                .l()?;
            string(env, failure)
        })?;
        match failure {
            None => Ok(()),
            Some(message) => Err(Error::NotConnected(format!("sending to the buds: {message}"))),
        }
    }

    fn recv(&self, timeout: Duration) -> Result<Option<Vec<u8>>> {
        // Rounded up, so a wait shorter than a millisecond still waits.
        let millis = i64::try_from(timeout.as_micros().div_ceil(1000)).unwrap_or(i64::MAX);
        self.call("reading from the buds", |env, platform| {
            let bytes = env
                .call_method(
                    platform,
                    jni_str!("recv"),
                    jni_sig!((timeout_ms: jlong) -> jbyte[]),
                    &[JValue::Long(millis)],
                )?
                .l()?;
            if bytes.is_null() {
                return Ok(None);
            }
            let bytes = env.cast_local::<JByteArray>(bytes)?;
            Ok(Some(env.convert_byte_array(&bytes)?))
        })
    }

    fn close(&self) {
        let _ = self.call("closing the link", |env, platform| {
            env.call_method(platform, jni_str!("close"), jni_sig!(() -> void), &[])?
                .v()
        });
    }

    fn event(&self, json: &str) {
        self.tell(jni_str!("event"), json);
    }

    fn log(&self, line: &str) {
        self.tell(jni_str!("log"), line);
    }
}
