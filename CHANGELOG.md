# Changelog

All notable changes to MTB Tool module are documented here. Format based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioned by
`versionCode` in `module.prop` (tags: `v<versionCode>`).

## [Unreleased]

## [1.0.11] - 2026-08-04

### Fixed — audit round: restore deadlock, peridot snapshot, stale delete, NR messaging
- **backup.verify semantics** (restore deadlock): verification of a valid
  backup that DIFFERS from the modem is now a SUCCESS (`ok:true`,
  `integrity_ok:true`, `all_match:false`) — mismatch is the reason restore
  exists. `ok:false` only when a live pre-read fails; `integrity_ok` only for
  checksum/path/version problems. The restore review dialog opens on any
  verified backup, highlights differing entries as restore targets, and
  disables restore only for integrity/read failures.
- **Peridot snapshot preset**: `nr_nsa_band_pref` (exit-0 QMI failure on
  POCO F6/peridot) removed from the default snapshot — it now snapshots the
  3 working bandlock paths; failed paths are never silently skipped.
- **NV Delete stale-target**: the read result is bound to the exact
  path+slot that produced it (`readTarget`); changing base/subpath/slot
  clears it, Review Delete only arms when the target matches the current
  inputs, the dialog freezes the reviewed target, and the delete re-reads
  the frozen target immediately before executing — aborting with a
  changed-state warning if the bytes differ from review.
- **NR mode messaging**: apply failures now distinguish "write attempted"
  (verification/rollback/backup details from the payload) from "nothing
  written" (read-before-write / backup failure); the current mode is always
  re-read live after success or failure instead of assuming the target.
- Cleanup: last `latest.json` wording removed; dead emergency-restore code
  deleted. Smoke: verify-mismatch + integrity-tamper checks updated to the
  new contract.

## [1.0.10] - 2026-08-04

### Fixed — safety-critical UX (on-device review, 4 release blockers)
- **NR mode selector no longer writes on tap** (caused 2 accidental modem
  writes on device): tapping an option only sets a local PENDING selection;
  the UI shows Current → Pending, target byte/path/slot, a backup notice and
  an explicit "Preview & Apply 5G Mode Change" button; after apply the mode
  is re-read and verified; on failure the pending selection reverts and the
  current modem value is untouched.
- **NV Delete is two-step**: first button is "Review Delete" (disabled until
  the current NV was read); the dialog shows path, slot and current bytes and
  requires typing DELETE to arm execution.
- **Backup restore is two-step**: restore starts with a read-only verify
  (backup ID, per-entry integrity + live matches_current); the dialog
  requires typing RESTORE; the one-tap "Restore Latest Backup Now" and
  inaccurate "latest.json" wording are gone (the backend resolves the newest
  manifest dynamically).
- **errno handling no longer hides valid JSON** (features.check partial
  results were discarded as "Exec failed (errno 1)"): the bridge parses
  stdout FIRST; ok:false payloads arrive as ApiError.payload and screens
  render the partial feature list + failed paths. status:"error" features
  disable their mutation buttons.
- **Peridot DIAG UX**: unsupported detection auto-switches to Manual
  Selection with an "Auto-detection unsupported on this firmware" label —
  an empty band list is never presented as a capability result; Apply stays
  disabled until current NV reads succeed.
- **UX cleanup**: Cells signal values rounded to one decimal (e.g. -98.6 dBm);
  Bridge Diagnostics collapsed under an Advanced Diagnostics toggle; backend
  status card shows `mtbctl vX`; backup.create enforces 1..16 paths and
  rejects duplicate paths before any read or manifest creation.

## [1.0.9] - 2026-08-04

### Fixed — WebUI bridge release blocker (on-device, POCO F6)
- **Static kernelsu import**: the official `kernelsu@3.0.2` npm package is now
  a pinned dependency and `import { exec } from 'kernelsu'` is BUNDLED by
  Vite. The previous `import(/* @vite-ignore */ 'kernelsu')` left a bare
  module specifier that the installed WebView could not resolve — every
  screen reported "No exec bridge available" while the same mtbctl binary
  worked from Termux.
- **Bridge adapters by documented surface**: `window.kernelsu.exec` shim
  (alternate hosts / WebUI X) first, then the bundled kernelsu package
  (KernelSU/ReSukiSU manager `ksu` global). No guessed globals. The selected
  bridge is reported in the UI.
- **Bridge self-test**: `mtbctl probe` is executed through the bridge on
  startup; `ready` requires valid JSON + mtbctl_version. Debug panel shows
  bridge kind, self-test errno, stderr summary and mtbctl path/version.
- **Fail-safe gating**: while the bridge is unavailable or probe failed —
  Restart Modem, bandlock apply, feature disable/restore, NV write/delete,
  import apply and backup restore are ALL disabled; Cell polling stops;
  one clear banner is shown; read-only navigation stays available.
