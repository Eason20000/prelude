#!/usr/bin/env bash
# Post-generate Android patches (idempotent; safe to re-run).
#
# SPDX-License-Identifier: GPL-3.0-only
#
# Pixiewood regenerates .pixiewood/android from scratch, so anything custom
# must be (re-)applied after every `pixiewood generate`:
#   1. install PreludeActivity (ToplevelActivity subclass with MIDI bootstrap)
#   2. point the manifest launcher activity at it (surgical attribute swap;
#      intent-filters and everything else stay as generated)
#   3. declare the MIDI feature (not required: devices without MIDI still
#      install; the app degrades to "no ports")
#
# Usage: ./apply-android-patches.sh   (run inside android/)
set -euo pipefail

GEN=".pixiewood/android"
MANIFEST="$GEN/app/src/main/AndroidManifest.xml"
JAVA_DST="$GEN/app/src/main/java/top/vikasmi/prelude"

[ -f "$MANIFEST" ] || {
    echo "apply-android-patches: $MANIFEST missing; run pixiewood generate first" >&2
    exit 1
}

# 1. Activity subclass.
mkdir -p "$JAVA_DST"
cp "android-java/top/vikasmi/prelude/PreludeActivity.java" "$JAVA_DST/"

# 2. Launcher activity swap (exactly one stock declaration expected unless
# already patched).
if grep -q 'android:name="top.vikasmi.prelude.PreludeActivity"' "$MANIFEST"; then
    echo "apply-android-patches: activity already patched"
else
    count=$(grep -c 'android:name="org.gtk.android.ToplevelActivity"' "$MANIFEST" || true)
    [ "$count" = "1" ] || {
        echo "apply-android-patches: expected 1 stock activity, found $count (upstream template drift?)" >&2
        exit 1
    }
    sed -i 's|android:name="org.gtk.android.ToplevelActivity"|android:name="top.vikasmi.prelude.PreludeActivity"|' "$MANIFEST"
    echo "apply-android-patches: activity swapped"
fi

# 3. MIDI feature declaration.
if grep -q 'android.software.midi' "$MANIFEST"; then
    echo "apply-android-patches: uses-feature present"
else
    sed -i 's|<uses-permission android:name="android.permission.REORDER_TASKS"/>|<uses-permission android:name="android.permission.REORDER_TASKS"/>\n  <uses-feature android:name="android.software.midi" android:required="false"/>|' "$MANIFEST"
    echo "apply-android-patches: uses-feature added"
fi

# 4. Patch fontconfig test-spacing to include libintl_dep.
#
# Upstream fontconfig 2.18+ test-spacing.c includes fcint.h which #include
# <libintl.h> when ENABLE_NLS, but the meson test entry only declares
# freetype_dep — missing libintl_dep.  This causes "libintl.h file not found"
# on cross-compile (Pixiewood Android).  Patch the generated meson.build
# in-place; idempotent via grep guard.
FC_TEST="../android/subprojects/fontconfig/test/meson.build"
if [ -f "$FC_TEST" ] && ! grep -q "test-spacing.*libintl_dep" "$FC_TEST"; then
    sed -i "s|\['test-spacing\.c', {'include_directories': include_directories('\.\./src'), 'dependencies': freetype_dep}\],|\['test-spacing.c', {'include_directories': include_directories('../src'), 'dependencies': [freetype_dep, libintl_dep]}],|" "$FC_TEST"
    echo "apply-android-patches: patched fontconfig test-spacing libintl_dep"
fi
