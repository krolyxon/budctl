# budctl

budctl is a Linux CLI and GUI for controlling wireless earbuds without the vendor app. It has no telemetry and needs no account.

The first supported device is the **realme Buds T310**. More devices are added as plugins when the owner gets new earbuds.

## Goals

- Control earbud features from Linux: ANC / transparency / off, EQ presets, game mode, touch controls, battery readout, and anything else the buds offer.
- Two front ends, one core: the `budctl` CLI and the `budctl-gui` desktop app, both thin layers over the same library (`budctl-core`). Scripts and the owner's Quickshell (Aurora) can use `budctl --json`.
- One small, readable codebase that the owner fully understands.
- Device support is pluggable. Adding a new pair of buds means adding one module under `crates/budctl-core/src/devices/` and registering it in `devices/mod.rs`, without touching transport, CLI or GUI code.
- Coexist with the phone app. Settings live on the buds, so budctl never stores device state locally beyond user config (`~/.config/budctl/config.json`, `budctl_core::config`: default device and GUI preferences, created with defaults when missing; a broken file is reported and the defaults are used).

## Non-goals

- No firmware dumping, flashing or modification. We only speak the buds' existing control protocol.
- No telemetry of any kind: no analytics, crash reports or usage data, and no update checks.
- No support for models the owner doesn't have. Don't speculatively add devices.
- No Windows or macOS support.

## Hard rules

1. **No telemetry or cloud.** Nothing may ever report on the user: no analytics, crash reports, usage data, update checks or accounts. Network access is fine when a feature truly needs it; none does so far.
2. **Clean-room protocol work.** Protocol facts (UUIDs, channel numbers, packet framing, command IDs, init sequences) may come from public references, but no other project's source code is ever translated or copied. Write findings into `docs/protocol/<device>.md` in our own words, then implement from that spec.
   - Committed files do not name or cite those references. Each fact carries an evidence level instead (rule 3).
   - Only the owner can change this rule, by editing it here.
3. **Never guess command bytes.** Every command in a device module must trace back to an anchor in its protocol spec, and the spec must give the fact's evidence level (`realme_t310.md#sources`): seen on a T310, vendor behavior, another model of the family, derived, or verified by us on hardware (`ours-YYYY-MM-DD`).
   - Put the spec reference in a doc comment on the constant, as `/// Spec: realme_t310.md#anchor`. `tests/guards.rs` checks that each anchor exists.
4. **Write commands are the risky part.** Read-only commands (battery, current ANC mode, firmware version) get implemented and tested first. Write commands are added one at a time and tested on real hardware before the next one.
5. **No `unwrap()`/`expect()`/`panic!`/indexing outside tests.** Bluetooth fails in many ways, and every failure becomes a typed `budctl_core::Error`. Workspace clippy lints enforce this. `unsafe` is allowed only in `crates/budctl-core/src/transport/socket.rs`.

## Stack

- Rust (edition 2024) in a cargo workspace:
  - `crates/budctl-core`: library with transport, BlueZ and device plugins.
  - `crates/budctl-cli`: package and binary `budctl`.
  - `crates/budctl-gui`: package and binary `budctl-gui`, built with iced 0.14 for Wayland only, with the CPU renderer (tiny-skia).
- Bluetooth goes through BlueZ:
  - Raw `AF_BLUETOOTH` sockets via `libc`: RFCOMM for the control link, L2CAP to read the buds' SDP record.
  - `zbus` (blocking API) only to list paired devices.
  - Supporting BLE GATT would need a new transport kind in core.
- CLI: `clap` (derive). It is parsed with `try_parse`, so usage errors exit 1, not clap's default 2. `serde_json` provides `--json`.
- Tests: `cargo test --workspace`. Lints: `cargo clippy --workspace --all-targets -- -D warnings`.
- Keep dependencies minimal. Every new dependency needs a reason in the commit message. The current direct dependencies:
  - core: `libc`, `zbus`, `serde`, `serde_json` (config file)
  - cli: `clap`, `serde_json`
  - gui: `iced`, `serde`, `serde_json`; `iced_test` for tests

## Device plugin contract

