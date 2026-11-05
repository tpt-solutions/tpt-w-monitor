# tpt-monitor — Project Checklist

TPT Solutions — dual-licensed under MIT OR Apache-2.0.
Beautiful, customizable system resource monitor (Process Hacker / System
Informer alternative). Rust core engine (process/resource sampling,
smart-suggestion rules) + Tauri UI (Svelte/Vite frontend, gridstack.js widget
grid). Targets Windows and Linux for v1; Archon (tpt-archon-kernel telemetry
+ capability scheduler) is design-only until the core is solid.

Reference: Archon platform — https://github.com/tpt-solutions/tpt-archon

Decisions locked in for this roadmap:
- Scope is **Task-Manager-plus**, not full Process Hacker parity: resource
  dashboard + process list + kill/suspend/resume/priority/affinity + smart
  suggestions. No handle viewer, DLL/module viewer, services manager,
  per-process network-connection list, or security-token/privilege viewer in
  v1 — backlog.
- UI is **Tauri** (Rust `src-tauri/` + Svelte/Vite frontend), with
  **gridstack.js** driving the widget grid (add/remove/resize/rearrange/save
  layout) — matches the spec's "highly customizable, widget-based dashboard"
  requirement, deviating from some siblings' plain-TS default on purpose.
- Widget model is **dashboard + desktop overlay**: widgets live in the main
  dashboard window by default, and can individually be popped out as
  always-on-top, click-through-toggle desktop overlay windows
  (Rainmeter-style).
- **Elevate by default**: the app requests admin on launch (Windows UAC
  manifest) / documents a root-launch path (Linux), matching Process
  Hacker/System Informer, for full visibility and control from the start.
- **Smart suggestions are suggest-only** in v1: rule-based heuristics plus a
  curated safe/critical process list surface one-click-apply suggestions. No
  unattended auto-apply automation rules — backlog.
- **GPU monitoring is vendor-agnostic** in v1: Windows DXGI/PDH per-process
  GPU engine + VRAM counters, Linux DRM `fdinfo` per-process GPU stats. No
  NVIDIA NVML / AMD ADL vendor-specific detail (temps, clocks, power,
  encoder) — backlog.
- **Hardware sensors (temperature/fan) are in v1**, with graceful
  "unavailable" fallback where the OS/driver doesn't safely expose them
  (notably tricky on Windows without a kernel driver).
- **History is short in-memory only** in v1 (minutes/hours, live graphs). No
  persistent multi-day store or CSV export — backlog.

---

## Status Summary

| Phase | Area | Progress |
|-------|------|----------|
| 0  | Project Setup & Licensing                      | 🟡 In Progress |
| 1  | Core Data Model & Metrics Engine                | ✅ Complete |
| 2  | Windows Backend                                 | 🟡 In Progress |
| 3  | Linux Backend                                   | 🟡 In Progress |
| 4  | Hardware Sensors (Temperature & Fan)             | 🟡 In Progress |
| 5  | Process Actions & Smart Suggestions Engine       | 🟡 In Progress |
| 6  | Tauri Shell & IPC Layer                          | 🟡 In Progress |
| 7  | Widget Dashboard Frontend                        | 🟡 In Progress |
| 8  | Desktop Overlay Widgets & Tray                   | 🟡 In Progress |
| 9  | Smart Suggestions UI                             | 🟡 In Progress |
| 10 | Archon Support (Design Only)                     | ✅ Complete |
| 11 | Performance & Polish                             | ❌ Not Started |
| 12 | Testing & QA                                     | ❌ Not Started |
| 13 | Release v1                                       | ❌ Not Started |

---

## Phase 0 — Project Setup & Licensing
- [x] Init git repo, `.gitignore` (Rust + Node/Tauri targets + Windows build artifacts)
- [x] Add `LICENSE-MIT` and `LICENSE-APACHE` (dual license, TPT Solutions copyright)
- [x] Add SPDX dual-license header/notice convention for source files (see `CONTRIBUTING.md`)
- [x] Cargo workspace (root workspace, `core/`, `platform-*/`, `src-tauri/` and `src/` Svelte frontend done): `core/` (metrics model, suggestion engine, process-action trait), `platform-windows/`, `platform-linux/`, `src-tauri/` (Tauri backend wrapping core + multi-window/overlay management), `src/` (Svelte + Vite frontend); shared `[workspace.package]` (`license`, `authors = ["TPT Solutions"]`, `repository`) 
- [x] `rust-toolchain.toml` (MSRV pin), `rustfmt.toml`, `clippy.toml`
- [x] Configure `tauri.conf.json` (app id, window defaults, bundle identifiers, `windows.requireAdministrator` manifest setting for tpt-monitor)
- [x] TypeScript config (tsconfig, Vite, Svelte), add `gridstack.js` dependency
- [x] `README.md` (from spec.txt), build instructions, license badges
- [x] Set up CI (GitHub Actions, build+lint+test on Windows + Linux runners) (workflow in `.github/workflows/ci.yml`; not yet run on GitHub)
- [ ] Repo hosting under `tpt-solutions` GitHub org

