#!/usr/bin/env python3
"""
Bundle libmpv and its Homebrew dependencies into Watch Together.app/Contents/Frameworks
Makes the macOS bundle completely self-contained without requiring Homebrew on client machines.

Dependencies are followed whether they are referenced by absolute Homebrew path,
@rpath/ or @loader_path/. Every bundled library is referenced by the same leaf
name it is linked against, so @rpath lookups inside Frameworks always resolve.
The script fails if the result still points outside the bundle or needs a newer
macOS than the app's LSMinimumSystemVersion.
"""

import os
import plistlib
import sys
import shutil
import subprocess
from pathlib import Path

HOMEBREW_PREFIXES = ("/opt/homebrew/", "/usr/local/")
HOMEBREW_LIB_DIRS = [Path("/opt/homebrew/lib"), Path("/usr/local/lib")]


def run(cmd, check=False):
    res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if res.returncode != 0:
        if check:
            print(f"Error running {' '.join(cmd)}: {res.stderr.strip()}", file=sys.stderr)
            sys.exit(1)
        print(f"Warning running {' '.join(cmd)}: {res.stderr.strip()}", file=sys.stderr)
    return res.stdout.strip()


def linked_libraries(path):
    """Install names a Mach-O file links against (excluding its own id)."""
    out = run(["otool", "-L", str(path)], check=True).splitlines()[1:]
    deps = [line.strip().split(" ")[0] for line in out if line.strip()]
    own_id = run(["otool", "-D", str(path)]).splitlines()[1:]
    return [d for d in deps if not own_id or d != own_id[0].strip()]


def rpaths(path):
    out = run(["otool", "-l", str(path)], check=True).splitlines()
    result = []
    for i, line in enumerate(out):
        if line.strip() == "cmd LC_RPATH":
            result.append(out[i + 2].strip().split(" ")[1])
    return result


def min_os(path):
    out = run(["otool", "-l", str(path)], check=True).splitlines()
    for i, line in enumerate(out):
        if line.strip() == "cmd LC_BUILD_VERSION":
            for follow in out[i + 1:i + 6]:
                if follow.strip().startswith("minos"):
                    return follow.split()[1]
        if line.strip() == "cmd LC_VERSION_MIN_MACOSX":
            return out[i + 2].split()[1]
    return None


def version_tuple(v):
    return tuple(int(x) for x in v.split("."))


def is_system(dep):
    return dep.startswith("/usr/lib/") or dep.startswith("/System/")


