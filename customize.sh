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
[ -x "$MTB" ] || abort "! ERROR: $MTB not found or not executable. This device does not expose the Qualcomm MTB tool. Installation aborted — no changes were made."
ui_print "- Found $MTB"

# --- data dir ---
mkdir -p "$DATA_DIR/backups" 2>/dev/null || abort "! ERROR: cannot create $DATA_DIR"
chmod 0700 "$DATA_DIR"
ui_print "- Data dir ready: $DATA_DIR"

# --- permissions: executables only ---
# KSU/APatch set webroot perms + SELinux context themselves, so we never touch
# webroot; Magisk needs the scripts and binary executable.
[ -n "$KSU" ] || [ -n "$APATCH" ] && ui_print "- KernelSU/APatch: webroot handled by manager"
set_perm $MODPATH/bin/mtbctl 0 0 0755
set_perm $MODPATH/action.sh 0 0 0755
set_perm $MODPATH/uninstall.sh 0 0 0755

# --- runtime probe (read-only) ---
"$MODPATH/bin/mtbctl" probe >/dev/null 2>&1 || abort "! ERROR: mtbctl failed its read-only compatibility probe."
ui_print "- mtbctl probe OK"

# --- cleanup stale runtime files ---
rm -f "$DATA_DIR/.lock"

ui_print "- Installed. Open the module WebUI to manage EFS NV items."
ui_print "- Emergency restore: module menu (action.sh) or WebUI Backups tab."