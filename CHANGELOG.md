# Changelog

All notable changes to MTB Tool module are documented here. Format based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioned by
`versionCode` in `module.prop` (tags: `v<versionCode>`).

## [Unreleased]

## [1.0.0] - 2026-08-03

### Added
- Magisk/KernelSU module shell (module.prop, customize.sh, service.sh, action.sh, uninstall.sh).
- `mtbctl` Rust backend: NV read/write/delete, band lock (DIAG band detection + masks),
  12 modem features disable/restore, cells monitor, JSON import, backups, config,
  localhost HTTP bridge (`serve`).
- Svelte 5 WebUI: Dashboard, Bandlock, Features, NV & Import, Cells, Backups —
  MTB Control design system (Linear-inspired, see frontend/DESIGN.md).
- GitHub Actions: `ci.yml` (cargo test + WebUI test/build), `release.yml`
  (auto-release on versionCode bump: NDK cross-compile, zip, tag, release).
- Safety: backup-before-write, NV path allowlist, single-writer lock, no auto-apply at boot.
