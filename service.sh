#!/system/bin/sh
# MTB Tool — service.sh
# Starts the localhost HTTP bridge (mtbctl serve) used by WebUI hosts that
# cannot exec directly (e.g. Magisk WebView hosts). Binds 127.0.0.1 ONLY.
# NEVER applies any NV/bandlock change at boot — by design.

MODDIR=${0%/*}
DATA_DIR=/data/adb/mtbtool
MTBCTL="$MODDIR/bin/mtbctl"
PORT=28082
PIDFILE="$DATA_DIR/serve.pid"
LOG="$DATA_DIR/serve.log"

[ -x "$MTBCTL" ] || exit 0
[ -x /vendor/bin/mtb ] || exit 0

# already running?
if [ -f "$PIDFILE" ] && kill -0 "$(cat "$PIDFILE" 2>/dev/null)" 2>/dev/null; then
    exit 0
fi
rm -f "$PIDFILE"

mkdir -p "$DATA_DIR"
nohup "$MTBCTL" serve --port "$PORT" >> "$LOG" 2>&1 &
echo $! > "$PIDFILE"
