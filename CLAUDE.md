# budctl

budctl is a Linux CLI (and later a GUI) for controlling wireless earbuds without the vendor app. It has no telemetry, needs no account and never makes a network call.

The first supported device is the **realme Buds T310**. More devices are added as plugins when the owner gets new earbuds.

## Goals

- Control earbud features from Linux: ANC / transparency / off, EQ presets, game mode, touch controls, battery readout, and anything else the buds offer.
- Two front ends, one core: a `budctl` CLI and later a `budctl-gui`, both thin layers over the same library (`budctl-core`). The GUI is undecided. It will be either a Quickshell/Aurora integration (which can use `budctl --json`) or a standalone Rust GUI.
- One small, readable codebase that the owner fully understands.
- Device support is pluggable. Adding a new pair of buds means adding one module under `crates/budctl-core/src/devices/` and registering it in `devices/mod.rs`, without touching transport, CLI or GUI code.
- Coexist with the phone app. Settings live on the buds, so budctl never stores device state locally beyond user config.

## Non-goals

- No firmware dumping, flashing or modification. We only speak the buds' existing control protocol.
- No network access of any kind: no update checks, crash reports or analytics.
- No support for models the owner doesn't have. Don't speculatively add devices.
- No Windows or macOS support.

## Hard rules

1. **No network code.** Nothing in this workspace may open a socket except a Bluetooth socket (RFCOMM for the control link, L2CAP for SDP).
   - The one exception, approved by the owner: the local system D-Bus Unix socket, used through `zbus` only to talk to BlueZ (`org.bluez`). budctl opens that socket itself at the fixed path, so an environment variable can never redirect it to TCP.
   - No HTTP clients (reqwest, hyper, ureq, surf, etc.) may appear in the dependency tree, direct or transitive. This is enforced with `cargo deny` (`deny.toml`), clippy `disallowed-types`, and `crates/budctl-core/tests/guards.rs`.
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
  - `crates/budctl-gui` will come later.
- Bluetooth goes through BlueZ:
  - Raw `AF_BLUETOOTH` sockets via `libc`: RFCOMM for the control link, L2CAP to read the buds' SDP record.
  - `zbus` (blocking API) only to list paired devices.
  - Supporting BLE GATT would need a new transport kind in core.
- CLI: `clap` (derive). It is parsed with `try_parse`, so usage errors exit 1, not clap's default 2. `serde_json` provides `--json`.
- Tests: `cargo test --workspace`. Lints: `cargo clippy --workspace --all-targets -- -D warnings`. Dependency policy: `cargo deny check bans licenses sources`.
- Keep dependencies minimal. Every new dependency needs a reason in the commit message. The current direct dependencies:
  - core: `libc`, `zbus`, `serde`
  - cli: `clap`, `serde_json`

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
- CLI commands connect, act and disconnect. The GUI must also release the channel when idle rather than hold it open.
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
4. **GUI**: Quickshell/Aurora via `--json`, or a Rust GUI on top of the same core.
5. **More devices**: as acquired.

## Working with Claude

- Claude has no access to the buds. Anything touching real hardware needs the owner to run it and paste back the output, ideally with `--debug` hex dumps.
- Each hardware checkpoint goes into `docs/protocol/realme_t310.md#verification-log`, with the date and the evidence level ("ours-YYYY-MM-DD").
- When the spec is unclear, ask for a capture rather than guessing:
  - `sudo btmon -w file.btsnoop` on Linux shows budctl's own traffic.
  - An Android "Bluetooth HCI snoop log" shows what realme Link sends.
- Keep the encode/decode logic pure and covered by tests, so most changes can be verified without hardware.
- Before adding a feature, check the spec says the T310 supports it. The protocol family covers many models, and not every command applies to this one. The T310's command bitmap (spec §5) is the authority: never send a command whose bit is off.

## License

GPL-3.0-or-later (`LICENSE`). Rule 2 forbids copying other projects' code.
