# Security Review — tpt-monitor (pre-release)

Status: first review, done from source by reading the code paths and tests. It has not
been validated by a penetration test or on an elevated test machine. Re-run it before
each release (see the checklist at the end).

## Why it matters

On Windows the app requests administrator rights at launch (`requireAdministrator` in
`src-tauri/windows-app.manifest`). Everything the webview does therefore runs elevated.
A bug in the UI, or a malicious page loaded into the webview, could act with full rights.
The review concentrates on keeping the webview from reaching anything it should not.

## Attack surface

| Surface | Where | Who can reach it |
|---|---|---|
| IPC commands (`#[tauri::command]`) | `src-tauri/src/commands.rs` | The main window and overlay windows |
| Overlay windows and their URLs | `src-tauri/src/lib.rs` `spawn_overlay_window` | Created only by the backend |
| Window-to-window events (`overlay-dock`) | `src/App.svelte`, `src/OverlayApp.svelte` | Any window can emit |
| Layout and overlay files in the app data folder | `layouts.rs`, `overlays.rs`, `protected.rs` | Anyone with write access to the user's app data |
| Process actions | `core/src/action.rs`, platform crates | Through `apply_action` only |
| Frontend rendering | `src/**/*.svelte` | Content is local only |

## What was checked

- **Command execution:** none. The backend never spawns a shell or runs external programs.
- **HTML injection:** no `{@html}`, `innerHTML`, `eval` or `new Function` in `src/`.
- **Network access:** none from the app. The only outbound channel is the webview's IPC.
- **Capabilities:** the main window has `core:default`. Overlay windows have `core:default`
  plus `core:window:allow-start-dragging`. No plugin with file or shell access is granted.
- **CSP:** `default-src 'self'`, with `style-src 'self' 'unsafe-inline'` for Svelte's
  inline styles. Scripts are limited to the app's own origin.
- **Process actions:** a PID must appear in the latest snapshot. Kill needs confirmation.
  Critical processes refuse kill and suspend even when confirmed. `ActionDto` rejects
  unknown kinds.
- **Layout and overlay input:** names are checked against a character set, file size is
  limited, and content must parse. Overlay ids must match a known kind plus a small number,
  which keeps window labels safe. Sizes and opacity are clamped.
- **Protected list:** only bare executable names are accepted. Built-in names cannot be removed.

## Findings

| # | Severity | Finding | Status |
|---|---|---|---|
| 1 | Medium | The dock handler used a widget kind from another window's event payload without checking it. A bad kind would throw while mounting. | **Fixed** in `src/App.svelte`: the kind must be a known widget kind. |
| 2 | Medium | The app runs elevated, so any code running in the webview has administrator rights. | **Accepted, with mitigations:** no remote content, strict CSP, no HTML injection. Future option: split the sampler from the UI into a separate unelevated process. |
| 3 | Low–Medium | Time-of-check to time-of-use: a PID checked against a snapshot up to a second old could be reused by another program before the action runs. | **Fixed.** Every action now sends the executable name the user saw. The backend reads the name from the OS at the moment of the action and refuses the action if it differs or cannot be read. Covered by boundary tests and a live Windows test. |
| 4 | Low | Any window can emit `overlay-dock`. The worst outcome is a widget appearing on the dashboard. | **Mitigated** by finding 1's fix. |
| 5 | Low | `protected.json` and the layout files can be edited by anyone with write access to the user's app data. Protection can only be added this way, and built-in names stay protected. | **Accepted.** Same trust boundary as the user's own files. |
| 6 | Info | Per-connection TCP counters need administrator rights, so the feature stays off for ordinary runs. | **Accepted** and documented. |

## Residual risks

- Finding 3 is fixed. A process whose name cannot be read (for example a protected Windows
  process when the app is not elevated) is refused rather than acted on, so this trades a few
  refusals for certainty about the target.
- Elevation applies to the whole app, not just the sampler. This is the trade-off the spec
  chose. Splitting processes would reduce it but needs a secure IPC channel between them.

## Release checklist

- [ ] Re-read every `#[tauri::command]` and confirm it validates its inputs.
- [ ] Confirm the CSP in `src-tauri/tauri.conf.json` and `index.html` still matches.
- [ ] Confirm `grep` finds no `{@html}`, `innerHTML` or `eval` in `src/`.
- [x] Finding 3 fixed (checked at action time).
- [ ] Run the elevated test suite, which covers the TCP counters and the elevation check.
