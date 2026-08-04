# Changelog

All notable changes to MTB Tool module are documented here. Format based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioned by
`versionCode` in `module.prop` (tags: `v<versionCode>`).

## [Unreleased]

## [1.0.1] - 2026-08-04

### Security (audit fixes)
- **RPC-only bridge**: frontend now executes exactly one fixed command
  (`mtbctl rpc --b64 <payload>`) with a method allowlist and backend-side
  validation — closes the root command injection path.
- **HTTP daemon removed**: no `serve` command, no service.sh, no port 28082.
  Magisk support via WebUI hosts with a controlled exec bridge (e.g. WebUI X).
- **Backup manifest v2**: version/device/createdAt + per-entry size and SHA-256;
  restore verifies integrity of every entry before writing anything (fail closed).
- **Read-back verification**: every write/delete (nv, bandlock, features,
  import, restore) is re-read and compared; responses carry `verified` flags
  and the UI only claims success when verification passes.
- action.sh: volume-key confirmation + correct exit codes; customize.sh uses
  `abort` and sets permissions only on executables (webroot left to KSU).
- Frontend: moddir hardcoded (no localStorage override), no HTTP fallback,
  verified-message gating.

### Fixed
- `features disable` wrote decimal bytes as a single argv entry — now one
  decimal byte per argument, like every other write path.

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
