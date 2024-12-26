#!/bin/sh

for file in ./*; do
  echo "probing file $file"
  crf=$(
    ab-av1 crf-search \
      -i "$file" \
      --pix-format yuv420p \
      --preset 3 \
      --enc-input hwaccel=auto \
      --min-vmaf 96 \
      --thorough | grep predicted | cut -d ' ' -f 2
  )
  echo "using crf $crf for file $file"

  echo "encoding file $file"
  ffmpeg \
    -hwaccel auto \
    -i $file \
    -pix_fmt yuv420p \
    -c:v libsvtav1 \
    -crf "$crf" \
    -preset 3 \
    -c:a libopus \
    -b:a 160k \
    -map 0:v \
    -map 0:a:m:language:jpn \
    -map 0:s:m:language:eng \
    "${file%.*}.enc.mkv"
  echo "finished encoding file $file"
done
