# Changelog

All notable changes to tpt-monitor are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[SemVer](https://semver.org/) (see `docs/versioning.md`).

## [Unreleased]

Pre-release. Nothing has been published yet. Hardware-dependent parts are marked in
`todo.md` and have not all been checked on real machines.

### Added

- Core engine: metrics model, backend traits, process-action confirmation gate, adaptive
  sampler, ring-buffer history, and the suggestion engine.
- Windows backend: process list, per-process CPU, memory and disk I/O, visible-window
  detection, per-core CPU, whole-machine network totals, TCP traffic per process (needs
  an elevated run), GPU engine and VRAM per process (PDH), and process actions.
- Linux backend: `/proc` and `/sys` metrics, whole-machine network totals, hwmon sensors,
  and process actions through `libc`.
- Suggestions: sustained CPU and sustained disk rules on background processes (or any
  process while the user is idle), always suggest-only. Dismiss and restore. A critical
  process list that cannot be bypassed, plus user additions that can only be removed by
  the user.
- Dashboard: gridstack widgets for CPU, memory, temperature and fans, processes (grouped,
  sortable, filterable), suggestions, network, disk, and GPU. Named layouts and autosave.
- Desktop overlays: any widget can pop out into an always-on-top window, with opacity,
  click-through, dock back, and saved position.
- System tray: CPU and memory bars as the icon, top processes, and quick actions.
- First-run onboarding that reports this run's elevation.
- Security review in `docs/security-review.md`.

### Security

- Every process action sends the executable name the user saw, and the backend refuses
  the action if the PID now belongs to a different program.
- Kill and suspend are refused for critical processes even when confirmed.

### Known limitations

- Per-process network on Linux is not attributed yet.
- UDP traffic is not attributed to processes on Windows.
- Windows temperature sensors through WMI are not evaluated yet.
- The RAM suggestion rule is deliberately not built (see the decisions log in `todo.md`).
- The elevated test run and the Linux host run are still to do.
