# Android app

budctl on the owner's phone, so the phone needs no other app to control the buds. It is a Kotlin
app with budctl-core inside it: the same protocol code, tests and spec as the CLI and the desktop
app.

This file is the plan. Each step says when it is done. The phone checkpoints go into the
[verification log](protocol/realme_t310.md#verification-log), like the desktop app's.

## Why Kotlin as well as Rust

- Android has no BlueZ, and apps cannot open the kernel Bluetooth sockets that
  `transport/socket.rs` uses. An app reaches RFCOMM only through Android's `BluetoothSocket`, a
  Java API.
- iced does not run on Android.
- So Kotlin does what only Android can do, Bluetooth and the screen. Rust does everything that
  knows the buds.

## What runs where

```
android/                          Kotlin, built with Gradle
  ui/           Compose: the desktop app's pages, in a phone layout
  Bluetooth.kt  paired devices; a BluetoothSocket as send / recv(timeout)
  Core.kt       the Rust library: commands in, events out, as JSON
        │ JNI
crates/budctl-android             cdylib: the JNI exports, a Transport over Bluetooth.kt
        │
crates/budctl-core                the protocol, unchanged: frame, link, codec, devices, worker
```

- **budctl-core** builds for Android as it is. BlueZ, the sockets and SDP are Linux-only
  (`cfg(target_os = "linux")`). Android's target OS is `android`, not `linux`.
- **The worker** (`budctl_core::worker`) is the thread that owns the session. It answers each
  command with one event, answers the buds' own requests while it waits, and closes the session
  10 s after the last command. The desktop app runs it as an iced subscription, and the Android
  app runs the same one.
- **budctl-android** turns JSON into worker commands and worker events into JSON, and wraps
  Kotlin's Bluetooth link as a `Transport`. It holds no protocol logic.
- **Kotlin** lists the paired devices, opens the RFCOMM socket and draws the pages. It holds no
  protocol logic either: no command bytes and no tables.

<a id="boundary"></a>
## The boundary

Rust exports, called from Kotlin (`Core.kt`):

| Function | Does |
|---|---|
| `start(platform, idleMs, debug): String?` | starts the worker, in place of any running one; `platform` is Kotlin's side, below. Null, or why it could not start |
| `command(json): String?` | queues one command; its event comes back through `platform.event`. Null, or why it was not queued |
| `stop()` | ends the worker, which closes any session |

Kotlin's side (`Platform`), called on the worker's thread. It catches its own exceptions and
answers with values:

| Method | Does |
|---|---|
| `pairedDevices(): String` | the paired devices as JSON: address, name, alias, connected, service UUIDs |
| `connect(address, service): String?` | opens the RFCOMM link to that service, in place of any open one. Null, or `{"kind", "message"}` with the kind `busy`, `not-connected`, `timeout` or `failed` |
| `send(bytes): String?` | null, or why the link failed |
| `recv(timeoutMs): ByteArray?` | the next bytes; null once the time runs out; an empty array once the link is closed |
| `close()` | closes the link |
| `event(json)` | one worker event, for the UI |
| `log(line)` | a session or debug line, for logcat and the in-app log |

Commands look like `{"command": "noise", "mode": "anc", "level": "deep"}` and events like
`{"event": "noise", "mode": "anc", "level": "deep"}`. `crates/budctl-android/src/json.rs` lists
them all.

- JSON keeps the boundary small, and it carries the same serde types the CLI's `--json` writes.
- Rust picks the device with `budctl_core::select`, as the CLI does, so the messages ("not
  connected", "pick one") are the same.
- The `jni` crate, not UniFFI: one dependency and no code generator.

<a id="bluetooth"></a>
## Bluetooth on Android

Written from Android's API reference. No other app's code is read for it (rule 2).

- **Permission:** `BLUETOOTH_CONNECT`, asked for on first start. minSdk is 31 (Android 12), where
  that permission begins.
- **No `INTERNET` permission.** Android then enforces rule 1: the app cannot open a network
  socket. A build check fails if a library adds the permission to the merged manifest.
- **Paired devices:** `BluetoothAdapter.getBondedDevices()` gives the name, alias, address and
  cached service UUIDs. Whether the buds are connected comes from the A2DP and headset profile
  proxies.
- **Connect:** `createRfcommSocketToServiceRecord(uuid)`, for the first of the model's
  `rfcomm_uuids()` that the buds list (Android caches their service UUIDs; with none cached yet,
  the first one). Android runs the SDP lookup that `sdp.rs` does on Linux. `connect()` takes no
  timeout, so a watchdog closes the socket after 8 s, when `rfcomm.rs` gives up too.
- **Reads with a timeout:** the socket's input stream has no read timeout. A reader thread puts
  what it reads in a queue, `recv(timeoutMs)` waits on the queue, and `close()` ends the thread.
  The end of the stream means the buds closed the link (case lid closed?).
- **Errors:** Android reports a failed connect as a plain `IOException`, with no errno. When the
  buds are not connected to the phone, it is `not-connected`. When they are and RFCOMM still
  fails, it is `busy` ("control channel busy — is another app connected?").
- **Letting go of the channel:** the session closes 10 s after the last command, as on the
  desktop, and also when the app leaves the screen (`onStop`). There is no background service, so
  the laptop is never locked out for long.
- **Debug:** a switch in Settings turns on the hex dump. `budctl_core::debug` gets a pluggable
  output (stderr by default). On Android it goes to logcat and to an in-app log that can be
  copied, so a checkpoint's output can be pasted straight from the phone.

<a id="steps"></a>
## Steps

0. **Toolchain:** Android Studio, which brings the SDK (`~/Android/Sdk`), the emulator and a JDK
   for Gradle (`/opt/android-studio/jbr`). The NDK, from Studio's SDK Manager.
   `rustup target add aarch64-linux-android x86_64-linux-android` and `cargo install cargo-ndk`.
   *Done when* the NDK is in `~/Android/Sdk/ndk` and `adb devices` lists the emulator. The phone
   joins before A1, over USB debugging (`android-udev` gives adb access to it without root).
1. **The core builds for Android.** `bluez`, `transport::{rfcomm, sdp, socket}`, `scan` and
   `connect` go behind `cfg(target_os = "linux")`, and `libc` and `zbus` become Linux-only
   dependencies. `PairedDevice` moves to the crate root, since Android fills it too. CI checks the
   core for `aarch64-linux-android`.
   *Done when* `cargo clippy -p budctl-core --target aarch64-linux-android -- -D warnings` passes
   and nothing changes on Linux.
2. **The worker moves into core** (`budctl_core::worker`), with its tests. A `Report` trait takes
   its events and log lines. The desktop app's `worker.rs` keeps only the iced subscription and
   the BlueZ connect.
   *Done when* the desktop app behaves the same and all tests pass.
3. **`crates/budctl-android`:** the JNI exports, `Deserialize` for the commands and `Serialize`
   for the events, the `Transport` over Kotlin's link, and the pluggable debug output. Rule 5
   needs an exception first (see [Decisions](#decisions)).
   *Done when* `cargo ndk -t arm64-v8a build -p budctl-android` builds and the crate's tests
   (JSON round trips, a scripted link) pass on Linux.
4. **The Android project** (`android/`): Gradle, with `cargo ndk` run from Gradle, `Bluetooth.kt`,
   `Link.kt`, `Core.kt`, and a plain screen with the status as text, the noise-control choice and
   the debug log. JVM tests cover the link's reader thread and timeouts. Build and install with
   `cd android && ./gradlew assembleDebug` and
   `adb install -r app/build/outputs/apk/debug/app-debug.apk`.
   *Done when* **A1** and **A2** pass on the phone.
5. **The pages**, built the way the desktop app was: first the look with demo buds, then live
   buds, then one page at a time (Overview, Equalizer, Gestures, Earbuds, Settings).
   - The look is the desktop app's: Nothing OS shapes and type in Material You colors. The colors
     come from Android's dynamic color (the phone's wallpaper palette), mapped to the same roles;
     a switch on the APP page puts the built-in near-black palette back.
   - The same fonts, from `crates/budctl-gui/fonts` (Gradle adds that folder to the assets). Icons,
     battery rings, dotted sliders and pill switches are drawn on Compose canvases, with
     `motion.rs`'s timings.
   - The desktop's sidebar becomes a row of dot-matrix page words above the page. A swipe changes
     the page, and the pages fade into each other rather than slide.
   - Touch only: no vim keys. The APP page keeps the debug switch and the log, with a copy
     button.
   - Claude checks the look on the emulator with the demo buds, from adb screenshots: launch with
     `--ez demo true`.
   - Built: the look and every page of the desktop app (Overview, Equalizer, Gestures, Earbud
     settings), plus the APP page.
   *Done when* **A3** passes for every page.
6. **Release.** One version for everything: Gradle reads it from `Cargo.toml`. CI builds and
   tests the app on every push, and a tag adds a signed APK to the GitHub release. The signing key
   stays with the owner, as a CI secret. No Play Store. New versions come from the GitHub releases
   (an app such as Obtainium can watch them); budctl itself never checks.
   *Done when* a tagged release carries the APK and the phone runs it.
7. **Retire the other control app on the phone** (the owner), once **A4** passes.

<a id="emulator"></a>
## Emulator and phone

- The emulator (x86_64, from Android Studio) runs the app for everything but the buds: the build,
  the JNI library, the permission prompt, leaving and coming back, and the pages with the demo
  buds. Claude drives it over adb and takes the screenshots, so none of that waits for the owner.
- Its Bluetooth is simulated: a virtual adapter that only sees other emulated devices, never the
  buds. Everything that talks to the buds (A1–A4) runs on the phone.
- So debug builds carry x86_64 for the emulator and arm64-v8a for the phone. The release APK
  carries arm64-v8a only.

<a id="checkpoints"></a>
## Phone checkpoints

Run on the phone with debug on, and paste the in-app log.

- **A1, read-only:** the app finds the buds and shows their status. Then the busy path (another
  control app holding the channel) and the not-connected path (case lid closed).
- **A2:** noise control off → transparency → on, read back each time. The session closes 10 s
  after the last command, and when the app leaves the screen.
- **A3:** each page's writes, as G3–G8 were for the desktop app.
- **A4:** take turns between the phone app and `budctl status` on the laptop. Each gets the
  channel once the other has let go.

<a id="decisions"></a>
## Decisions

- **Taken (2026-11-01):**
  - The app lives in this repo, in `android/`.
  - Kotlin and Compose for the app, Rust for everything that knows the buds.
  - `jni` and JSON, not UniFFI.
  - arm64 for the phone and the release APK; debug builds add x86_64 for the emulator.
  - The application id is `io.github.krolyxon.budctl`.
  - Released as an APK on GitHub, not on the Play Store.
  - Rule 5's second exception: the JNI exports' `#[unsafe(no_mangle)]`, in
    `crates/budctl-android/src/lib.rs`, with nothing else unsafe in that crate.

## Not planned

- A background service, a persistent notification or a home-screen widget. The app talks to the
  buds only while it is on screen.
- Other phone architectures, Android before 12, and the Play Store.
- Later, only if wanted: a Quick Settings tile for noise control.
