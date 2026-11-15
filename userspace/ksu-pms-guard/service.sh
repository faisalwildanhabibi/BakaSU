#!/system/bin/sh
MODDIR=${0%/*}

# Ensure ksu working directory exists
mkdir -p /data/adb/ksu

# Initialize default hide_list for Dual-World isolation
HIDE_LIST="/data/adb/ksu/.hide_list"
if [ ! -f "$HIDE_LIST" ]; then
    touch "$HIDE_LIST"
    echo "# KernelSU Dual-World Hidden Packages" >> "$HIDE_LIST"
    echo "me.weishu.kernelsu" >> "$HIDE_LIST"
    echo "org.bakasu.bakasu" >> "$HIDE_LIST"
    echo "io.github.a13e300.ksu" >> "$HIDE_LIST"
    echo "com.topjohnwu.magisk" >> "$HIDE_LIST"
    echo "io.github.vvb2060.magisk" >> "$HIDE_LIST"
    echo "org.lsposed.manager" >> "$HIDE_LIST"
    chmod 0644 "$HIDE_LIST"
fi

# Initialize default target list (empty means all non-root apps are guarded)
TARGET_LIST="/data/adb/ksu/target.txt"
if [ ! -f "$TARGET_LIST" ]; then
    touch "$TARGET_LIST"
    chmod 0644 "$TARGET_LIST"
fi

# Initialize default sus_paths for SuSFS / TargetedHide filesystem cloaking
SUS_PATHS="/data/adb/ksu/sus_paths.txt"
if [ ! -f "$SUS_PATHS" ]; then
    touch "$SUS_PATHS"
    echo "# SuSFS / TargetedHide VFS Cloak Paths" >> "$SUS_PATHS"
    echo "/data/adb/ksu" >> "$SUS_PATHS"
    echo "/data/adb/modules" >> "$SUS_PATHS"
    chmod 0644 "$SUS_PATHS"
fi
