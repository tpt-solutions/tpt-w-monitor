# Archon Support — Design (Draft)

Status: **design only**. No Archon code is written or planned for v1.
Owner: TPT Solutions. Upstream: <https://github.com/tpt-solutions/tpt-archon>.

## Goal

On Archon, read per-application CPU, memory, GPU, disk and network usage
directly from `tpt-archon-kernel`'s built-in telemetry and capability scheduler,
instead of sampling `/proc`-style counters. The aim is accurate metrics with no
polling overhead for each user-space application.

## Current state of the upstream API

The public `tpt-archon` README documents the storage engine
(`tpt-archon-core`), the zero-copy bridge (`tpt-archon-bridge`), the
capability-based microkernel (`tpt-archon-kernel`, with a driver framework and a
`Resource::Device` capability) and the SQL layer (`tpt-archon-relational`).

It does **not** document:

- a telemetry, metrics, or tracing interface;
- resource accounting, quotas, or limits;
- a capability scheduler API (the README mentions scheduler deadlock-freedom
  only as a formal-verification target);
- the capability rights model, delegation, or revocation rules;
- any query or subscription API for other applications' usage.

Because of this, the integration below is a plan against an interface that does
not exist yet. It must be revisited once the Archon team publishes one.

## Proposed integration

The core already abstracts backends behind `SystemMonitor`
(`core/src/traits.rs`). An Archon backend would be a third implementation next to
the Windows and Linux ones:

```text
            ┌──────────────── tpt-monitor-core ────────────────┐
            │ SystemMonitor trait  ·  Sampler  ·  Action gate  │
            └───────▲──────────────────▲──────────────────▲────┘
                    │                  │                  │
          platform-windows     platform-linux     archon (feature "archon")
                                                   │
                                     tpt-archon-kernel telemetry +
                                     capability scheduler (TBD API)
```

Mapping required (to be confirmed against the real API):

| `SystemMonitor` need | Archon source (hypothesised) |
|---|---|
| `sample_processes` — CPU/RAM/IO per app | kernel telemetry query per task/capability holder |
| `sample_system` — totals, per-core | kernel aggregate telemetry |
| `apply_action` — kill / suspend / resume / priority / affinity | capability-scheduler operations, gated by a capability grant |
| `is_available` | capability present and telemetry endpoint reachable |

Push versus pull: if Archon offers subscriptions, the `Sampler` would run in
event mode and the `SamplingScheduler` would be bypassed for that backend. If
it offers only queries, the existing scheduler applies unchanged.

## Open questions (pending tpt-archon team)

1. **Telemetry API shape**: is it query-based, subscription/streaming, or both? What are the units and sampling cadence?
2. **Capability grant model**: what capability lets a monitor read *other* applications' resource usage? Is it per-app, per-system, or revocable at runtime?
3. **Metric mapping**: which Archon metrics map onto `ProcessMetrics` and `SystemMetrics`, and which are absent (in which case the field stays `None`)?
4. **Process-action equivalents**: how do suspend, kill and priority change map onto the scheduler, and do they require the same confirmation semantics as on Windows and Linux?
5. **Identity**: how does an Archon task ID map to the `Pid` type used in the UI and suggestion engine?

## Post-v1 feature-flag plan

- Add an optional Cargo feature `archon` to the app crate (not to `core`, which
  stays dependency-free).
- Ship an `ArchonSystemMonitor` stub that implements `SystemMonitor`:
  - `is_available()` returns `false`;
  - every method returns `MonitorError::Unsupported("archon backend")`.
- The stub lets the app compile and select backends at runtime with the feature
  on, before the real API exists.
- Replace the stub only after the open questions above are answered, and gate
  the real implementation behind the same feature.
- Keep the stub's behaviour under test so that a "no Archon" system never
  produces spurious process data.

## Non-goals

- No change to the Windows or Linux backends on Archon's behalf.
- No assumption that Archon's scheduler can suspend system services; the
  critical-process list still applies.