- **CI bundle inspection**: built JS must contain the bundled bridge
  (`ksu.exec`), must NOT contain `import("kernelsu")` or bare
  `from"kernelsu"` specifiers.
- **Read-only backup.create / backup.verify** (backend + UI): manual NV
  snapshot without writing, and integrity + live re-read verification of any
  backup — safe pre-write tooling for the device staircase.
- Browser-verified fail-closed behavior (no device): all mutation controls
  disabled, diagnostics shown. On-device WebUI probe still pending.
- Writes/restores remain GATED.

## [1.0.8] - 2026-08-04

### Fixed — read-only device results (POCO F6/peridot round 2)
- **features.check no longer reports false success**: a required-path read
  error is now `status:"error"`, top-level `ok:false` with a `failed_paths`
  summary; `is_disabled` is only evaluated when EVERY required read is
  Present; true Absent stays distinct (`status:"absent"`). Mixed
  Present/Error regression tests added.
- **bandlock.detect no longer guesses**: the real peridot DIAG request
  returns an 11-byte generic response (`15 4B 13 04 00 00 00 00 33 9D 7E`,
  `data_size=11`). The old code fell back to hardcoded offsets 36/108/172
  and reported ok:true with empty bands. Now: fallback offsets are REMOVED —
  candidates must be detected within the payload bounds, an 11-byte payload
  returns `ok:false` with `raw_byte_count` + `response_hex` diagnostics, and
  DIAG semantic failure markers are rejected even with exit 0. Band
  detection is treated as UNSUPPORTED on peridot until a valid request
  format is identified (no guessing). `raw-10-diag.txt` vendored as fixture.
- **nv.read contract**: a failed read returns `absent:null` + `ok:false`
  (a read error never proves absence).
- Frontend: features screen surfaces the failed-read summary; band detection
  shows "unsupported — configure manually" instead of silence.
- Writes/restores remain GATED (no controlled write yet).

## [1.0.7] - 2026-08-04

### Fixed — real-device parser (POCO F6 / peridot, Android 14)
First on-device captures exposed output formats the host harness had guessed
wrong:
- **Duplicate output blocks**: successful EFS reads print every byte TWICE —
  once with an `mtb:` prefix and once with an `RIL` prefix. The old parser
  merged them (declared 8 bytes, parsed 16). The new parser classifies lines
  by prefix and uses the RIL block (authoritative and complete; the mtb
  block can be truncated, e.g. 63 of 64 bytes).
- **Exit-0 QMI failures**: `rsp.result = -117` / "qmi response fail" /
  "error_code(-117)" print with process exit code 0 and no bytes. These are
  now EfsRead::Error — previously they were treated as Absent, which would
  have allowed a write/backup on a broken read.
- **Declared length honored**: `data len(N)` is read and parse results are
  validated against it; truncated output is an error, not a guess.
- **Regression fixtures**: the four real device captures are vendored in
  `tests/fixtures/` and asserted in `test_real_device_fixtures`
  (lte_bandpref 8 bytes, lte extension 24, nr_band_pref 64 with RIL block
  preferred, nr_nsa QMI failure).
- `tests/fake-mtb.sh` now emits the real peridot format (mtb: + RIL blocks,
  data len, QMI-fail mode) so the smoke suite exercises the same shapes.
- Write/restore remain GATED: no modem write is enabled until the restored
  parser is verified on-device.

## [1.0.6] - 2026-08-04

### Fixed
- **Deterministic "newest backup" ordering** (audit): `backup_order_key()`
  parses the full `<millis>_<nanos>_<pid>_<counter>_<reason>` id — the
  previous parser split on `_` but failed to parse nanos (`splitn(2)` left a
  mixed segment), so same-millisecond backups fell back to read_dir order.
  The key is now used everywhere "newest" matters: `backup restore latest`,
  `list_backups()` ordering, feature-restore manifest selection and the
  Backups WebUI order.
- **Regression tests**: handcrafted same-second/same-millis manifests
  created in reversed order prove counter-2 wins regardless of filesystem
  order; full-segment parser unit tests.
- CI `--test-threads=1` and `workflow_dispatch` now ship IN the release tag
  (previously only on HEAD after the tag).
- README: "HTTP bridge" wording replaced with "RPC bridge"; roadmap port
  entry removed.

## [1.0.5] - 2026-08-04

### Fixed
- **`backup restore latest` tie-break**: multiple backups created within the
  same second are now resolved deterministically by the millis/nanos embedded
  in the backup ID (previously read_dir order could pick an older backup as
  "latest" — caught by CI smoke on a different filesystem). Added regression
  test.

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