## Phase 1 — Core Data Model & Metrics Engine
- [x] Process/resource data model: per-process CPU%, RAM (working set/private/commit), GPU engine %/VRAM, disk read/write bytes+rate, network up/down rate; system-wide aggregates and per-core/per-drive/per-adapter breakdowns
- [x] Shared `trait SystemMonitor` (enumerate processes, sample per-process metrics, sample system-wide metrics, process actions) so Windows/Linux backends share one interface
- [x] `trait SensorProvider` (temperature/fan queries, `Option<T>`-returning for graceful "unavailable")
- [x] Sampling scheduler: configurable interval, adaptive slow-down when dashboard unfocused/minimized
- [x] Rolling in-memory ring buffers for short-window history (per metric, per process/system series)
- [x] Core unit tests (mocked backend implementing the traits)

## Phase 2 — Windows Backend
- [x] Process enumeration + per-process CPU/RAM via `NtQuerySystemInformation`(`SystemProcessInformation`) or `Toolhelp32Snapshot` + `GetProcessTimes`
- [x] Per-process disk I/O via `GetProcessIoCounters`
- [ ] Per-process network attribution via IP Helper API (`GetExtendedTcpTable`/`GetExtendedUdpTable` owning-PID mapping) + periodic byte-counter deltas (partial: TCP only, per-connection byte counters via ESTATS; needs an elevated run, which is not yet verified live; UDP has no byte counters and is not attributed)
- [x] Per-process GPU engine utilization + VRAM via PDH `GPU Engine`/`GPU Process Memory` counter sets (DXGI-backed, vendor-agnostic, Win10 1803+) (verified on this machine: 278 processes report engine utilisation; the busiest engine per process is used)
- [x] Process actions: kill (`TerminateProcess`), suspend/resume (`NtSuspendProcess`/`NtResumeProcess`), priority (`SetPriorityClass`), affinity (`SetProcessAffinityMask`) (live-tested against a child process: kill, suspend, resume, priority)
- [ ] UAC elevation manifest (`requireAdministrator`) wired into the Tauri/MSVC build; verify unelevated-launch fallback messaging (manifest embedded via `src-tauri/build.rs`; unelevated-launch messaging still pending)
- [ ] Unit/integration tests against a mockable Win32 boundary (CI-safe) (partial: calc unit tests and live child-process tests; no mockable Win32 layer yet)

## Phase 3 — Linux Backend
- [x] Process enumeration + per-process CPU/RAM via `/proc/[pid]/stat`, `/proc/[pid]/status` (compiled for Linux and parser-tested; not yet run on a Linux host)
- [x] Per-process disk I/O via `/proc/[pid]/io`
- [ ] Per-process network attribution via `/proc/net/{tcp,udp}` + `/proc/[pid]/fd` inode matching (nethogs-style)
- [ ] Per-process GPU stats via DRM `fdinfo` (`/proc/[pid]/fdinfo/<gpu-fd>`, kernel 5.19+, driver-dependent) with system-wide sysfs fallback where unavailable
- [x] Process actions: kill (`SIGKILL`/`SIGTERM`), suspend/resume (`SIGSTOP`/`SIGCONT`), priority (`setpriority`/renice), affinity (`sched_setaffinity`) (via libc; not yet run on a Linux host)
- [ ] Document/implement root-launch elevation path (run via `pkexec`/`sudo`; polkit policy file for GUI launch)
- [ ] Verify behavior across at least one major distro (X11 + Wayland)

