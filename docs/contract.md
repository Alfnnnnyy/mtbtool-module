# MTB Tool Module — Cross-Agent Contract (v2 — security hardening)

Supersedes v1. Changes after the v1.0.0 audit:

## 1. RPC-only frontend bridge (root command injection fix)
- Frontend NEVER builds arbitrary mtbctl command strings with user input.
- The ONLY exec the frontend may run (fixed command, no interpolation):
  `/data/adb/modules/mtbtool/bin/mtbctl rpc --b64 <PAYLOAD>`
- moddir is HARDCODED to `/data/adb/modules/mtbtool`. No localStorage override.
- PAYLOAD = base64url (no padding, charset A-Za-z0-9_-) of:
  `{"method":"<method>","params":{...}}`
- Backend: decode → parse JSON → method MUST match an explicit allowlist →
  validate params (path allowlist, slot 0/1, hex, size caps) → dispatch →
  `/vendor/bin/mtb` via Command::new with per-arg argv (never sh -c).
- Methods (dot notation; backend also accepts the legacy space form):
  probe, nv.read, nv.write, nv.delete, bandlock.get, bandlock.set,
  bandlock.detect, features.check, features.disable, features.restore,
  cells.get, modem.restart, import.preview, import.apply, backup.list,
  backup.restore, config.get, config.set
- Params keys: nv.* {path, slot, hex?, reason?}; bandlock.set {lte, nrNsa,
  nrSa, slot}; bandlock.get/detect {slot}; features.* {id, slot}; cells.get
  {slot}; import.* {json}; backup.restore {id}; config.set {json}; probe {}.
  slot may be number or string.
- rpc responses: identical JSON to the CLI command output.

## 2. HTTP daemon REMOVED (unauthenticated root API fix)
- `serve` command, service.sh, port 28082, nohup daemon: GONE. Do not
  reference them anywhere in frontend/README.
- Magisk support = WebUI host with a controlled exec bridge (e.g. WebUI X,
  which exposes the kernelsu exec API to module WebUIs). The bridge may fall
  back to `window.kernelsu` if the dynamic import fails — but MUST NOT call a
  local HTTP API.

## 3. Backup manifest v2 + integrity verification
Backup file `/data/adb/mtbtool/backups/<ts>_<reason>.json`:
```json
{
  "version": 2,
  "id": "<ts>_<reason>",
  "time": 1785771303,
  "reason": "bandlock_set",
  "device": "peridot",
  "createdAt": "2026-08-04T00:00:00Z",
  "entries": [
    {"slot": 0, "path": "/nv/...", "bytes": "hex|null", "size": 8, "sha256": "hex"}
  ]
}
```
- `sha256` = SHA-256 of the raw bytes (hex, lowercase); `null` when the item
  was absent (delete-restore). `size` = byte count (0 for null).
- `device` from getprop ro.product.device (fallback "unknown").
- Restore MUST verify before writing: version==2, every path passes the NV
  allowlist, bytes hex-valid, size matches, sha256 matches, backup file JSON
  parses (fail closed with clear error, no partial restore).
- After every write (nv write/delete, bandlock set, features disable/restore,
  import apply, backup restore): RE-READ and COMPARE bytes; report per-item
  `verified: true|false`. Write responses include the expected bytes.
- `latest.json` copy is still maintained for action.sh emergency restore.

## 4. Frontend verified-message gating
- Success toasts like "Bandlock NV write verified and applied successfully!"
  may only be shown when the backend response has verified:true for all items.
  Otherwise show a warning "written but read-back verification FAILED — check
  Backups and restore if needed".

## 5. Response shape notes (v2 additions)
- nv write/delete: add `verified` (bool) + `expected` (hex written).
- bandlock.set: `verified` becomes per-path object
  `{"<path>": {"bytes": "hex", "match": true}}` (match = re-read == written).
- import.apply results: add `verified` per result.
- backup.restore restored[]: add `verified` per entry.
- probe: `mtbctl_version` = Cargo pkg version; add `device`.
- CLI shape unchanged otherwise. `rpc` command: `mtbctl rpc --b64 <payload>`
  → same JSON; errors: {"ok":false,"error":"..."} exit 0 (usage errors exit 1).

Everything else from v1 contract (CLI commands, band lists, feature table,
cells parsing, DIAG heuristic, path allowlist, flock, no auto-apply) is
unchanged. Backend files: rpc.rs replaces http.rs; backup.rs manifest v2.
