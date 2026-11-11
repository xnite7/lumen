#!/bin/bash
# Screenshot the GPUI app under Xvfb (lavapipe). usage: tools/gshot.sh out.png [secs] [WxH]
OUT=$1; SECS=${2:-8}; SIZE=${3:-1440x900}
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json
BIN=${BIN:-/home/claude/lumen/desktop/target/debug/couch}
xvfb-run -a -s "-screen 0 ${SIZE}x24" bash -c "
  $BIN > /tmp/gshot.log 2>&1 & P=\$!
  sleep 3; W=\$(xdotool search --onlyvisible --name '' | tail -1)
  xdotool windowunmap \$W; xdotool windowmap \$W
  sleep $SECS
  ffmpeg -loglevel error -y -f x11grab -video_size $SIZE -i \$DISPLAY -frames:v 1 $OUT
  kill \$P"
