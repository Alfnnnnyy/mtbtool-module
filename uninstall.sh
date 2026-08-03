#!/system/bin/sh
# MTB Tool — uninstall.sh
# Preserves /data/adb/mtbtool (backups + config) by design: NV history is
# user data. Remove that directory manually if a full wipe is wanted.

MODDIR=${0%/*}
DATA_DIR=/data/adb/mtbtool

# stop the HTTP bridge if running
if [ -f "$DATA_DIR/serve.pid" ]; then
    kill "$(cat "$DATA_DIR/serve.pid")" 2>/dev/null
    rm -f "$DATA_DIR/serve.pid"
fi

echo "MTB Tool uninstalled."
echo "Backups kept at $DATA_DIR (delete manually to wipe)."
