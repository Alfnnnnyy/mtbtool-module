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
      4) # read: tag per 16-byte line; absent => exit 0 empty (real modem behavior)
        if [ -n "${FAKE_MTB_FAIL_PATH:-}" ] && [[ "$path" == *"$FAKE_MTB_FAIL_PATH"* ]]; then
          exit 3
        fi
        if [ -f "$STORE/$k" ]; then
          hex=$(cat "$STORE/$k")
          echo "$hex" | fold -w2 | while read -r b; do
            echo "xiaomi_nvefs_test_efs_read: $b"
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
        printf '%s' "$hex" > "$STORE/$k"
        exit 0
        ;;
      6) rm -f "$STORE/$k"; exit 0 ;;
    esac
    ;;
  5) # DIAG (read = arg8 "4" per DIAG_READ_ARGS: 5 0 0 0 1000 75 19 4 ...)
    if [ "$8" = "4" ]; then
      python3 - <<'PYEOF'
data = bytearray(200)
mask = 0
for b in (1,3,7,8,40): mask |= 1 << (b-1)
data[36:45] = mask.to_bytes(9, 'little')
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
