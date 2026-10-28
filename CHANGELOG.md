# Changelog

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