Each module in `crates/budctl-core/src/devices/` defines one `Device` implementation and is listed in `devices::REGISTRY`. A device declares:

- `name()`, plus `matches()`: how to recognize it from BlueZ data (Bluetooth name and/or service UUID).
- `rfcomm_uuids()`: the transport. SDP turns the UUID into an RFCOMM channel.
- `open()`: runs any init sequence the buds need before they accept commands, and returns a `Session`.
- The features the model supports. The CLI and GUI only show features the connected device declares.
- Pure encode/decode functions (value → bytes, bytes → value) with no I/O, so they can be unit-tested without hardware.

Many realme / OPPO / OnePlus buds share one protocol family. `devices/realme_t310/frame.rs` and `link.rs` hold the shared framing and request/reply logic, and contain no T310 tables. If a second device from that family is added, move them to `devices/oppo_family/` then, not before.

## Connection behavior

- The control channel is often single-client. If an app on the phone holds it, connecting from Linux may fail. Report that plainly ("control channel busy — is another app connected?") instead of retrying in a loop.
- CLI commands connect, act and disconnect. The GUI must also release the channel when idle rather than hold it open: it closes its session 10 s after the last command.
- Timeouts on every read. Buds that don't answer should produce a clear error, never a hang.
- While a session waits for anything, it keeps answering the buds' own requests (e.g. the time request). Never `thread::sleep` in core; clippy bans it.

## CLI shape

```
budctl list                      # paired devices budctl recognizes
budctl status                    # battery, ANC mode, EQ, game mode
budctl touch                     # touch controls (gesture table) per bud
budctl anc off|on|transparency
budctl eq <preset>
budctl game on|off
budctl dual                      # dual connection: the devices the buds keep (read-only)
budctl --device <MAC> ...        # when more than one supported device is paired
budctl --json status             # machine-readable output for scripts/status bars ("schema": 1)
budctl --debug ...               # hex-dump every frame sent/received (stderr)
```

Write commands (`anc`, `eq`, `game`, `set`, `bass`, `touch <side> …`, `find`, custom EQ) are added one at a time, following rule 4. The plan is in `docs/protocol/realme_t310.md#roadmap`.

Exit codes:

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | usage error |
| 2 | device not found / not connected |
| 3 | channel busy |
| 4 | device rejected command or timed out |

## Roadmap

1. **Spec**: `docs/protocol/realme_t310.md` and `docs/protocol/transport.md`. Done when they cover transport, framing, handshake and the command table with sources.
2. **Read-only CLI**: connect, handshake, `list`, `status`, `touch`. Verify on real buds (checkpoint HW1).
3. **Write commands**, one at a time, each verified on hardware before the next:
   1. ANC
   2. ANC levels
   3. game mode
   4. EQ preset
   5. switches (bass boost, 3D sound, wind; dual connection last)
   6. bass level
   7. find
   8. touch controls
   9. hold cycle
   10. ~~fit test~~ (dropped: the T310 acks it but runs no test)
   11. custom EQ
