#!/usr/bin/env python3
"""
Bundle libmpv and its Homebrew dependencies into Watch Together.app/Contents/Frameworks
Makes the macOS bundle completely self-contained without requiring Homebrew on client machines.
"""

import os
import sys
import shutil
import subprocess
from pathlib import Path

def run(cmd):
    res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        print(f"Warning running {' '.join(cmd)}: {res.stderr.strip()}", file=sys.stderr)
    return res.stdout.strip()

def get_dependencies(dylib_path):
    out = run(["otool", "-L", str(dylib_path)])
    deps = []
    for line in out.splitlines()[1:]:
        line = line.strip()
        if not line:
            continue
        dep = line.split(" ")[0]
        # Only collect non-system dependencies (Homebrew libraries)
        if dep.startswith("/opt/homebrew/") or dep.startswith("/usr/local/"):
            deps.append(dep)
    return deps

def bundle_app(app_path):
    app_path = Path(app_path).resolve()
    if not app_path.exists():
        print(f"Error: {app_path} does not exist", file=sys.stderr)
        sys.exit(1)

    frameworks_dir = app_path / "Contents" / "Frameworks"
    macos_dir = app_path / "Contents" / "MacOS"
    frameworks_dir.mkdir(parents=True, exist_ok=True)

    # Locate initial libmpv
    libmpv_candidates = [
        Path("/opt/homebrew/lib/libmpv.2.dylib"),
        Path("/opt/homebrew/lib/libmpv.dylib"),
        Path("/usr/local/lib/libmpv.2.dylib"),
        Path("/usr/local/lib/libmpv.dylib"),
    ]
    libmpv_src = None
    for c in libmpv_candidates:
        if c.exists():
            libmpv_src = c.resolve()
            break

    if not libmpv_src:
        print("Error: libmpv not found on host machine", file=sys.stderr)
        sys.exit(1)

    print(f"Bundling starting from libmpv: {libmpv_src}")

    # BFS copy and resolve dependencies
    queue = [libmpv_src]
    copied = {}  # orig_path_str -> target_path

    while queue:
        current = queue.pop(0)
        canon = current.resolve()
        name = canon.name
        target = frameworks_dir / name

        if str(canon) in copied:
            continue

        if not target.exists() or target.stat().st_size != canon.stat().st_size:
            print(f"  Copying {name}...")
            shutil.copy2(canon, target)
            os.chmod(target, 0o755)

        copied[str(canon)] = target
        copied[str(current)] = target

        deps = get_dependencies(canon)
        for dep in deps:
            dep_path = Path(dep)
            if dep_path.exists() and str(dep_path.resolve()) not in copied:
                queue.append(dep_path)

    print(f"Copied {len(copied)} dynamic libraries into Frameworks.")

    # Ensure libmpv.dylib alias exists if libmpv.2.dylib is present
    p2 = frameworks_dir / "libmpv.2.dylib"
    p = frameworks_dir / "libmpv.dylib"
    if p2.exists() and not p.exists():
        try:
            p.symlink_to("libmpv.2.dylib")
        except Exception:
            shutil.copy2(p2, p)

    # Fix install names and rpaths for all libraries in Frameworks
    for lib in frameworks_dir.glob("*.dylib"):
        lib_name = lib.name
        # Set its own id
        run(["install_name_tool", "-id", f"@rpath/{lib_name}", str(lib)])

        # Update dependent paths
        out = run(["otool", "-L", str(lib)])
        for line in out.splitlines()[1:]:
            dep = line.strip().split(" ")[0]
            if dep.startswith("/opt/homebrew/") or dep.startswith("/usr/local/"):
                dep_name = Path(dep).resolve().name
                run(["install_name_tool", "-change", dep, f"@rpath/{dep_name}", str(lib)])

    # Also make sure the main binary in MacOS has @executable_path/../Frameworks in rpath
    for exe in macos_dir.iterdir():
        if exe.is_file() and not exe.is_symlink():
            print(f"Adding @executable_path/../Frameworks to rpath of {exe.name}...")
            run(["install_name_tool", "-add_rpath", "@executable_path/../Frameworks", str(exe)])
            run(["install_name_tool", "-add_rpath", "@loader_path/../Frameworks", str(exe)])

    # Ad-hoc sign the app so macOS Gatekeeper / AMFI doesn't complain about modified dylibs
    print("Applying ad-hoc code signature to all bundled frameworks and main app...")
    run(["codesign", "--force", "--deep", "-s", "-", str(app_path)])
    print("Done! Bundle is fully self-contained.")

if __name__ == "__main__":
    app_target = sys.argv[1] if len(sys.argv) > 1 else "target/release/bundle/macos/Watch Together.app"
    bundle_app(app_target)
