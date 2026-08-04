#!/bin/bash
# Host smoke test for mtbctl against a protocol-faithful fake /vendor/bin/mtb.
# Simulates the REAL mtb output formats (one tagged line per byte, ASDIV DATA:,
# rsp data:), per the original app parsers in app/src/main/java/.../ui/NvParseUtils.kt
# and CellMonitor.kt. Debian is NOT Android: this checks logic and wire formats,
# not the device (getprop, toybox, real mtb quirks).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="${MTBCTL_BIN:-$ROOT/backend/target/release/mtbctl}"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

export MTB_BIN="$ROOT/tests/fake-mtb.sh"
export MTBTOOL_DIR="$WORK/data"
mkdir -p "$WORK/store"
export FAKE_MTB_STORE="$WORK/store"

jqcheck() { python3 -c "import json,sys; json.load(sys.stdin)"; }

fail=0
check() { # name, condition-string
  if eval "$2"; then echo "PASS: $1"; else echo "FAIL: $1"; fail=1; fi
}

b64url() { python3 -c "import base64,sys; print(base64.urlsafe_b64encode(sys.stdin.buffer.read()).rstrip(b'=').decode())"; }
# --- basic protocol roundtrip ---
"$BIN" nv write /nv/item_files/modem/mmode/lte_bandpref 0000008000000085 --slot 0 --reason smoke | jqcheck
out=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 0)
check "nv read roundtrip" "echo '$out' | grep -q '0000008000000085'"

# --- bandlock set/get ---
out=$("$BIN" bandlock set --lte "1,3,7,8,40" --nrNsa "78" --slot 0)
check "bandlock set ok" "echo '$out' | grep -q '\"ok\":true'"
out=$("$BIN" bandlock set --lte "1,3" --slot 1)
check "bandlock set sim1 path" "echo '$out' | grep -q 'lte_bandpref_Subscription01'"
out=$("$BIN" bandlock get --slot 0)
check "bandlock get bands" "echo '$out' | grep -q '40' && echo '$out' | grep -q '78'"

# test bandlock partial category writes & store untouched check
out=$("$BIN" bandlock set --lte "1,3" --slot 0)
check "bandlock set partial writes lte" "echo '$out' | grep -q 'lte_bandpref' && ! echo '$out' | grep -q 'nr_band_pref'"

# --- DIAG band detect ---
out=$("$BIN" bandlock detect)
check "detect finds LTE bands" "echo '$out' | grep -q '1,3,7,8,40'"

# --- cells ---
out=$("$BIN" cells get --slot 0)
check "cells LTE PCC parsed" "echo '$out' | grep -q '\"earfcn\":1650'"
check "cells tx power" "echo '$out' | grep -q '\"tx_power\":18'"

# --- import (original app JSON format: op w/d, data hex) ---
out=$("$BIN" import preview --json '{"sim0":{"/nv/item_files/modem/lte/rrc/efs/":{"t":{"op":"w","data":"aabb"}}}}')
check "import preview" "echo '$out' | grep -q '\"commands\"'"
out=$("$BIN" import apply --json '{"dualsim":{"/nv/item_files/modem/lte/rrc/efs/":{"t":{"op":"w","data":"aabb"}}}}')
check "import apply dualsim" "echo '$out' | grep -q '\"ok_count\":2'"

# --- security ---
out=$("$BIN" nv write /data/local/tmp/evil 00 --slot 0 || true)
check "path allowlist rejects" "echo '$out' | grep -q 'does not match allowed NV prefixes'"
out=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 9 || true)
check "slot bounds reject" "echo '$out' | grep -q 'Invalid slot'"

# --- backups + emergency restore ---
out=$("$BIN" backup restore latest)
check "emergency restore latest" "echo '$out' | grep -q '\"ok\":true' && echo '$out' | grep -q '\"verified\":true'"

# --- delete-then-restore roundtrip ---
"$BIN" nv write /nv/item_files/modem/mmode/lte_bandpref 11223344 --slot 0 --reason pre_del | jqcheck
"$BIN" nv delete /nv/item_files/modem/mmode/lte_bandpref --slot 0 --reason do_del | jqcheck
read_del=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 0)
check "nv delete shows absent" "echo '$read_del' | grep -q '\"absent\":true'"
rest_out=$("$BIN" backup restore latest)
check "restore deleted item" "echo '$rest_out' | grep -q '\"ok\":true'"
read_rest=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 0)
check "nv restored bytes" "echo '$read_rest' | grep -q '11223344'"

# --- bandlock validation ---
set_empty=$("$BIN" bandlock set --slot 0 || true)
check "bandlock set empty rejected" "echo '$set_empty' | grep -q 'refusing zero-band mask'"
set_nr_no_allow=$("$BIN" bandlock set --nrNsa "" --slot 0 || true)
check "bandlock set --nrNsa empty rejected without allow" "echo '$set_nr_no_allow' | grep -q 'allowEmpty'"
set_nr_allow=$("$BIN" bandlock set --nrNsa "" --allow-empty --slot 0)
check "bandlock set --nrNsa empty accepted with allow" "echo '$set_nr_allow' | grep -q '\"ok\":true'"
check "bandlock set invalid band exit code" '! "$BIN" bandlock set --lte "9999" --slot 0 > /dev/null 2>&1'

# test bandlock get partial read error
out_fail_get=$(FAKE_MTB_FAIL_PATH="nr_band_pref" "$BIN" bandlock get --slot 0 || true)
check "bandlock get partial read error" "echo '$out_fail_get' | grep -q '\"ok\":false' && echo '$out_fail_get' | grep -q 'read failed for'"
# test probe missing mtb
out_probe_nomtb=$(MTB_BIN=/nonexistent "$BIN" rpc --b64 "$(printf '%s' '{"method":"probe","params":{}}' | b64url)" || true)
check "probe missing mtb" "echo '$out_probe_nomtb' | grep -q '\"ok\":false'"

