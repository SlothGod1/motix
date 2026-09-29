#!/bin/sh
# Regenerates the probe fixtures. Needs FFmpeg 6+ with libx264, libx265, libvpx, libopus, libsvtav1 or libaom.
set -e
F="ffmpeg -hide_banner -loglevel error -y"
$F -f lavfi -i testsrc2=s=64x36:r=30000/1001:d=0.5 -f lavfi -i sine=f=440:r=48000:d=0.5 -ac 2 -c:v libx264 -pix_fmt yuv420p -c:a aac -b:a 32k -shortest h264_aac_2997.mp4
$F -f lavfi -i testsrc2=s=64x36:r=60:d=0.25 -c:v libx265 -x265-params log-level=none -pix_fmt yuv420p10le -color_primaries bt2020 -color_trc smpte2084 -colorspace bt2020nc -tag:v hvc1 hevc10_pq.mp4
$F -f lavfi -i testsrc2=s=64x36:r=50:d=0.2 -c:v libx265 -x265-params log-level=none -pix_fmt yuv420p10le -color_primaries bt2020 -color_trc arib-std-b67 -colorspace bt2020nc -tag:v hvc1 hevc10_hlg.mov
$F -display_rotation 90 -i h264_aac_2997.mp4 -c copy rotated_phone.mp4
$F -f lavfi -i sine=f=440:r=44100:d=0.5 -c:a aac -b:a 32k audio_only.m4a
$F -f lavfi -i sine=f=440:r=48000:d=0.5 -ac 2 -c:a pcm_s16le stereo_48k.wav
$F -f lavfi -i sine=f=440:r=96000:d=0.25 -ac 1 -c:a pcm_s24le mono_96k_24bit.wav
$F -f lavfi -i testsrc2=s=64x36:r=24:d=0.25 -f lavfi -i sine=r=48000:d=0.25 -c:v libvpx-vp9 -pix_fmt yuv420p -c:a libopus -b:a 24k -shortest vp9_opus.webm
$F -f lavfi -i testsrc2=s=64x36:r=25:d=0.2 -f lavfi -i sine=r=48000:d=0.2 -c:v libx264 -c:a flac -shortest h264_flac_25.mkv
$F -f lavfi -i testsrc2=s=48x64:d=0.04 -frames:v 1 still.png
$F -f lavfi -i testsrc2=s=72x40:d=0.04 -frames:v 1 still.jpg
$F -f lavfi -i testsrc2=s=64x36:r=30:d=0.2 -c:v libsvtav1 -pix_fmt yuv420p10le av1_10bit.mp4 2>/dev/null \
  || $F -f lavfi -i testsrc2=s=64x36:r=30:d=0.2 -c:v libaom-av1 -cpu-used 8 -pix_fmt yuv420p10le av1_10bit.mp4