## Phase 4 — Hardware Sensors (Temperature & Fan)
- [x] Linux: read `lm-sensors`/`hwmon` sysfs (`/sys/class/hwmon/*`) for CPU/GPU temps + fan RPM (`LinuxSensors` reads hwmon; not yet run on a Linux host)
- [ ] Windows: investigate safe access path (WMI `MSAcpi_ThermalZoneTemperature` first, evaluate a signed embedded-controller/SMBus driver approach — e.g. LibreHardwareMonitor-style — only if WMI proves insufficient; document the security/driver-signing tradeoffs either way) (stub reports unavailable; WMI not yet evaluated)
- [ ] Graceful "unavailable" state in the data model + UI when no safe sensor path exists for the current hardware/OS combo (data model and IPC done; UI pending Phase 7)
- [ ] Unit tests for parsing (hwmon) and mocked WMI responses (sysfs integer parsing tested; hwmon tree and WMI tests pending)

## Phase 5 — Process Actions & Smart Suggestions Engine
- [x] Wire kill/suspend/resume/priority/affinity actions from Phase 2/3 behind the `SystemMonitor` trait, with confirmation-required flag for destructive actions
- [x] Curated critical-process list (never-suggest: `csrss.exe`, `wininit.exe`, `services.exe`, `systemd`, `kthreadd`, etc. per-OS) enforced in the suggestion engine, not just the UI
- [ ] Heuristic rules: sustained CPU/RAM/disk usage on a backgrounded (no visible/foreground window) or user-idle process over a rolling window (CPU rule done, including the user-idle condition on Windows via GetLastInputInfo with 5 minutes as idle; Linux has no idle source yet; RAM and disk rules not yet)
- [x] Suggestion model: rationale, affected process, recommended action, confidence; per-process "don't suggest again" override
- [x] Suggestion engine unit tests (fixture process histories → expected suggestions, critical-list never triggers)

## Phase 6 — Tauri Shell & IPC Layer
- [x] Tauri app scaffold wired to `core` sampling scheduler
- [x] Event push from Rust sampling loop to frontend (Tauri events/channels) with throttling for high-process-count machines
- [x] Command layer for process actions + suggestion apply/dismiss
- [x] Multi-window support configured for overlay widgets (Phase 8): overlay windows with their own capability file
- [x] IPC boundary tests (command validation, no arbitrary PID/action injection beyond intended surface)

## Phase 7 — Widget Dashboard Frontend
- [x] App shell (Svelte + Vite), light/dark theme
- [x] `gridstack.js`-driven widget grid: add/remove/resize/rearrange (verified rendering in headless Edge; interactive drag and resize not yet exercised)
- [x] Save/load named dashboard layouts (JSON persisted to disk) (stored by the backend in the app data folder; the last session autosaves)
- [x] Widget: process list table (sortable/filterable/grouped, live CPU/GPU/RAM/Disk/Net columns + sparklines) (sortable, filterable, grouped by name, with a per-row CPU trend and network and GPU columns; not yet checked with live data in a browser)
- [x] Widget: system CPU graph (total + per-core toggle) (total history graph; per-core shown as current bars, not history)
- [x] Widget: RAM graph
- [ ] Widget: GPU engine + VRAM graph (per-process GPU list is live; no graph yet)
- [ ] Widget: disk I/O graph (per-drive) (busiest processes by I/O; per-drive totals not yet provided)
- [ ] Widget: network I/O graph (up/down) (busiest TCP processes are live; no graph yet, and whole-machine totals are still missing)
- [x] Widget: temperature/fan gauges (Phase 4 data, "unavailable" state handled in UI) (shown as lists, not gauges)
- [x] Widget palette/add-widget UI

