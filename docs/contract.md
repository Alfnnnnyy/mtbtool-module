# MTB Tool Module — Cross-Agent Contract (v1)

Repo root: /root/project/mtbtool-module (fork of h3nnes/mtbtool-android-app, MIT).
The original Android app lives in `app/` (reference source, DO NOT modify).
Target device: POCO F6 (HyperOS, Qualcomm SM8635 modem, /vendor/bin/mtb present),
root via ReSukiSU (KernelSU fork). Module must also work under Magisk.

## Module identity
- module.prop: id=mtbtool, name=MTB Tool, version=v1.0.0, versionCode=1,
  author=Alfnnnnyy, description=short, minKernel=..., support Magisk+KSU+APatch.
- Install dir: /data/adb/modules/mtbtool/ → `$MODDIR` = /data/adb/modules/mtbtool
- Binary: `$MODDIR/bin/mtbctl` (Rust, aarch64-linux-android, built ONLY in GA)
- Data dir: `$MTBTOOL_DIR` = /data/adb/mtbtool (config.json, backups/)
- mtb vendor binary: /vendor/bin/mtb (NEVER bundle/overwrite it)

## mtbctl CLI contract
Usage: `mtbctl <command> [args]`. ALWAYS prints exactly one JSON object to
stdout; exit 0 for handled commands (incl. ok:false results), non-zero only
for usage errors. Env overrides (for host testing): MTBTOOL_DIR, MTB_BIN.

Commands (all output keys must match exactly):
- `probe` → {ok, mtb_path, mtb_exists, mtb_executable, mtbctl_version, model, android_sdk, data_dir}
- `nv read <path> [--slot 0|1]` → {ok, exit, absent, bytes:"<lower-hex>"}
  absent=true when exit!=0 or zero bytes returned (modem returns exit 0 + empty for missing item).
- `nv write <path> <hex> [--slot N] [--reason R]` → backup first; then write
  (mtb `4 5 <slot> <path> <dec bytes>`); then re-read verify → {ok, exit, before, after, backup:{id,time,entries}}
- `nv delete <path> [--slot N] [--reason R]` → backup with null entry; delete
  (mtb `4 6`); verify re-read → {ok, backup}
- `bandlock get [--slot N]` → {ok, paths:{ltePrimary,lteExtension,nrNsa,nr}, bytes:{...4 hex strings},
  bands:{lte:[],nrNsa:[],nrSa:[]}, errors:{path:msg}}
- `bandlock set --lte "1,3,7" --nrNsa "" --nrSa "" [--slot N]` → build 4 masks
  (LTE primary 8B B1-64; LTE ext 24B B66/B71; NR NSA 64B; NR SA 64B), backup each,
  write each, re-read verify → {ok, writes:[{path,backup_id}], verified:{path:hex}}
- `bandlock detect` → DIAG open+read (`mtb 5 <17-byte header> <ascii path>` /
  `mtb 5 <21-byte read args>`), parse `rsp data:` 0xNN tokens, auto offset scan
  (zero-spurious + >=5 known bands, NR two offsets >=10 apart, fallback 36/108/172)
  → {ok, lte:[], nrNsa:[], nrSa:[], offsets:{lte,nrSa,nrNsa}, raw_byte_count}
- `features check [--slot N]` → {ok, features:[{id,label,status:enabled|disabled|absent|error,paths:[{path,absent,bytes}]}]}
- `features disable <id> [--slot N]` / `features restore <id> [--slot N]` →
  {ok, id, writes:[{path,backup_id}]} / restore = rewrite captured bytes or delete
  when originally absent (verify delete with re-read, like original app)
- `cells get [--slot N]` → {ok, ts, lte:[{label,earfcn,pci,rsrp,rsrq,rssi,snr}],
  nr:[{label,rsrp,rsrq}], tx_power:int|null} (mtb `9 <opt> <slot>`; LTE opts 0-3, NR 10-12, tx 31)
- `modem restart` → {ok, exit} (mtb `11 0`)
- `import preview --json <s>` → {ok, commands:[{slot,op:w|d,path,bytes}], errors:[]}
- `import apply --json <s>` → backup every write; sequential exec → {ok, results:[{slot,op,path,ok,exit,backup_id}], ok_count, fail_count}
- `backup list` → {ok, backups:[{id,time,reason,entries,size}]} newest first
- `backup restore <id|latest>` → {ok, restored:[{slot,path,ok,exit}]} (write or delete per null entry)
- `config get` → {ok, config:{manual_lte:[],manual_nrNsa:[],manual_nrSa:[],slot}}
- `config set --json <s>` → {ok}
- `serve --port N` → HTTP server, binds 127.0.0.1 ONLY, POST /api body
  {"cmd":"...","args":{...}} → same JSON as CLI; GET /health → {ok:true};
  max body 64KB; per-connection thread; log to stderr.

## Hard security rules (backend-enforced, never bypass)
1. NV path allowlist prefixes: /nv/item_files/modem/mmode/ ,
   /nv/item_files/modem/lte/rrc/efs/ , /nv/item_files/modem/nr5g/RRC/ ,
   /nv/item_files/modem/lte/RRC/ . Reject anything else, reject ".." and control chars.