# test nv write verify-fail auto-restore
"$BIN" nv write /nv/item_files/modem/mmode/lte_bandpref 11223344 --slot 0 --reason pre_fail_test | jqcheck
out_write_fail=$(FAKE_MTB_FAIL_WRITE="aabbccdd" "$BIN" nv write /nv/item_files/modem/mmode/lte_bandpref aabbccdd --slot 0 || true)
check "nv write verify-fail ok:false" "echo '$out_write_fail' | grep -q '\"ok\":false'"
check "nv write verify-fail rollback attempted" "echo '$out_write_fail' | grep -q '\"attempted\":true'"
read_after_restore=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 0)
check "nv write verify-fail bytes restored" "echo '$read_after_restore' | grep -q '11223344'"

# test import apply failure rollback mid-list
out_import_fail=$(FAKE_MTB_FAIL_WRITE="aabbccdd" "$BIN" import apply --json '{"sim0":{"/nv/item_files/modem/mmode/lte_bandpref":{"t":{"op":"w","data":"55555555"}},"/nv/item_files/modem/mmode/nr_band_pref":{"t":{"op":"w","data":"aabbccdd"}}}}' || true)
check "import apply failure ok:false" "echo '$out_import_fail' | grep -q '\"ok\":false'"
check "import apply failure rollback attempted" "echo '$out_import_fail' | grep -q '\"attempted\":true'"
read_imp_restored=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 0)
check "import apply first command restored" "echo '$read_imp_restored' | grep -q '11223344'"

# --- features.check must distinguish Error from Absent ---
out=$(FAKE_MTB_QMI_FAIL=1 "$BIN" features check --slot 0 || true)
check "features check qmi -> ok:false" "echo '$out' | grep -q '\"ok\":false'"
check "features check qmi -> failed_paths" "echo '$out' | grep -q 'failed_paths'"
check "features check qmi -> status error" "echo '$out' | grep -q '\"status\":\"error\"'"

# --- bandlock.detect must reject the real 11-byte peridot DIAG payload ---
out=$(FAKE_MTB_DIAG_REAL=1 "$BIN" bandlock detect || true)
check "detect 11-byte real payload -> ok:false" "echo '$out' | grep -q '\"ok\":false'"
check "detect error mentions unsupported" "echo '$out' | grep -q 'unsupported/truncated'"
check "detect reports raw_byte_count 11" "echo '$out' | grep -q 'raw_byte_count\":11'"

# --- nv.read: failed read must not claim absent ---
out=$(FAKE_MTB_QMI_FAIL=1 "$BIN" nv read /nv/item_files/modem/mmode/nr_band_pref --slot 0 || true)
check "nv.read qmi -> absent null not true" "echo '$out' | grep -q '\"absent\":null'"

# --- real-format: QMI failure with exit 0 must be Error, not Absent ---
out=$(FAKE_MTB_QMI_FAIL=1 "$BIN" nv read /nv/item_files/modem/mmode/nr_nsa_band_pref --slot 0 || true)
check "qmi fail exit0 -> error not absent" "echo '$out' | grep -q 'qmi read failure'"

# --- read-only backup.create / backup.verify ---
out=$("$BIN" backup create --paths /nv/item_files/modem/mmode/lte_bandpref,/nv/item_files/modem/mmode/nr_band_pref --slot 0 --reason smoke_snapshot)
check "backup create read-only snapshot" "echo '$out' | grep -q '\"ok\":true'"
vid=$(echo "$out" | python3 -c "import json,sys; print(json.load(sys.stdin)['backup']['id'])")
out=$("$BIN" backup verify "$vid")
check "backup verify matches" "echo '$out' | grep -q '\"ok\":true'"
# tamper the fake store for lte_bandpref, verify must now fail closed
FAKE_STORE_KEY=$(echo '0|/nv/item_files/modem/mmode/lte_bandpref' | tr '/' '_')
printf '00' > "$WORK/store/$FAKE_STORE_KEY"
out=$("$BIN" backup verify "$vid" || true)
check "backup verify detects tamper" "echo '$out' | grep -q '\"ok\":false'"

# --- RPC bridge (mtbctl rpc --b64) ---
p=$(printf '%s' '{"method":"probe","params":{}}' | b64url)
out=$("$BIN" rpc --b64 "$p")
check "rpc probe" "echo '$out' | grep -q '\"ok\":true'"
p2=$(printf '%s' '{"method":"nv.read","params":{"path":"/nv/item_files/modem/mmode/lte_bandpref","slot":0}}' | b64url)
out=$("$BIN" rpc --b64 "$p2")
check "rpc nv.read" "echo '$out' | grep -q '\"ok\":true'"
p3=$(printf '%s' '{"method":"evil.run","params":{}}' | b64url)
check "rpc evil method exit code" '! "$BIN" rpc --b64 "$p3" > /dev/null 2>&1'
out=$("$BIN" rpc --b64 'notbase64!!' || true)
check "rpc junk rejected" "echo '$out' | grep -q 'rpc decode'"
out=$("$BIN" rpc --b64 "$(printf '%s' '{"method":"nv.write","params":{"path":"/data/x","hex":"00","slot":0}}' | b64url)" || true)
check "rpc path allowlist enforced" "echo '$out' | grep -q 'does not match allowed NV prefixes'"

echo
[ "$fail" = "0" ] && echo "ALL SMOKE TESTS PASSED" || { echo "SMOKE FAILURES"; exit 1; }
