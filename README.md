# Hat Timelapse

A timelapse workflow in Rust for Linux, macOS and Windows. It works alongside
[LightCraft](https://github.com/storytold/lightcraft) and Adobe Lightroom.

You edit a few keyframes in your photo editor. Hat Timelapse handles the rest: it
spreads those edits across every frame, fixes exposure jumps from day-to-night
shooting, removes flicker, frames the shot, and delivers a finished video or
a TIFF sequence ready for EffectCraft and FilmCraft.

```
 camera files ─► hat_timelapse init ─► keyframes ─► edit keyframes in LightCraft or Lightroom
                                                        │
                ┌───────────────────────────────────────┘
                ▼
     holy-grail ─► interpolate ─► deflicker ─► render (LightCraft) or export (Lightroom)
                                                        │
                ┌───────────────────────────────────────┘
                ▼
     framing: crop · pan/zoom · scale · colour space
                │
                ├─► video:  H.264 · HEVC · AV1 · ProRes · DNxHR, HD → 8K and beyond, SDR or HDR
                └─► TIFF:   16-bit or 32-bit float sequence for EffectCraft / FilmCraft
```

## Working with LightCraft, Lightroom and other editors

Hat Timelapse does not reimplement raw development. It uses the same open file
formats the editors already use:

| Hand-off | How |
| --- | --- |
| Develop settings | Camera Raw (`crs:`) properties in `.xmp` sidecars. Lightroom and LightCraft both read and write them. |
| Developing frames | `lightcraft-cli render` for every frame, with values passed as `--set control=value`. |
| Lightroom route | Lightroom reads the sidecars (*Metadata → Read Metadata from Files*). You export from Lightroom, then point Hat Timelapse at that folder. |
| Compositing and editing | A numbered TIFF sequence with a `sequence.json` manifest (frame rate, size, colour space), for EffectCraft and FilmCraft. |

## Requirements

- Rust 1.88 or newer to build.
- `ffmpeg` on the PATH for video. It needs libx264, libx265, prores_ks and
  dnxhd, plus libsvtav1 for AV1. `hat_timelapse doctor` checks which are present.
- `lightcraft-cli` on the PATH for the LightCraft route. Build it from the
  LightCraft repo with `cargo build --release -p lightcraft-cli`.
- `exiftool` (optional). It reads metadata from CR3, RAF and other raws that
  are not TIFF-based. DNG, CR2, NEF, ARW, ORF, PEF, RW2, JPEG and TIFF are read
  natively.

```bash
cargo build --release
```

The binary is `target/release/hat_timelapse`.

## Quick start: LightCraft

```bash
cd ~/Timelapses/sunset-2026-11-08
hat_timelapse init --keyframes 4        # scan, read EXIF, mark 4 keyframes
hat_timelapse keyframes label           # label keyframes so they stand out in the editor
hat_timelapse holy-grail                # compensate shutter/aperture/ISO changes
```

Open the keyframe images in LightCraft. Edit them and make sure sidecars are
written. Then run:

```bash
hat_timelapse interpolate               # spread the keyframe edits across all frames
hat_timelapse deflicker                 # remove remaining flicker
hat_timelapse render                    # develop every frame with LightCraft
hat_timelapse video --size 4k --codec h264
hat_timelapse video --size 4k --codec prores422-hq
hat_timelapse export --size 4k          # TIFF sequence for EffectCraft / FilmCraft
```

## Quick start: Lightroom

The steps are the same up to `deflicker`. After each Hat Timelapse step, select all
frames in Lightroom and run *Metadata → Read Metadata from Files*. Then export
the sequence from Lightroom to a folder, in TIFF or high-quality JPEG, keeping
the original file names:

```bash
hat_timelapse video --from ~/Exports/sunset --size 4k --codec hevc
```

Lightroom ignores `.xmp` sidecars for JPEG, TIFF and DNG files. Use raw files,
or use *Read Metadata from Files* after Hat Timelapse writes the sidecars.

## Commands

| Command | What it does |
| --- | --- |
| `init` | Scans the folder in natural filename order, reads exposure and capture time, and creates `hat_timelapse.json`. |
| `status` | Shows frames, keyframes, EV range, capture span, and the current holy-grail and deflicker figures. |
| `keyframes auto N` / `set 0 120 240` / `list` / `label` | Chooses keyframes and labels them in the sidecars. |
| `holy-grail` | Finds exposure changes from EXIF and offsets each frame back to the reference frame's settings. |
| `interpolate [--method smooth\|linear]` | Reads keyframe sidecars and interpolates every parameter they share. `smooth` is a monotone cubic, so it never overshoots a keyframe. |
| `deflicker [--source originals\|previews] [--window 15] [--roi x,y,w,h]` | Measures brightness and smooths it over the window. Raw files are measured from their embedded preview. |
| `filter time\|weekdays\|every\|outliers\|reset` | Long-term and construction filters (see below). |
| `motion add FRAME --zoom Z --cx X --cy Y` | Pan and zoom keyframes, interpolated across the sequence. |
| `write` | Rewrites every sidecar from the project. |
| `render [--previews]` | Develops included frames with LightCraft into `.hat_timelapse/rendered` (or small previews for deflicker). Frames that already exist are skipped, so it can be resumed. |
| `export` | Writes a framed TIFF sequence and `sequence.json`. |
| `video` | Frames and encodes a video. |
| `batch` | Runs any chain of steps over many projects. |
| `doctor` | Checks for LightCraft, ffmpeg and its encoders, and exiftool. |

### Keyframed parameters

These are interpolated across frames: Exposure, Contrast, Highlights, Shadows,
Whites, Blacks, Temperature, Tint, Texture, Clarity, Dehaze, Vibrance,
Saturation, and the crop rectangle and angle. Lightroom uses the crop values
for pan and zoom. Exposure carries Hat Timelapse's holy-grail and deflicker offsets
on top of your edit. When keyframes are read again, those offsets are removed
first, so re-running a step never moves your own values.

The mapping from Camera Raw names to LightCraft controls is in
`crates/hat_timelapse_core/src/params.rs`. You can override it per project under
`settings.lightcraft_controls` in `hat_timelapse.json`. The render command itself
is a template under `settings.lightcraft_command`.

## Output

### Sizes

`hd` 1280×720, `fhd` 1920×1080, `2.7k`, `4k`/`uhd` 3840×2160, `dci4k`
4096×2160, `5k`, `6k`, `8k` 7680×4320, `dci8k` 8192×4320, `12k`, or any
`WIDTHxHEIGHT`. The source is cropped to the output aspect ratio, with pan and
zoom applied, then resampled with Lanczos. Above 4K, x265 runs with
memory-saving settings.

### Codecs

| `--codec` | Format | Bit depth / chroma | HDR |
| --- | --- | --- | --- |
| `h264` | MP4 | 8-bit 4:2:0 | No |
| `hevc` | MP4 (`hvc1`, plays on Apple devices) | 10-bit 4:2:0 | HDR10 (with mastering metadata) and HLG |
| `av1` | MP4 | 10-bit 4:2:0 | Yes |
| `prores422-hq` | MOV | 10-bit 4:2:2 | Yes |
| `prores4444` / `prores4444-xq` | MOV | 12-bit 4:4:4 | Yes |
| `dnxhr-hq` | MOV | 8-bit 4:2:2 | No |
| `dnxhr-hqx` | MOV | 10-bit 4:2:2 | Yes |
| `dnxhr-444` | MOV | 10-bit 4:4:4 | Yes |

`--motion-blur N` blends N frames. `--quality` sets CRF for H.264, HEVC and AV1.

### Colour management and HDR

`--colour` selects the output space. Hat Timelapse does the colour conversion
itself, at 32-bit float. ffmpeg only converts RGB to YUV with the correct
matrix and tags the stream.

| `--colour` | Primaries | Transfer | Use |
| --- | --- | --- | --- |
| `rec709` (default) | BT.709 | sRGB values, tagged BT.709 | Web, broadcast HD/UHD SDR |
| `display-p3` | P3-D65 | sRGB | Apple wide-gamut SDR |
| `rec2020` | BT.2020 | sRGB curve, tagged BT.2020 | Wide-gamut SDR masters |
| `hdr-pq` | BT.2020 | SMPTE ST 2084 (PQ). Diffuse white at 203 nits (BT.2408). | HDR10 |
| `hdr-hlg` | BT.2020 | ARIB STD-B67 (HLG). Diffuse white at 75%. | Broadcast HDR, YouTube |
| `linear` | BT.709 | Linear, 32-bit float TIFF | Compositing in EffectCraft (TIFF export only) |

For true HDR, export 32-bit float TIFF from LightCraft (it supports float TIFF
export with HDR editing). Set `settings.lightcraft_command` to request it.
Hat Timelapse treats float input as scene-linear, so highlights above diffuse
white keep their headroom all the way into the PQ or HLG master. 8 and 16-bit
inputs are treated as sRGB.

## Long-term and construction timelapses

Nothing is deleted. Filters mark frames as excluded, can be combined, and can
be reset.

```bash
hat_timelapse filter weekdays sat sun       # site closed at weekends
hat_timelapse filter time 7 18              # working hours only (wraps midnight if from > to)
hat_timelapse filter outliers --max-ev 1    # lens covered, dark storms, flash
hat_timelapse filter every 1d               # one frame per day, the one nearest that day's median brightness
hat_timelapse deflicker --window 31         # stronger smoothing for daily frames
hat_timelapse video --size 4k --codec h264 --motion-blur 3
```

## Batch

Give `batch` project folders, or a parent folder whose sub-folders are
projects. Steps run in order for each project. Finished renders are reused, so
an interrupted batch can be restarted.

```bash
hat_timelapse batch ~/Timelapses/2026-11 \
  --steps init,holy-grail,interpolate,deflicker,render,video \
  --size 4k --codec hevc --also prores422-hq,dnxhr-hqx --keep-going
```

`-j N` limits worker threads for any command.

## Project layout

```
crates/
  hat_timelapse_core     sequence, EXIF, keyframe interpolation, holy grail, deflicker, long-term filters
  hat_timelapse_xmp      safe read/update of crs: XMP sidecars (everything else is preserved)
  hat_timelapse_render   LightCraft driver, framing and colour management, ffmpeg encoding
  hat_timelapse_cli      the `hat_timelapse` command
docs/                workflow notes, feature list and roadmap
```

Each project folder gets `hat_timelapse.json` (plain JSON, safe to commit or hand
edit) and a `.hat_timelapse/` working folder for renders, previews and framed
sequences.

## Status

Version 0.1, command line only. The full pipeline runs end to end and is
tested on Linux. CI builds and tests on Linux, macOS and Windows. The
LightCraft control names and render template still need confirming against a
LightCraft build; see `docs/ROADMAP.md`. A desktop app with a graph-based
keyframe and brightness editor is the next major piece.

## Licence

Dual-licensed under MIT or Apache-2.0, at your option.

Hat Timelapse is an independent project. It is not affiliated with ArtCraft,
LightCraft, LRTimelapse or Adobe. It contains no code, assets or
text from LRTimelapse or Adobe products.
