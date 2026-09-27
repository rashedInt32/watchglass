#!/usr/bin/env sh
# docs/demo.webm → docs/demo.mp4 (share) and docs/demo.gif (README).
set -eu
cd "$(dirname "$0")/.."
test -f docs/demo.webm || { echo "run pnpm demo first"; exit 1; }

# MP4: H.264, even dimensions, plays everywhere.
ffmpeg -y -loglevel error -i docs/demo.webm \
  -vf "scale=trunc(iw/2)*2:trunc(ih/2)*2" \
  -c:v libx264 -preset slow -crf 20 -pix_fmt yuv420p -movflags +faststart \
  docs/demo.mp4

# GIF: 1000 px wide, 12 fps, two-pass palette for clean text.
ffmpeg -y -loglevel error -i docs/demo.webm \
  -vf "fps=12,scale=1000:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=4" \
  docs/demo.gif

ls -la docs/demo.mp4 docs/demo.gif | awk '{print $5, $9}'