def resolve(dep, loader):
    """Find the on-disk file a dependency of `loader` refers to, or None for system libraries."""
    if is_system(dep):
        return None
    loader_dir = loader.parent
    if dep.startswith("@loader_path/"):
        candidates = [loader_dir / dep[len("@loader_path/"):]]
    elif dep.startswith("@rpath/"):
        leaf = dep[len("@rpath/"):]
        search = []
        for rp in rpaths(loader):
            rp = rp.replace("@loader_path", str(loader_dir))
            if not rp.startswith("@"):
                search.append(Path(rp))
        search += [loader_dir] + HOMEBREW_LIB_DIRS
        candidates = [d / leaf for d in search]
    else:
        candidates = [Path(dep)]
    for c in candidates:
        if c.exists():
            return c
    print(f"Error: cannot resolve {dep} (needed by {loader})", file=sys.stderr)
    sys.exit(1)


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
        Path("/usr/local/lib/libmpv.2.dylib"),
    ]
    libmpv_src = next((c for c in libmpv_candidates if c.exists()), None)
    if not libmpv_src:
        print("Error: libmpv not found on host machine", file=sys.stderr)
        sys.exit(1)

    print(f"Bundling starting from libmpv: {libmpv_src.resolve()}")

    # BFS over the dependency graph.
    # bundled: canonical source path -> leaf name inside Frameworks
    # references: (bundled leaf name, install name as linked) -> leaf name of the dependency
    bundled = {str(libmpv_src.resolve()): "libmpv.2.dylib"}
    references = []
    queue = [libmpv_src.resolve()]

    while queue:
        current = queue.pop(0)
        current_name = bundled[str(current)]
        for dep in linked_libraries(current):
            src = resolve(dep, current)
            if src is None:
                continue
            canon = str(src.resolve())
            if canon not in bundled:
                bundled[canon] = Path(dep).name
                queue.append(Path(canon))
            references.append((current_name, dep, bundled[canon]))

    for canon, name in bundled.items():
        target = frameworks_dir / name
        print(f"  Copying {name}...")
        if target.exists() or target.is_symlink():
            target.unlink()
        shutil.copy2(canon, target)
        os.chmod(target, 0o755)

    print(f"Copied {len(bundled)} dynamic libraries into Frameworks.")

    # Ensure libmpv.dylib alias exists for the loader's fallback name
    p = frameworks_dir / "libmpv.dylib"
    if not p.exists():
        p.symlink_to("libmpv.2.dylib")

    # Fix install names, references and rpaths so everything resolves inside Frameworks
    for name in bundled.values():
        lib = frameworks_dir / name
        run(["install_name_tool", "-id", f"@rpath/{name}", str(lib)], check=True)
        for rp in rpaths(lib):
            run(["install_name_tool", "-delete_rpath", rp, str(lib)], check=True)
        run(["install_name_tool", "-add_rpath", "@loader_path", str(lib)], check=True)
    for owner, dep, dep_name in references:
        if dep != f"@rpath/{dep_name}":
            run(["install_name_tool", "-change", dep, f"@rpath/{dep_name}",
                 str(frameworks_dir / owner)], check=True)

    # Also make sure the main binary in MacOS has @executable_path/../Frameworks in rpath
    for exe in macos_dir.iterdir():
        if exe.is_file() and not exe.is_symlink():
            existing = rpaths(exe)
            for rp in ("@executable_path/../Frameworks", "@loader_path/../Frameworks"):
                if rp not in existing:
                    print(f"Adding {rp} to rpath of {exe.name}...")
                    run(["install_name_tool", "-add_rpath", rp, str(exe)], check=True)

    verify_bundle(app_path)

    # Ad-hoc sign the app so macOS Gatekeeper / AMFI doesn't complain about modified dylibs
    print("Applying ad-hoc code signature to all bundled frameworks and main app...")
    run(["codesign", "--force", "--deep", "-s", "-", str(app_path)], check=True)
    print("Done! Bundle is fully self-contained.")


def verify_bundle(app_path):
    """Fail if any bundled Mach-O escapes the bundle or targets a newer macOS than declared."""
    contents = app_path / "Contents"
    frameworks_dir = contents / "Frameworks"
    with open(contents / "Info.plist", "rb") as f:
        declared = plistlib.load(f).get("LSMinimumSystemVersion", "10.13")

    binaries = [p for p in frameworks_dir.glob("*.dylib") if not p.is_symlink()]
    binaries += [p for p in (contents / "MacOS").iterdir() if p.is_file() and not p.is_symlink()]

    problems = []
    for binary in binaries:
        for dep in linked_libraries(binary):
            if is_system(dep):
                continue
            if dep.startswith("@rpath/"):
                if not (frameworks_dir / dep[len("@rpath/"):]).exists():
                    problems.append(f"{binary.name}: {dep} is missing from Frameworks")
            else:
                problems.append(f"{binary.name}: links outside the bundle: {dep}")
        for rp in rpaths(binary):
            if rp.startswith(HOMEBREW_PREFIXES):
                problems.append(f"{binary.name}: rpath points outside the bundle: {rp}")
        minos = min_os(binary)
        if minos and version_tuple(minos) > version_tuple(declared):
            problems.append(f"{binary.name}: requires macOS {minos}, app declares {declared}")

    if problems:
        print("Bundle verification FAILED:", file=sys.stderr)
        for p in problems:
            print(f"  {p}", file=sys.stderr)
        sys.exit(1)
    print(f"Verified {len(binaries)} Mach-O files: all dependencies bundled, all run on macOS {declared}+.")


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if a != "--verify-only"]
    app_target = args[0] if args else "target/release/bundle/macos/Watch Together.app"
    if "--verify-only" in sys.argv:
        verify_bundle(Path(app_target).resolve())
    else:
        bundle_app(app_target)
