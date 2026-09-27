#!/usr/bin/env sh
# docs/demo-panel.webm + docs/demo.webm → docs/demo-panel.gif, docs/demo.gif
# (README) and docs/demo.mp4 (both parts, share).
set -eu
cd "$(dirname "$0")/.."
test -f docs/demo.webm -a -f docs/demo-panel.webm || { echo "run pnpm demo first"; exit 1; }

# MP4: the panel part padded onto the board's 2880x1800 canvas, then the board.
ffmpeg -y -loglevel error -i docs/demo-panel.webm -i docs/demo.webm \
  -filter_complex "[0:v]fps=30,pad=2880:1800:(ow-iw)/2:(oh-ih)/2:color=#0b1220[p];[1:v]fps=30,scale=2880:1800[b];[p][b]concat=n=2:v=1:a=0,format=yuv420p[v]" \
  -map "[v]" -c:v libx264 -preset slow -crf 20 -movflags +faststart \
  docs/demo.mp4

# GIFs: 12 fps, two-pass palette for clean text.
gif() {
  ffmpeg -y -loglevel error -i "$1" \
    -vf "fps=12,scale=$3:-1:flags=lanczos,split[s0][s1];[s0]palettegen=max_colors=128:stats_mode=diff[p];[s1][p]paletteuse=dither=bayer:bayer_scale=4" \
    "$2"
}
gif docs/demo-panel.webm docs/demo-panel.gif 560
gif docs/demo.webm docs/demo.gif 1000

ls -la docs/demo.mp4 docs/demo-panel.gif docs/demo.gif | awk '{print $5, $9}'
