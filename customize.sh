#!/system/bin/sh
# MTB Tool module installer (Magisk / KernelSU / APatch)
# Safe default: never applies any NV change at install time.

DATA_DIR=/data/adb/mtbtool
MTB=/vendor/bin/mtb

ui_print "*******************************"
ui_print " MTB Tool v$VERSION"
ui_print "*******************************"

# --- backend detection ---
if [ -n "$KSU" ]; then
    ui_print "- KernelSU detected (version $KSU_KERNEL_VER_CODE)"
elif [ -n "$APATCH" ]; then
    ui_print "- APatch detected"
elif [ -n "$MAGISK_VER_CODE" ]; then
    ui_print "- Magisk detected ($MAGISK_VER)"
else
    ui_print "- Manager: unknown (assume Magisk-compatible)"
fi

# --- required vendor binary ---
if [ ! -x "$MTB" ]; then
    ui_print "! ERROR: $MTB not found or not executable."
    ui_print "! This device does not expose the Qualcomm MTB tool."
    ui_print "! Installation aborted — no changes were made."
    exit 1
fi
ui_print "- Found $MTB"

# --- data dir ---
mkdir -p "$DATA_DIR/backups" 2>/dev/null || {
    ui_print "! ERROR: cannot create $DATA_DIR"
    exit 1
}
chmod 0700 "$DATA_DIR"
ui_print "- Data dir ready: $DATA_DIR"

# --- cleanup stale runtime files ---
rm -f "$DATA_DIR/serve.pid" "$DATA_DIR/.lock"

set_perm_recursive $MODPATH 0 0 0755 0644
set_perm $MODPATH/bin/mtbctl 0 0 0755
set_perm $MODPATH/service.sh 0 0 0755
set_perm $MODPATH/action.sh 0 0 0755

ui_print "- Installed. Open the module WebUI to manage EFS NV items."
ui_print "- Emergency restore: module menu (action.sh) or WebUI Backups tab."
