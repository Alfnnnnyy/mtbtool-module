#!/system/bin/sh
# MTB Tool — action.sh (Module Usage / "Action" button)
# Emergency restore: reverts the latest backup. Intentionally the ONLY
# action exposed here — the WebUI is the primary interface.

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

echo "Reverting to the latest backup..."
"$MTBCTL" backup restore latest
echo ""
echo "Done. If the modem misbehaves, restart it (WebUI -> Modem restart)."
