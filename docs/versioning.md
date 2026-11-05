# Versioning and release process

## Version scheme

tpt-monitor uses [SemVer 2.0](https://semver.org/):

- **MAJOR** (`X.0.0`): a change that breaks saved files (layouts, overlays, `protected.json`)
  or the IPC contract between the frontend and the backend.
- **MINOR** (`0.X.0`): new features that keep existing files and the IPC contract working.
- **PATCH** (`0.0.X`): fixes only.

Before 1.0.0, minor versions may still change behaviour, but saved files stay readable.

The version lives in one place: `[workspace.package] version` in `Cargo.toml`. The
frontend (`package.json`) and `src-tauri/tauri.conf.json` must match it. The release
script (to be written with the Phase 13 packaging work) checks that all three agree.

## Changelog

- Every change that users or packagers would notice goes under `## [Unreleased]` in
  `CHANGELOG.md`, in the same commit as the change.
- Sections: Added, Changed, Deprecated, Removed, Fixed, Security.
- At release, `[Unreleased]` becomes `[X.Y.Z] - YYYY-MM-DD`, and a new empty
  `[Unreleased]` section is added above it.
- Security fixes are listed under Security, with the finding number from
  `docs/security-review.md` when there is one.

## Release steps

1. Run the full checks: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace`, `npm run check`, `npm test`, `npm run build`.
2. Run the elevated test suite on a Windows machine (see `todo.md`, Phase 12).
3. Run the release checklist in `docs/security-review.md`.
4. Update the version in the three places above and the changelog.
5. Commit as `Release vX.Y.Z`, then tag `vX.Y.Z` (annotated) on that commit.
6. Build the installers (Phase 13 packaging) and attach them to the tag.

## Pre-releases

Use `-alpha.N`, `-beta.N` or `-rc.N` suffixes, for example `0.2.0-beta.1`. Pre-releases
are for testing only and are not listed as the latest release.