2. slot ∈ {0,1} else reject.
3. Hex payload: even length, [0-9a-fA-F], len ≤ 1024 chars. Write payload ≤ 512 bytes.
4. Import: ≤ 200 entries; JSON parsed strictly; op ∈ {w,d}; unknown top-level keys rejected.
5. Backup MUST succeed before ANY write/delete; abort on backup failure.
6. flock on $MTBTOOL_DIR/.lock for all mutations (single writer).
7. NEVER auto-apply anything at boot. No bandlock on service start.
8. Feature ids only from embedded table; band numbers validated against 3GPP lists.
9. serve mode: bind 127.0.0.1; never 0.0.0.0; no shell interpolation — args parsed as structured values.

## Backup format
$MTBTOOL_DIR/backups/<unix_ts>_<reason>.json →
{"id":"<unix_ts>_<reason>","time":<ts>,"reason":"...","entries":[{"slot":0,"path":"...","bytes":"hex|null"}]}
Also maintain $MTBTOOL_DIR/backups/latest.json (copy of most recent). Restore = write bytes or delete when null.

## Feature table (port from app/src/main/java/dev/henrik/mtbtool/FeatureDef.kt EXACTLY)
12 features, ids: r17_2t2t, r16_2t1t, ul_mimo, fdd_ul_mimo, nr_ulca, dl_nrca,
lowband_4rx, nsa_tf_nrca, nsa_ff_nrca, nsa_tt_nrca, segmentation, dss.
Base path prefix /nv/item_files/modem/nr5g/RRC/. Port reads/writes/isDisabled byte-for-byte.
Feature status semantics (from FeaturesChecker.kt): any read path absent → status=absent
(modem default); else isDisabled(bytes) → disabled; else enabled. Errors → error.

## Band lists (from BandlockManager.kt EXACTLY)
ALL_LTE_BANDS = [1,2,3,4,5,7,8,12,13,14,17,18,19,20,21,25,26,28,29,30,32,34,38,39,40,41,42,43,46,48,66,71]
ALL_NR_BANDS = [1,2,3,5,7,8,12,14,18,20,25,26,28,29,30,34,38,39,40,41,46,48,50,51,53,65,66,70,71,74,75,76,77,78,79,80,81,82,83,84,86,89,90,91,92,93,94,95,96,97,100,101,102,104,257,258,260,261]
NV paths: /nv/item_files/modem/mmode/{lte_bandpref (8B), lte_bandpref_extn_65_256 (24B), nr_band_pref (64B), nr_nsa_band_pref (64B)} + _Subscription01 variants for slot 1.
Bitmask: bit(band-1) set → band enabled. DIAG offsets heuristic port from detectBandOffsets.

## Cells parsing (port from CellMonitor.kt EXACTLY)
parseAsdivLine → key=value map; invalid float sentinel >= 65534.5 → null;
LTE keys earfcn/pci/rsrp/rsrq/rssi/snr; NR rsrp/rsrq. StalenessTracker threshold 3 is FRONTEND concern.

## mtb output parsing (port from NvParseUtils.kt + BandlockManager.kt)
EFS read lines contain tag "xiaomi_nvefs_test_efs_read:" + trailing 2-hex token per line.
DIAG response line starts with "rsp data:" + "0xNN" tokens.
Service wrappers prefix output with "EXIT:<n>" — only relevant for HTTP bridging; CLI runs mtb directly and uses exit code.

## Frontend contract
- Svelte 5 (runes syntax, $state/$derived/$props/$effect), TypeScript, Vite.
- vite.config.ts: base './', outDir ../webroot, emptyOutDir, external ['kernelsu'],
  vitePreprocess from @sveltejs/vite-plugin-svelte (NEVER svelte-preprocess).
- webroot/ is gitignored, built ONLY in GitHub Actions.
- package.json: PIN EXACT versions, no ^. Deps: svelte, @sveltejs/vite-plugin-svelte,
  vite, typescript, lucide-svelte, vitest (+ @vitest/ui not needed).
- bridge.ts: `api(cmd, args)` → Promise<any>:
  1) dynamic import('kernelsu') → ksu.exec("$MODDIR/bin/mtbctl <cmd> <shell-safe args>")
     parse stdout JSON. MODDIR=/data/adb/modules/mtbtool, overridable in localStorage 'mtbtool_moddir'.
  2) fallback: fetch http://127.0.0.1:28082/api (2 retries, 500ms; try localhost 2nd).
  Never Promise.withResolvers. Output textarea programmatic set must dispatch input event.
- Screens (HyperOS dark theme, Miuix-like, CSS variables, Lucide icons):
  Dashboard (probe, danger warning, quick actions, latest backup), Bandlock (detect/manual,
  band grid LTE+NR NSA+NR SA, hex preview old→new, two-step confirm, apply, modem restart),
  Features (check, disable/restore toggles per feature, NR mode selector), NV & Import
  (read single path w/ dropdown bases, import JSON file: preview table + apply),
  Cells (polling 2s default, stops when document.hidden, staleness 3), Backups (list/restore/emergency).
  Labels/colors: port from original app where sensible (dark, blue accent ~#0099FF,
  warning red #F44336, ok green #4CAF50).

## File layout (agents write ONLY their trees)
backend/   → Cargo project (crate mtbctl, edition 2021, deps: serde, serde_json only)
frontend/  → Svelte app (src/, package.json, configs)
Everything else (module.prop, *.sh, .github/, README, CHANGELOG) → Main agent.