## Phase 8 — Desktop Overlay Widgets & Tray
- [x] Pop-out action per widget → dedicated Tauri window (`decorations:false`, `alwaysOnTop:true`, `skipTaskbar:true`, transparent background) (the widget moves out of the grid; the overlay restores on next launch)
- [x] Click-through toggle (Windows `WS_EX_TRANSPARENT`; best-effort X11/Wayland equivalent on Linux) (uses Tauri's ignore-cursor-events; not yet tested on Windows or Linux)
- [ ] Drag-to-reposition + resize, per-monitor placement persistence, "dock back to dashboard" action (drag, resize and dock back are in; positions are saved as absolute coordinates, not keyed per monitor yet)
- [x] Independent per-overlay opacity setting (CSS opacity on the widget body, not window alpha)
- [x] System tray icon with live mini CPU/RAM history graph + quick context menu (open dashboard, top processes, exit) (CPU and memory bars share the icon, with top-process lines and open, click-through and quit; not yet exercised on a desktop)

## Phase 9 — Smart Suggestions UI
- [x] Suggestions feed/panel (sorted by impact), rationale display per suggestion
- [ ] One-click apply, with an extra confirmation step for destructive actions (kill) (apply is one click; no destructive suggestions are generated yet, and the kill confirmation lives in the process table)
- [x] Dismiss / "don't suggest again for this process" controls (dismiss and restore both work from the Suggestions widget)
- [ ] Safe/critical process list viewer + user override management (view only for critical list; opt-out per suggestion rule) (read-only viewer is in the Suggestions widget; override management is not)

## Phase 10 — Archon Support (Design Only)
- [x] `docs/archon-design.md`: design for querying `tpt-archon-kernel` built-in telemetry + capability scheduler directly for zero-overhead, poll-free per-app resource metrics
- [x] Open Questions (pending tpt-archon team): telemetry subscription/query API shape, capability grant model for reading other apps' resource usage, mapping Archon's metrics to the existing `SystemMonitor` trait, process-action equivalents (suspend/kill) under Archon's capability model (recorded in `docs/archon-design.md`; answers still needed from the Archon team)
- [x] Post-v1 `archon` Cargo feature flag plan (`ArchonSystemMonitor` stub, `is_available() -> false`, `Unsupported` errors until implemented)

## Phase 11 — Performance & Polish
- [ ] Profile the sampling loop itself (CPU/RAM footprint) — the monitor must not become the thing worth suspending
- [ ] Adaptive sampling interval tuning (unfocused/minimized dashboard, no open overlays)
- [ ] App icon, branding assets (TPT Solutions)
- [x] First-run onboarding (explain elevation prompt, note sensor availability may vary by hardware) (a card explains this run from the backend report: elevated or not; dismissal is remembered; not yet seen in a running app)
- [ ] Crash reporting / local error logging (opt-in only)
- [ ] Accessibility pass (keyboard navigation, screen reader labels on dashboard + suggestions panel)

## Phase 12 — Testing & QA
- [ ] Unit tests: metrics engine, suggestion engine, sensor parsing (mocked per-platform backends)
- [ ] Integration tests: process actions against a spawned test process (kill/suspend/resume/priority/affinity)
- [ ] Manual QA: Windows 10/11 (elevated + unelevated launch)
- [ ] Manual QA: Linux (X11 + Wayland, at least one major distro; root and non-root launch)
- [ ] Stress test: process list + graphs responsiveness with hundreds/thousands of processes (e.g. many browser tabs)
- [ ] Security review: elevated-process attack surface (IPC command validation, no arbitrary code exec via overlay/window creation), critical-process list cannot be bypassed to suggest suspending core OS processes

## Phase 13 — Release v1
- [ ] Finalize versioning scheme (SemVer) and changelog process
- [ ] Windows installer (MSI/NSIS via Tauri bundler) with UAC manifest embedded, code-signing decision
- [ ] Linux packaging (.deb/.rpm/AppImage via Tauri bundler), documented elevation launch path (polkit/pkexec)
- [ ] Publish LICENSE files + notices in release artifacts
- [ ] User-facing docs (install guide, widget/overlay guide, suggestions safety explainer)
- [ ] Tag v1.0.0 release, publish to GitHub (tpt-solutions org)
- [ ] Post-release triage process for bug reports

---

## Backlog / Future Phases (post-v1)

- [ ] Full Process Hacker parity: handle viewer, DLL/module viewer, services manager, per-process network-connection list, security tokens/privileges viewer, memory viewer/disassembler
- [ ] Suggestion automation: user-defined auto-apply rules (e.g. "auto-suspend after N min idle") with safety guardrails
- [ ] Vendor-specific GPU detail: NVIDIA NVML, AMD ADL/ROCm (temperatures, clocks, power draw, encoder/decoder usage)
- [ ] Persistent long-term history (on-disk time-series store) + CSV/graph export, configurable retention policy
- [ ] CLI (scriptable process list/actions, e.g. `tpt-monitor top`, `tpt-monitor suspend <pid>`)
- [ ] macOS support
- [ ] Remote/headless monitoring agent mode
- [ ] Archon backend implementation (post design-only phase, pending public `tpt-archon` API)
