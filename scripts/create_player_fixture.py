#!/usr/bin/env python3
"""Create a small, real MKV with two audio tracks and a Cyrillic SRT sidecar,
an external audio file (dub.m4a) and a 1080p60 clip (hd60.mp4) for frame-rate checks."""
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
# Separate audio file (e.g. a dub) for external audio track checks.
subprocess.run([
    "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
    "-f", "lavfi", "-i", "sine=frequency=660", "-t", "10", "-c:a", "aac",
    str(root / "dub.m4a")
], check=True)
# HDR10 clip (HEVC 10-bit, PQ, BT.2020) with known luminance: three bars of 1000, 203
# (SDR reference white) and 100 nits, black along the bottom. 10-bit limited-range PQ
# codes: 1000 nits -> 723, 203 -> 573, 100 -> 509, black -> 64.
subprocess.run([
    "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
    "-f", "lavfi", "-i", "color=black:s=640x360:r=30,format=yuv420p10le",
    "-vf", "geq=lum='if(gt(Y,H*3/4),64,if(lt(X,W/3),723,if(lt(X,W*2/3),573,509)))':cb=512:cr=512",
    "-t", "5", "-c:v", "libx265", "-pix_fmt", "yuv420p10le",
    "-x265-params", "log-level=error:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:range=limited:hdr10=1:max-cll=1000,400:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1)",
    "-color_primaries", "bt2020", "-color_trc", "smpte2084", "-colorspace", "bt2020nc", "-color_range", "tv",
    str(root / "hdr10.mp4")
], check=True)
# 1080p60 clip for frame-rate checks of the video output paths.
subprocess.run([
    "ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-y",
    "-f", "lavfi", "-i", "testsrc2=size=1920x1080:rate=60",
    "-t", "6", "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
    str(root / "hd60.mp4")
], check=True)
video.with_suffix(".srt").write_text("1\n00:00:00,000 --> 00:00:09,000\nТест субтитров — Watch Together\n", encoding="utf-8")
print(video.resolve())
