#!/bin/sh

query() {
  file=$1
  stream=$2
  ffprobe -v quiet -show_streams -select_streams "$stream" "$file"
}

if [ $# -eq 0 ]; then
  files="./*"
else
  files=$*
fi

for file in $files; do
  printf %s\\n "checking file $file"
  if [ -z "$(query "$file" v)" ]; then
    printf %s\\n "file $file is missing video stream"
    continue
  elif [ -z "$(query "$file" a:m:language:jpn)" ]; then
    printf %s\\n "file $file is missing jpn audio stream"
    continue
  elif [ -z "$(query "$file" s:m:language:eng)" ]; then
    printf %s\\n "file $file is missing eng subtitle stream"
    continue
  fi
  printf %s\\n "file $file is valid"

  printf %s\\n "probing file $file"
  crf=$(
    ab-av1 crf-search \
      -i "$file" \
      --pix-format yuv420p10le \
      --preset 3 \
      --enc-input hwaccel=auto \
      --min-vmaf 96 \
      --thorough | grep predicted | cut -d ' ' -f 2
  )
  if [ "$crf" = "" ]; then
    crf=24
  fi
  printf %s\\n "using crf $crf for file $file"

  printf %s\\n "encoding file $file"
  ffmpeg \
    -hwaccel auto \
    -i "$file" \
    -pix_fmt yuv420p10le \
    -c:v libsvtav1 \
    -crf "$crf" \
    -preset 3 \
    -c:a libopus \
    -b:a 160k \
    -c:s copy \
    -map 0:V \
    -map 0:a:m:language:jpn \
    -map 0:s:m:language:eng \
    "${file%.*}.enc.mkv"
  printf %s\\n "finished encoding file $file"
done
