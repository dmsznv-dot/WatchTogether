#!/usr/bin/env python3
"""Create a small, real MKV with two audio tracks and a Cyrillic SRT sidecar."""
from pathlib import Path
import subprocess
import sys
root = Path(sys.argv[1] if len(sys.argv) > 1 else "target/player-fixtures")
root.mkdir(parents=True, exist_ok=True)
video = root / "multi.mkv"
embedded = root / "embedded.ass"
embedded.write_text("""[Script Info]
ScriptType: v4.00+
PlayResX: 320
PlayResY: 180
[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,1,0,2,10,10,10,1
[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:09.00,Default,,0,0,0,,Встроенные ASS субтитры
""", encoding="utf-8")
subprocess.run([
    "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
    "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=24",
    "-f", "lavfi", "-i", "sine=frequency=440",
    "-f", "lavfi", "-i", "sine=frequency=880",
    "-i", str(embedded),
    "-t", "10", "-map", "0:v", "-map", "1:a", "-map", "2:a", "-map", "3:s",
    "-c:v", "libx264", "-c:a", "aac", "-c:s", "ass",
    "-metadata:s:a:0", "language=eng", "-metadata:s:a:1", "language=rus", str(video)
], check=True)
video.with_suffix(".srt").write_text("1\n00:00:00,000 --> 00:00:09,000\nТест субтитров — Watch Together\n", encoding="utf-8")
print(video.resolve())
