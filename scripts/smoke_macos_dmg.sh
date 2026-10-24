#!/bin/bash
# Verifies the packaged macOS DMG the way a user gets it: copies the app out of the
# image, checks every bundled library, loads the bundled libmpv and launches the app.
#
# Usage: scripts/smoke_macos_dmg.sh <dmg> [--hide-homebrew]
#   --hide-homebrew  move /opt/homebrew aside while loading and launching (CI only),
#                    so nothing can fall back to Homebrew's libmpv.
set -euo pipefail

DMG="$1"
HIDE_HOMEBREW="${2:-}"
SCRIPTS="$(cd "$(dirname "$0")" && pwd)"
WORK="$(mktemp -d)"
MOUNT="$WORK/mnt"
HIDDEN=""

cleanup() {
    if [ -n "$HIDDEN" ]; then sudo mv /opt/homebrew.hidden /opt/homebrew; fi
    if [ -n "${APP_PID:-}" ]; then kill "$APP_PID" 2>/dev/null || true; fi
    hdiutil detach "$MOUNT" -quiet 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT

echo "== Copying app out of $DMG =="
mkdir -p "$MOUNT"
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT" "$DMG" -quiet
ditto "$MOUNT/Watch Together.app" "$WORK/Watch Together.app"
hdiutil detach "$MOUNT" -quiet
APP="$WORK/Watch Together.app"
FRAMEWORKS="$APP/Contents/Frameworks"

echo "== Static checks =="
python3 "$SCRIPTS/bundle_mpv_macos.py" "$APP" --verify-only
codesign --verify --deep --strict "$APP"
echo "Code signature OK"

# Compiled before Homebrew is hidden; mirrors MpvApi::load (dlopen by absolute path).
cat > "$WORK/load_mpv.c" <<'EOF'
#include <dlfcn.h>
#include <stdio.h>
int main(int argc, char **argv) {
    void *lib = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!lib) { fprintf(stderr, "dlopen failed: %s\n", dlerror()); return 1; }
    void *(*create)(void) = (void *(*)(void))dlsym(lib, "mpv_create");
    int (*init)(void *) = (int (*)(void *))dlsym(lib, "mpv_initialize");
    void (*destroy)(void *) = (void (*)(void *))dlsym(lib, "mpv_terminate_destroy");
    if (!create || !init || !destroy) { fprintf(stderr, "libmpv symbols missing\n"); return 1; }
    void *mpv = create();
    if (!mpv) { fprintf(stderr, "mpv_create failed\n"); return 1; }
    int rc = init(mpv);
    destroy(mpv);
    if (rc < 0) { fprintf(stderr, "mpv_initialize failed: %d\n", rc); return 1; }
    printf("libmpv loaded and initialized\n");
    return 0;
}
EOF
clang -o "$WORK/load_mpv" "$WORK/load_mpv.c"

if [ "$HIDE_HOMEBREW" = "--hide-homebrew" ] && [ -d /opt/homebrew ]; then
    echo "== Hiding /opt/homebrew =="
    sudo mv /opt/homebrew /opt/homebrew.hidden
    HIDDEN=1
fi

# Any library loaded from outside the bundle means the bundle is incomplete.
fail_on_outside_libs() {
    if grep -E "dyld.*(/opt/homebrew|/usr/local)/" "$1"; then
        echo "FAIL: libraries were loaded from outside the app bundle (above)"
        exit 1
    fi
}

echo "== Loading bundled libmpv =="
DYLD_PRINT_LIBRARIES=1 "$WORK/load_mpv" "$FRAMEWORKS/libmpv.2.dylib" 2> "$WORK/load.log" || {
    tail -20 "$WORK/load.log"
    echo "FAIL: bundled libmpv does not load"
    exit 1
}
fail_on_outside_libs "$WORK/load.log"

echo "== Launching the app for 20 s =="
DYLD_PRINT_LIBRARIES=1 "$APP/Contents/MacOS/watch-together-desktop" > "$WORK/app.log" 2>&1 &
APP_PID=$!
sleep 20
if ! kill -0 "$APP_PID" 2>/dev/null; then
    { grep -v "^dyld" "$WORK/app.log" || true; } | tail -40
    echo "FAIL: app exited during startup (another running Watch Together also makes it exit)"
    exit 1
fi
fail_on_outside_libs "$WORK/app.log"
grep -c "dyld.*Frameworks/libmpv" "$WORK/app.log" > /dev/null || {
    echo "FAIL: the running app did not load the bundled libmpv"
    exit 1
}
echo "PASS: packaged app runs with only its bundled media libraries"
