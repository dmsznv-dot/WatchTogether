#!/usr/bin/env python3
"""Download a pinned x64 libmpv build, verify it, and include it in installers."""
import hashlib
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request

URL = "https://github.com/shinchiro/mpv-winbuild-cmake/releases/download/20261021/mpv-dev-x86_64-20261021-git-e471496c56.7z"
SHA256 = "efc719eff3dc5954b4a5efc800632808bcec01cc9034940dd14e5eb7fe9ea633"

def setup_windows_mpv():
    root = Path(__file__).resolve().parent.parent
    with tempfile.TemporaryDirectory() as tmp:
        archive = Path(tmp) / "mpv.7z"
        urllib.request.urlretrieve(URL, archive)
        if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
            raise RuntimeError("Downloaded libmpv checksum does not match pinned release")
        subprocess.run(["7z", "x", str(archive), f"-o{tmp}/unpacked", "-y"], check=True)
        dlls = list((Path(tmp) / "unpacked").rglob("*.dll"))
        mpv = next((p for p in dlls if p.name in ("mpv-2.dll", "libmpv-2.dll")), None)
        if not mpv:
            raise RuntimeError("libmpv DLL not found in archive")
        for directory in (root / "target/debug", root / "target/release", root / "apps/desktop/src-tauri"):
            directory.mkdir(parents=True, exist_ok=True)
            shutil.copy2(mpv, directory / "mpv-2.dll")
    print("Verified libmpv is ready for development and packaging")

if __name__ == "__main__":
    setup_windows_mpv()
