#!/bin/bash
# Fake /vendor/bin/mtb for host smoke tests — emulates real mtb output formats.
STORE="${FAKE_MTB_STORE:-/tmp/fakemtb/store}"
key() { echo "$1|$2" | tr '/' '_'; }
case "$1" in
  0) echo "mtb version 1.0"; exit 0 ;;
  4)
    op="$2"; slot="$3"; path="$4"
    k=$(key "$slot" "$path")
    case "$op" in
      4) # read — REAL peridot format: per-byte tagged lines, printed TWICE
        if [ -f "$STORE/.readfail" ]; then
          rm -f "$STORE/.readfail"
          exit 3
        fi
        # (mtb: block then RIL block), plus a "data len(N)" declaration.
        # FAKE_MTB_FAIL_PATH => exit 3 (read error).
        # FAKE_MTB_QMI_FAIL=1  => exit 0 but QMI failure markers, no bytes
        # (parser must report Error, not Absent).
        if [ -n "${FAKE_MTB_FAIL_PATH:-}" ] && [[ "$path" == *"$FAKE_MTB_FAIL_PATH"* ]]; then
          exit 3
        fi
        if [ "${FAKE_MTB_QMI_FAIL:-0}" = "1" ]; then
          echo "mtb: [mtb][xiaomi_extend_nvefs.c:506] xiaomi_efs_read: result = 0, rsp.result = -117"
          echo "mtb: [mtb][xiaomi_extend_nvefs.c:516] xiaomi_efs_read: qmi response fail"
          echo "mtb: [mtb][cpp:172] xiaomi_nvefs_test_efs_read: xiaomi_extend_qmi_send_sync fail, REQUEST_ID_EFS"
          exit 0
        fi
        if [ -f "$STORE/$k" ]; then
          hex=$(cat "$STORE/$k")
          n=$(( ${#hex} / 2 ))
          echo "mtb: [mtb][cpp:176] xiaomi_nvefs_test_efs_read: data len($n)"
          echo "$hex" | fold -w2 | while read -r b; do
            echo "mtb: [mtb][cpp:179] xiaomi_nvefs_test_efs_read:  $b"
          done
          echo "RIL[xc:176] xiaomi_nvefs_test_efs_read: data len($n)"
          echo "$hex" | fold -w2 | while read -r b; do
            echo "RIL[xc:179] xiaomi_nvefs_test_efs_read:  $b"
          done
        fi
        exit 0
        ;;
      5) # write: decimal bytes follow
        shift 4
        hex=""
        for b in "$@"; do hex="$hex$(printf '%02x' "$b")"; done
        if [ -n "${FAKE_MTB_FAIL_WRITE:-}" ] && [[ "$hex" == *"$FAKE_MTB_FAIL_WRITE"* ]]; then
          hex="00"
        fi
        # error-injection modes: the write IS attempted, then exit nonzero.
        # Each mode is ONE-SHOT (flag file) so rollback writes inside the
        # same test behave normally instead of re-triggering the mode.
        if [ "${FAKE_MTB_WRITE_ERR_STORE_TARGET:-0}" = "1" ] && [ ! -f "$STORE/.err_store_target" ]; then
          printf '%s' "$hex" > "$STORE/$k"; touch "$STORE/.err_store_target"; exit 1
        fi
        if [ "${FAKE_MTB_WRITE_ERR_STORE_BAD:-0}" = "1" ] && [ ! -f "$STORE/.err_store_bad" ]; then
          printf '00' > "$STORE/$k"; touch "$STORE/.err_store_bad"; exit 1
        fi
        if [ "${FAKE_MTB_WRITE_ERR_NOCHANGE:-0}" = "1" ] && [ ! -f "$STORE/.err_nochange" ]; then
          touch "$STORE/.err_nochange"; exit 1
        fi
        if [ "${FAKE_MTB_WRITE_ERR_READFAIL:-0}" = "1" ] && [ ! -f "$STORE/.err_readfail" ]; then
          printf '%s' "$hex" > "$STORE/$k"
          touch "$STORE/.readfail" "$STORE/.err_readfail"
          exit 1
        fi
        printf '%s' "$hex" > "$STORE/$k"
        exit 0
        ;;
      6) rm -f "$STORE/$k"; exit 0 ;;
    esac
    ;;
  5) # DIAG (read = arg8 "4" per DIAG_READ_ARGS: 5 0 0 0 1000 75 19 4 ...)
    if [ "$8" = "4" ]; then
      if [ "${FAKE_MTB_DIAG_REAL:-0}" = "1" ]; then
        # Real peridot 11-byte generic response (data_size=11) — unsupported
        # for band-mask detection; parser must reject, not fall back.
        echo "mtb: [mtb][xd:349] response from 89[Unknown], len = 11"
        echo "mtb: [mtb][xd:410] callback, cmd_code = 0x15, data_size = 11"
        echo "rsp data len(11)"
        echo "rsp data: 0x15 0x4B 0x13 0x04 0x00 0x00 0x00 0x00 0x33 0x9D 0x7E"
        exit 0
      fi
      python3 - <<'PYEOF'
data = bytearray(200)
mask = 0
for b in (1, 3, 7, 8, 40): mask |= 1 << (b - 1)
data[36:45] = mask.to_bytes(9, 'little')
def set_nr(offset, bands):
    m = 0
    for b in bands: m |= 1 << (b - 1)
    data[offset:offset + 10] = m.to_bytes(10, 'little')
set_nr(108, [1, 2, 5])       # NR region 1 (SA)
set_nr(172, [7, 8, 14])      # NR region 2 (NSA), >= 10 apart
print("rsp data: " + " ".join(f"0x{b:02x}" for b in data))
PYEOF
      exit 0
    fi
    exit 0
    ;;
  9)
    opt="$2"
    case "$opt" in
      31) echo "TX INFO: tx_power = 18, something = 1"; exit 0 ;;
      0)  echo "ASDIV DATA: earfcn: 1650, pci: 102, rsrp_rx0: -92.0, rsrq_rx0: -9.5, rssi_rx0: -75.0, snr_rx0: 18.2"; exit 0 ;;
      10) echo "ASDIV DATA: rsrp_rx0: -88.3, rsrq: -8.1"; exit 0 ;;
    esac
    exit 0
    ;;
  11) echo "modem restarting"; exit 0 ;;
esac
echo "unknown args: $*" >&2
exit 1
