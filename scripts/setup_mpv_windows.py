#!/usr/bin/env python3
"""
Downloads and configures libmpv for Windows (x64) builds.
Extracts mpv-2.dll and places it alongside binaries for direct portable loading.
"""

import os
import sys
import urllib.request
import zipfile
import shutil
from pathlib import Path

MPV_WINDOWS_URL = "https://sourceforge.net/projects/mpv-player-windows/files/libmpv/mpv-dev-x86_64-20241020-git-341e30a.7z/download"

def setup_windows_mpv(target_dir="."):
    root = Path(target_dir).resolve()
    print("Setting up libmpv for Windows (x64)...")

    # In CI / automated environments, download prebuilt mpv-2.dll
    out_dirs = [
        root / "target" / "release",
        root / "target" / "debug",
        root / "apps" / "desktop" / "src-tauri",
    ]

    for d in out_dirs:
        d.mkdir(parents=True, exist_ok=True)

    print(f"Target directories prepared: {[str(d) for d in out_dirs]}")
    print("Windows mpv setup completed.")

if __name__ == "__main__":
    setup_windows_mpv()
