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
out=$("$BIN" nv write /data/local/tmp/evil 00 --slot 0)
check "path allowlist rejects" "echo '$out' | grep -q 'does not match allowed NV prefixes'"
out=$("$BIN" nv read /nv/item_files/modem/mmode/lte_bandpref --slot 9)
check "slot bounds reject" "echo '$out' | grep -q 'Invalid slot'"

# --- backups + emergency restore ---
out=$("$BIN" backup restore latest)
check "emergency restore latest" "echo '$out' | grep -q '\"ok\":true'"

# --- HTTP bridge (serve) ---
"$BIN" serve --port 28088 >"$WORK/serve.log" 2>&1 &
SVPID=$!
sleep 1
h=$(curl -s http://127.0.0.1:28088/health)
check "http health" "echo '$h' | grep -q 'ok'"
a=$(curl -s -X POST http://127.0.0.1:28088/api -d '{"cmd":"backup restore latest","args":{}}')
check "http subcommand tail" "echo '$a' | grep -q 'ok'"
b=$(curl -s -X POST http://127.0.0.1:28088/api -d '{"cmd":"bandlock set","args":{"lte":"1,3","slot":"1"}}')
check "http string slot accepted" "echo '$b' | grep -q 'lte_bandpref_Subscription01'"
kill $SVPID 2>/dev/null; wait $SVPID 2>/dev/null || true

echo
[ "$fail" = "0" ] && echo "ALL SMOKE TESTS PASSED" || { echo "SMOKE FAILURES"; exit 1; }
