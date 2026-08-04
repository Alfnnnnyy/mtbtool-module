# Changelog

All notable changes to MTB Tool module are documented here. Format based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioned by
`versionCode` in `module.prop` (tags: `v<versionCode>`).

## [Unreleased]

## [1.0.4] - 2026-08-04

### Fixed (audit round 4)
- **DIAG auto-detect contract**: frontend now reads the backend
  `bands:{lte,nrNsa,nrSa}` shape (the UI no longer clears the band grid after
  a successful detection); `raw_byte_count` restored in the response.
- **latest.json duplication removed**: "latest" resolves dynamically to the
  newest manifest by timestamp — no best-effort write that could leave a
  stale emergency-restore target; manifests stay the single source of truth.
- **backup.restore rollback is verified**: mid-restore failures roll back
  already-applied entries from a pre-restore snapshot and report
  `rollback.{attempted,verified,entries}` with read-back verification.
- **feature restore uses ONE transaction manifest**: only the latest
  `feature_disable_<id>` backup containing every write path is used; no more
  mixing entries from different transactions (features share NV paths).
- **Backup IDs**: nanosecond + pid + atomic counter — collision-free even for
  bursts in the same process.
- **Probe warning now reachable in UI**: warning banner shows when the binary
  exists but does not respond (`mtb.dart_exists && !mtb_executable`).
- **CI gate**: `svelte-check` + `tsc --noEmit` run before build (frontend
  deliveries must pass type/structure checks — caught `Promise.withResolvers`
  and missing `kernelsu` declarations).

## [1.0.3] - 2026-08-04

### Security & reliability (audit round 3)
- **Per-category band lock**: a band category that is not sent is left
  untouched; an explicitly empty category requires `allowEmpty` + UI
  confirmation — a single RAT can no longer be zeroed by accident.
- **bandlock.get read errors are fatal**: `ok:false` when any NV read fails;
  the UI blocks Apply until a clean read succeeds (failed reads can no longer
  become zero masks).
- **Verified rollback everywhere**: every multi-path operation (band lock,
  features, import, restore, nv auto-restore) now reports
  `rollback.{attempted, verified, entries[]}` with per-path read-back
  verification — `rolled_back` only claims what was actually verified.
- **`ok` means verified**: nv write/delete, import apply, feature
  disable/restore, and restore only report `ok:true` when every operation
  passed read-back comparison; nv write/delete auto-restores the backup on a
  verification failure.
- **Import stops + rolls back** at the first failed or unverified command.
- **Emergency restore is exclusive + transactional**: FileLock, pre-restore
  snapshot, stop-at-first-failure, rollback of already-applied entries.
- **Backup integrity mandatory**: entries with bytes require a 64-char
  lowercase hex SHA-256 (empty checksum = failure); backups are written
  atomically (temp + fsync + rename), any write failure aborts the operation.
- **Probe no false success**: `ok` now requires the mtb binary to exist, be
  executable AND respond (SELinux/exec failures surface as warning).

## [1.0.2] - 2026-08-04

### Security & reliability (audit round 2)
- **Delete recovery**: `nv delete` now backs up the REAL current bytes first —
  emergency restore re-writes them instead of deleting again.
- **Transactional band lock**: all 4 NV paths are read + backed up in ONE
  transaction, then written and read-back-verified; any failure triggers an
  automatic rollback and `ok:false` (no partial lock).
- **Import is fail-closed**: all before-states are read and backed up before
  any write; a read or backup failure aborts the entire import.
- **Three-state reads**: `Present / Absent / ReadError` are now distinct —
  read errors (SELinux, modem, timeout) abort writes instead of being treated
  as "item absent" and overwritten without a valid backup.
- **Strict band validation**: band tokens must be valid 3GPP band numbers;
  all-empty band lists are rejected unless `allowEmpty` is explicitly passed
  (guards against accidental zero masks disabling all connectivity).
- **Backup integrity**: unique IDs (ms + pid, no collision), backup-ID
  traversal rejected, restore verifies every entry's checksum + slot before
  writing, and `ok:true` for restore now requires every item verified.
- **Exit codes**: `mtbctl` exits non-zero whenever the response is `ok:false`
  (action.sh and scripts can no longer report false success).
- **Feature restore safety**: refuses to delete an NV item when no valid
  backup entry exists; restore entries are checksum-verified and
  read-back-verified.
- Tests: backup collision, ID traversal, three-state parsing, band-list
  validation, delete-restore roundtrip + RPC exit codes in the smoke suite.

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
