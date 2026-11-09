## What's in this release

The first public release of Hat Timelapse: a complete command-line
timelapse workflow for LightCraft and Lightroom.

- **Keyframes**: edit a few frames and every frame in between follows,
  smoothly. Settings you don't animate are copied from the first keyframe to
  every frame.
- **Holy grail**: shutter, aperture and ISO changes during day-to-night shoots
  are compensated automatically.
- **Deflicker**: brightness is measured on every frame and flicker removed,
  while sunsets and keyframed changes are kept.
- **LightCraft rendering**: full-resolution 16-bit or 32-bit float TIFF.
  Tested with LightCraft 0.20.0 and Canon EOS R CR3 files.
- **Video**: H.264, HEVC, AV1, ProRes 422 HQ / 4444 / 4444 XQ and
  DNxHR HQ / HQX / 444, from HD to 8K and beyond, with motion blur.
- **Colour and HDR**: Rec.709, Display P3, Rec.2020, HDR10 (PQ) and HLG.
- **Pan and zoom** keyframes.
- **TIFF sequences** for EffectCraft and FilmCraft.
- **Long-term and construction filters**: working hours, weekdays, one frame
  per day, and frames much brighter or darker than their neighbours.
- **Batch**: many sequences, several formats, one command.

## Install

**macOS and Linux**: in Terminal, run:

```bash
curl -fsSL https://raw.githubusercontent.com/neilmanfredit/hat_timelapse/main/install.sh | sh
```

**Windows**: download `hat_timelapse-…-windows-x86_64.zip` below, unzip it,
and put `hat_timelapse.exe` in a folder on your PATH.

Then install the companion tools: LightCraft's command-line tool, ffmpeg and
exiftool. Check everything with:

```bash
hat_timelapse doctor
```

Full steps for each system, including LightCraft's separate macOS download:
[Installation guide](https://github.com/neilmanfredit/hat_timelapse/blob/main/docs/INSTALL.md).

## Quick start

```bash
cd ~/Timelapses/my-sequence
hat_timelapse init --keyframes 3
hat_timelapse keyframes label
hat_timelapse holy-grail          # only if you changed exposure during the shoot
# edit the keyframes in LightCraft or Lightroom and save metadata (⌘S)
hat_timelapse interpolate
hat_timelapse deflicker
hat_timelapse render
hat_timelapse video --size 4k --codec h264
```

Step-by-step instructions, options and recipes:
[User guide](https://github.com/neilmanfredit/hat_timelapse/blob/main/docs/USER_GUIDE.md).

## Downloads

| File | For |
| --- | --- |
| `hat_timelapse-…-macos-universal.tar.gz` | macOS 11 or later, Apple silicon and Intel |
| `hat_timelapse-…-linux-x86_64.tar.gz` | Linux on Intel/AMD |
| `hat_timelapse-…-linux-aarch64.tar.gz` | Linux on ARM (Raspberry Pi 5, Ampere, etc.) |
| `hat_timelapse-…-windows-x86_64.zip` | Windows 10 and 11 |
| `SHA256SUMS.txt` | Checksums to verify the downloads |
