# Changelog

## Unreleased

- `budctl bass` sets bass boost band by band: `budctl bass low|medium|high <level>`. The buds
  ignored the old `budctl bass <level>`: they need to know which band the level is for.

## 0.1.4 (2026-11-01)

budctl comes to Android, and gets an icon.

- An Android app: the desktop app's pages and look on a phone (Android 12 or newer), with the same
  Rust library inside, colors from your wallpaper, and no internet permission. Install `budctl.apk`
  from this release. It is signed with budctl's own key; the certificate's SHA-256 fingerprint is
  `45:6F:9B:93:AA:40:1D:57:AA:6C:75:0D:FF:13:FA:82:5A:7B:28:B6:3D:D0:40:99:80:6F:A9:7C:14:75:23:12`.
- budctl has an icon: the LED battery ring around a dot-matrix "b". The release archives carry it,
  and `install.sh` and the PKGBUILD install it for the launcher entry.
- With `--debug`, `budctl-gui` also shows each command, connect and failure between the frames.

## 0.1.3 (2026-11-01)

The desktop app moves, the Nothing way: short, eased out, fading rather than sliding.

- Hovers, the chosen segment, the keyboard cursor and the current preset's circle fade instead
  of snapping. Switches stretch from the knob into the fill as they turn.
- Pages fade in when opened, and when the buds are first read; dialogs fade in and out, as do the
  error above a page and the bands of a custom preset.
- The battery rings light their dots one after the other, like the Glyph lights, when they first
  show and when a level changes.
- While the app looks for the buds, a row of dots scans and the sidebar's light blinks; a light
  blinks beside Stop while the buds ring.
- Nothing moves once a change has settled, so an idle window uses no CPU. `"gui": {"motion":
  false}` in the config turns the animations off.

## 0.1.2 (2026-10-29)

A new look for the desktop app, after Nothing OS in Material You colors, and a window that no
longer lags.

- `budctl-gui` has a new look, after Nothing OS in Material You colors, like the owner's Aurora
  shell: dot-matrix page names and battery figures (Doto), LED battery rings, dotted sliders for
  the bass level and the EQ bands (drag, or the mouse wheel), pill switches, tracks and chips,
  Space Grotesk and Space Mono text. The fonts are built in (SIL OFL 1.1, `OFL-fonts.txt` in the
  release archive).
- The palette file takes Aurora's Material 3 keys (`primary`, `tertiary`, `surfaceLow`,
  `outline`…), so `~/.cache/matugen/aurora.json` works as it is. `bg` and `fg` are still read;
  `surface` and `secondary` are no longer used.
- `budctl-gui` no longer lags. It repainted most of the window many times over for each hover,
  click or scroll step; now it repaints only what changed, and a scroll paints each card once.
  On the Overview, a hover went from about 25 ms of drawing to 2, a scroll step from about 50
  to at most 12, and the list of keys from 35 to almost nothing.
- The keys that need Shift work in `budctl-gui`: `?` (the list of keys), `G`, `J`, `K` and `gT`.
  The window used to see them without Shift (`/`, `g`, `j`, `k`, `gt`).
- Built from source, debug builds (`cargo run`) optimize the dependencies, so the desktop app
  runs smoothly there too.

## 0.1.1 (2026-10-28)

The first release, with builds for x86_64 and aarch64 Linux and an install script.

Supported buds: realme Buds T310.

- `budctl`, the command line: `list`, `status`, noise control and its levels, game mode, EQ
  presets and custom EQ presets (create, edit, rename, delete), switches (bass boost, 3D sound,
  wind, dual connection), bass level, find (ring the buds), touch controls, the hold cycle, and
  the devices dual connection keeps. `--json` for scripts and status bars, `--debug` for hex
  dumps.
- `budctl-gui`, the desktop app (Wayland): the same features on five pages, mouse first with
  vim-like keys on top, colors from a matugen palette.
- `~/.config/budctl/config.json`: a default device, and the desktop app's idle time and start
  page. Created with the defaults when missing.
- The desktop app writes its built-in colors to `~/.config/budctl/theme.json` when there is no
  such file, and logs errors and session events to `~/.local/state/budctl/gui.log`.
