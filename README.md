# tpt-monitor

A beautiful, customisable system resource monitor — a clean alternative to
Task Manager that doesn't ask you to be a Process Hacker expert.

Shows CPU, GPU, RAM, disk and network usage per application, with **smart
suggestions** such as "this background process is using 20% CPU and can be
safely suspended". Suggestions are advisory: nothing is applied without a click
and a confirmation for destructive actions.

Built by **TPT Solutions**.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
![Status: early development](https://img.shields.io/badge/status-early%20development-orange.svg)

## Status

Early development. The roadmap and per-phase progress are tracked in
[todo.md](todo.md).

Implemented so far:

- **Core** (`core/`): metrics model, backend traits, process-action confirmation
  gate, adaptive sampler, ring-buffer history, and the suggestion engine.
- **Windows backend** (`platform-windows/`): process list, per-process CPU, memory
  and disk I/O, visible-window detection, per-core CPU, and process actions.
- **Linux backend** (`platform-linux/`): the same metrics from `/proc`, hwmon
  sensors, and process actions via `libc`. Compiled for Linux, but not yet run on
  a Linux host.
- **Tauri shell** (`src-tauri/`): sampling loop, throttled `metrics` events, and
  IPC commands with strict input validation. Layouts are saved as JSON in the app
  data folder.
- **Dashboard** (`src/`): Svelte 5 + Vite, gridstack.js grid with CPU, memory,
  sensors, process table, suggestions, and disk/network/GPU panels. Light and dark
  themes.

- **Desktop overlays and tray**: any widget can pop out into an always-on-top,
  transparent window with opacity, click-through and dock-back controls. The tray
  shows a live CPU graph and the busiest processes.

Not yet built: network and GPU data, per-drive I/O, per-monitor overlay placement,
RAM in the tray graph, and the safe/critical list viewer. See `todo.md` for the full
list.

## Platforms

| Platform | Target |
|---|---|
| Windows 10 / 11 | v1 |
| Linux (X11 and Wayland) | v1 |
| Archon | design only — see [docs/archon-design.md](docs/archon-design.md) |

## Building

Requirements:

- Rust (the exact version is pinned in `rust-toolchain.toml`; `rustup` installs it automatically)
- Node.js 20.19+ or 22.12+ with npm (Vite 8 requires it)

```sh
git clone https://github.com/tpt-solutions/tpt-monitor
cd tpt-monitor
cargo test --workspace
npm install
npm test
npm run build
```

Frontend commands: `npm run dev` (with `cargo run -p tpt-monitor-tauri` in another
terminal), `npm run check` for type checking, `npm test` for unit tests.

Known npm issue on this machine (npm 10.8): a clean `npm install` can fail with
`Cannot read properties of null (reading 'edgesOut')`, and Vite's Rolldown
native binding may be missing. Workaround: `npm install --legacy-peer-deps`, then
`npm install --no-save --legacy-peer-deps @rolldown/binding-<your-platform>@1.2.16`.

On Windows the app requests administrator rights at launch (UAC). Running
`cargo run -p tpt-monitor-tauri` therefore prompts for elevation. Linux needs the
WebKitGTK development packages; see `.github/workflows/ci.yml` for the list.

## Repository layout

| Path | Contents |
|---|---|
| `core/` | Metrics model, `SystemMonitor` / `SensorProvider` traits, action gate, sampler, suggestion engine |
| `platform-windows/` | Windows backend (Win32 and NT calls; `sys.rs` is the only module with `unsafe`) |
| `platform-linux/` | Linux backend (`/proc`, `/sys`, `libc`); `parse.rs` is platform-neutral |
| `src-tauri/` | Tauri shell: sampling thread, IPC commands, DTOs, admin manifest |
| `src/` | Svelte 5 dashboard: `lib/` holds pure, tested logic; `lib/widgets/` the cards |
| `docs/` | Design notes |
| `spec.txt` | Original product specification |
| `todo.md` | Roadmap and checklist |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development checks and the
licence-header convention.

## Licence

Dual-licensed under either of

- MIT licence ([LICENSE-MIT](LICENSE-MIT))
- Apache Licence, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
