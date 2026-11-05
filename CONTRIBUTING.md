# Contributing to tpt-monitor

## Licensing

tpt-monitor is dual-licensed under **MIT OR Apache-2.0** (see
[LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE)). By contributing
you agree that your contribution is licensed the same way.

### SPDX header convention

Every source file starts with two comment lines, using the comment syntax of the
language:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions
```

Use the same two lines in TOML, Svelte/TypeScript and shell files, with `#`,
`//` or `<!-- -->` as appropriate. Generated files and lockfiles are exempt.

## Development checks

Run these before opening a pull request:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
npm run check
npm test
npm run build
```

The Rust toolchain is pinned in `rust-toolchain.toml`; the minimum supported
Rust version is declared as `rust-version` in the workspace `Cargo.toml`.

## Design rules

- Platform-specific code lives behind the `SystemMonitor` and `SensorProvider`
  traits in `core/`. The core stays dependency-free.
- Destructive process actions go through `execute_action` so the confirmation
  gate cannot be bypassed.
- Metrics a platform cannot provide are `None`, never `0`.
- `unsafe` code is confined to the platform crates and needs a `// SAFETY:` comment.