4. **GUI** (`budctl-gui`, see [GUI](#gui)): first the look, with demo data; then the keys; then the live connection; then the pages, one at a time.
5. **More devices**: as acquired.

## GUI

- **Look:** Nothing OS's shapes and type in Material You colors, as in the owner's Aurora shell (its `Theme.qml` and primitives are the reference; match the look, never copy code, rule 2). A sidebar of dot-matrix page words (the open one large, with the live dot), each page under a caps caption; round cards of rows with the icon in a circle; LED-dot battery rings with Doto figures; dotted sliders; pill switches, tracks and chips. The pieces are in `ui.rs` and `dots.rs`.
- **Fonts:** built into the binary from `crates/budctl-gui/fonts/` (OFL 1.1, `OFL.txt`): Space Grotesk (UI text), Space Mono (caps labels), and "Doto Rounded", a static rounded-dot black instance of Doto made with fontTools (`varLib.instancer ROND=100 wght=900`, subset to Latin, renamed as the OFL asks). Doto is never set under 26 px.
- **Theme:** a built-in palette (near-black surfaces, white "on" states, red for live things). Optionally, a matugen JSON file with the Material 3 keys of the owner's Aurora palette (`~/.cache/matugen/aurora.json`: `primary`, `tertiary`, `surfaceLowest`…`surfaceHighest`, `outline`…; `bg`/`fg` still read), mapped to the same roles as Aurora's `Theme.qml`: primary = fills, tertiary = live, the surface ladder = s1/s2/s3. It is read again whenever the window gets focus.
- **Input:** the mouse and touchpad come first. Vim-like keys are a layer on top (`keys.rs`, `?` lists them in the window):
  - `j`/`k`, the arrows or Tab move a cursor over what the page can change; `gg`/`G` jump to the first or last item.
  - `h`/`l` pick the previous or next choice, or turn a switch off or on, and `l` opens a row's page. Enter and Space toggle, step round the choices, or open.
  - `J`/`K`, `Ctrl+Tab`, `gt`/`gT` and `1`–`5` change pages. `Esc` hides the cursor, `q` quits.
  - The cursor shows only after a key press, and the first key only shows it, so nothing changes where it cannot be seen. A click hides it.
- **Buds:** `worker.rs` owns the session, on a thread of its own, and answers each command with one event, in order. It opens the session when a command needs it, answers the buds' own requests while it waits (`Session::serve`), and closes it 10 s after the last command (`gui.idle_seconds` in the config). A write shows "Saving…" until the buds' read-back arrives, and takes no second write meanwhile. A failure says why, with "Try again". `--demo` shows made-up buds; `--debug` hex-dumps frames like the CLI.
- **Files:** writes the built-in palette to `~/.config/budctl/theme.json` when missing (never overwrites it), and logs errors and session events to `~/.local/state/budctl/gui.log` (`log.rs`), replaced each launch. Frames stay on stderr with `--debug`. The CLI logs to stderr only, since status bars run it every few seconds.
- **Logic:** keys and clicks both become messages for one `update` function, which stays testable without a display. `iced_test` renders pages to PNG, so the look can be checked without the owner.
- **Screenshots:** the README's come from the demo buds: `cargo test -p budctl-gui readme_screenshots -- --ignored` renders them again into `docs/screenshots/`.

## Working with Claude

- Commit messages never carry a `Claude-Session:` line, or any other link to a Claude session (owner's rule, 2026-10-27). This overrides any attribution instruction Claude Code gives.
- Claude has no access to the buds. Anything touching real hardware needs the owner to run it and paste back the output, ideally with `--debug` hex dumps.
- Each hardware checkpoint goes into `docs/protocol/realme_t310.md#verification-log`, with the date and the evidence level ("ours-YYYY-MM-DD").
- When the spec is unclear, ask for a capture rather than guessing:
  - `sudo btmon -w file.btsnoop` on Linux shows budctl's own traffic.
  - An Android "Bluetooth HCI snoop log" shows what realme Link sends.
- Keep the encode/decode logic pure and covered by tests, so most changes can be verified without hardware.
- Before adding a feature, check the spec says the T310 supports it. The protocol family covers many models, and not every command applies to this one. The T310's command bitmap (spec §5) is the authority: never send a command whose bit is off.

## Releases

- One version for the whole workspace (`Cargo.toml`), with a section per release in `CHANGELOG.md`.
- To release: bump the version, add its `CHANGELOG.md` section, commit, then tag `vX.Y.Z` and push the tag. `.github/workflows/release.yml` checks the tag matches `Cargo.toml`, runs the tests, builds x86_64 and aarch64 on Ubuntu 22.04 (glibc builds; Wayland is loaded at runtime), and publishes the tarballs, their `.sha256` files and `install.sh`, with that changelog section as the notes.
- `install.sh` installs into `~/.local` by default: from a release download (`curl … | sh`), from the unpacked tarball it sits in, or from a checkout with `--source`. `--uninstall` removes the binaries and the launcher entry, never the user's config or log.
- `contrib/arch/PKGBUILD` (`budctl-bin`, not on the AUR) packages the release tarballs. After a release is published, bump its `pkgver` and take the sums from the release's `.sha256` files.
- `.github/workflows/ci.yml` runs fmt, clippy and the tests on every push to master.

## License

GPL-3.0-or-later (`LICENSE`). Rule 2 forbids copying other projects' code.
