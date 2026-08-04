#!/system/bin/sh
# MTB Tool — action.sh (module "Action" button)
# Emergency restore with explicit volume-key confirmation. Fails loudly.

MODDIR=${0%/*}
DATA_DIR=/data/adb/mtbtool
MTBCTL="$MODDIR/bin/mtbctl"

echo "*******************************"
echo " MTB Tool — Emergency restore"
echo "*******************************"
echo ""

if [ ! -x "$MTBCTL" ]; then
    echo "ERROR: $MTBCTL missing (module files incomplete)."
    exit 1
fi

# --- confirmation (volume keys; fail closed) ---
command -v getevent >/dev/null 2>&1 || {
    echo "ERROR: getevent unavailable — cannot confirm interactively."
    echo "Use the WebUI (Backups tab) instead."
    exit 1
}
echo "This will REVERT the modem to the latest backup."
echo "Press VOLUME UP to confirm, VOLUME DOWN to cancel (10s timeout)."
end=$(($(date +%s) + 10))
confirmed=0
while [ "$(date +%s)" -lt "$end" ]; do
    ev=$(timeout 1 getevent -lq 2>/dev/null | grep -m1 -E 'KEY_VOLUME(UP|DOWN).*DOWN' || true)
    [ -n "$ev" ] || continue
    case "$ev" in
        *VOLUMEUP*) confirmed=1 ;;
        *) confirmed=0 ;;
    esac
    break
done

if [ "$confirmed" -ne 1 ]; then
    echo "Cancelled (no VOLUME UP within 10s). No changes were made."
    exit 1
fi

echo "Confirmed. Reverting to the latest backup..."
if "$MTBCTL" backup restore latest; then
    echo ""
    echo "Restore completed. If the modem misbehaves, restart it (WebUI -> Modem restart)."
    exit 0
else
    RC=$?
    echo ""
    echo "ERROR: restore failed with code $RC. Check $DATA_DIR/backups."
    exit "$RC"
fi
