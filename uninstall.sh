#!/system/bin/sh
# MTB Tool — uninstall.sh
# Preserves /data/adb/mtbtool (backups + config) by design: NV history is
# user data. Remove that directory manually if a full wipe is wanted.

MODDIR=${0%/*}
DATA_DIR=/data/adb/mtbtool

echo "MTB Tool uninstalled."
echo "Backups kept at $DATA_DIR (delete manually to wipe)."
