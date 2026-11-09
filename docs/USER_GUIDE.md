# Hat Timelapse user guide

This guide takes a sequence from the memory card to a finished video. It
assumes Hat Timelapse, LightCraft (or Lightroom), ffmpeg and exiftool are
installed; see [INSTALL.md](INSTALL.md).

Commands are typed in Terminal (macOS), a shell (Linux) or PowerShell
(Windows). Every command accepts `--help`, for example
`hat_timelapse video --help`.

## Contents

1. [How it works](#1-how-it-works)
2. [Set up the folder](#2-set-up-the-folder)
3. [Create the project](#3-create-the-project)
4. [Choose keyframes](#4-choose-keyframes)
5. [Day-to-night: holy grail](#5-day-to-night-holy-grail)
6. [Edit the keyframes](#6-edit-the-keyframes)
7. [Spread the edits across every frame](#7-spread-the-edits-across-every-frame)
8. [Remove flicker](#8-remove-flicker)
9. [Develop every frame](#9-develop-every-frame)
10. [Pan and zoom](#10-pan-and-zoom)
11. [Make the video](#11-make-the-video)
12. [Export for EffectCraft or FilmCraft](#12-export-for-effectcraft-or-filmcraft)
13. [Long-term and construction timelapses](#13-long-term-and-construction-timelapses)
14. [Many sequences at once: batch](#14-many-sequences-at-once-batch)
15. [Going back and changing things](#15-going-back-and-changing-things)
16. [Recipes](#16-recipes)

---

## 1. How it works

You edit a handful of **keyframes** in your photo editor. Hat Timelapse:

- reads those edits and blends them smoothly across every frame in between;
- copies everything else you set on the first keyframe (profile, tone curve,
  colour, lens corrections, masks) to every frame;
- compensates for exposure changes you made on the camera during the shoot;
- measures every frame's brightness and removes flicker;
- develops every frame through LightCraft, or takes your Lightroom export;
- crops, pans, zooms and scales to the video size, then encodes the video.

Your edits live in the `.xmp` sidecar file next to each image, the same files
LightCraft and Lightroom use. Hat Timelapse never changes your raw files.

## 2. Set up the folder

Put one sequence in one folder, with nothing else in it:

```
~/Timelapses/2026-11-08-sunset/
    IMG_0001.CR3
    IMG_0002.CR3
    …
```

Frames are ordered by file name, so `IMG_9` comes before `IMG_10`. If your
camera's file numbers rolled over mid-sequence (`IMG_9999` → `IMG_0001`),
rename the files first so they sort in shooting order.

Move Terminal into the folder. Type `cd ` (with a space), drag the folder from
Finder into the window, and press Return. Every command below runs on the
folder you are in. To work on another folder without moving, add
`-p /path/to/folder`.

## 3. Create the project

```bash
hat_timelapse init --keyframes 3
```

This reads every file's capture time, shutter, aperture and ISO, and saves the
project as `hat_timelapse.json` in the folder. It also marks three evenly
spaced keyframes.

Check it:

```bash
hat_timelapse status
```

`status` shows the frame count, keyframes, the range of camera exposure, and
the capture start and end times. If it reports no exposure metadata for CR3 or
RAF files, install exiftool and run `hat_timelapse init --force`.

## 4. Choose keyframes

Keyframes are the frames you edit by hand. Use more of them where the light
changes quickly, such as around sunset.

```bash
hat_timelapse keyframes auto 5          # five evenly spaced, first and last included
hat_timelapse keyframes set 0 180 420 600 899   # exactly these frames (numbered from 0)
hat_timelapse keyframes list            # show them
hat_timelapse keyframes label           # label them "Keyframe" in their sidecars
```

`label` makes the keyframes easy to find in your editor. Run it **before**
importing into LightCraft or Lightroom, because both read labels on import.

**The first keyframe is the master.** Any setting you don't vary between
keyframes, such as the profile, tone curve, HSL, sharpening, lens corrections
and masks, is taken from the first keyframe and applied to every frame. Set the
overall look there.

## 5. Day-to-night: holy grail

Skip this step if you did not change shutter, aperture or ISO during the shoot.

When you lengthen the shutter or raise the ISO as the light fades, each change
makes the image jump brighter. Run:

```bash
hat_timelapse holy-grail
```

This lists every change it found, for example `frame 412  -1.00 EV`. It gives
each frame an exposure offset that cancels the jump, so brightness follows the
scene, not the camera dial. The offsets are added on top of whatever exposure
you set on your keyframes.

After this the sequence will look like it does to the eye: darker towards
night. Use your keyframe exposures to decide how bright the night should end
up.

## 6. Edit the keyframes

### In LightCraft

1. **Turn on automatic sidecars** (once): **Settings → Import → Automatically
   write changes into XMP sidecars**.
2. **Import the folder in place**: **File → Import**, choose the folder, and
   pick **Add in place** (not Copy or Move), so LightCraft works on the files
   where they are.
3. **Find the keyframes**: they carry the label "Keyframe" if you ran
   `keyframes label`, or use the frame numbers from `hat_timelapse keyframes
   list`. Frame 0 is the first file.
4. **Edit the first keyframe** to set the overall look.
5. **Edit the other keyframes**, changing only what should vary over time:
   exposure, white balance, contrast, highlights, shadows and so on.
6. **Save**: with automatic sidecars on, this happens as you go. Otherwise use
   **Photo → Save Metadata to File** (⌘S) on each keyframe.

### In Lightroom Classic

1. Import the folder with **Add** (not Copy).
2. Find the keyframes. Filter by label: Hat Timelapse uses a custom label, so
   choose *Custom Label* in the Library filter.
3. Edit the first keyframe for the overall look, then the others for what
   changes over time.
4. Select the keyframes and choose **Metadata → Save Metadata to File** (⌘S).

### What can change between keyframes

Exposure, Contrast, Highlights, Shadows, Whites, Blacks, Temperature, Tint,
Texture, Clarity, Dehaze, Vibrance, Saturation, and, in Lightroom only, the
crop. Anything else is taken from the first keyframe. If other keyframes set it
differently, Hat Timelapse warns you which settings it ignored.

## 7. Spread the edits across every frame

```bash
hat_timelapse interpolate
```

This reads the keyframes, blends every changing value smoothly across the
frames in between, copies the first keyframe's other settings to every frame,
and writes all the sidecars. It reports what it animated, for example:

```
interpolated 4 parameters across 900 frames from 5 keyframes: ["Exposure2012", "Highlights2012", "Shadows2012", "Temperature"]
copied 37 other settings from keyframe 0 to every frame
```

`--method smooth` (the default) eases between keyframes without overshooting
them. `--method linear` makes straight-line changes.

**To see the result in your editor**, select all frames and read the sidecars
back in:
- LightCraft: **Photo → Read Metadata from File**.
- Lightroom: **Metadata → Read Metadata from Files**.

## 8. Remove flicker

```bash
hat_timelapse deflicker
```

This measures the brightness of every frame and evens out the frame-to-frame
flicker, while keeping slow changes such as a sunset or your keyframed ramps.
It prints the largest correction it made.

Useful options:

| Option | Use |
| --- | --- |
| `--window 15` | How many frames to smooth over. Raise it (25–45) for stubborn flicker; lower it (7–11) if real changes, such as a passing cloud, are being flattened. |
| `--roi 0,0.5,1,0.5` | Measure only part of the frame (x, y, width, height, from 0 to 1). This example uses the bottom half, ignoring a changing sky. |
| `--reset` | Throw away earlier corrections and start again. |
| `--source previews` | Measure developed previews rather than the originals; see below. |

**For the most accurate result**, measure frames developed with all your
edits. Repeat these two steps until the reported correction is below about
0.02 EV, usually two or three passes:

```bash
hat_timelapse render --previews
hat_timelapse deflicker --source previews
```

After deflickering, read the sidecars back into your editor if you want to see
the corrected frames there (step 7).

## 9. Develop every frame

### With LightCraft

```bash
hat_timelapse render
```

This develops every frame at full resolution as a 16-bit TIFF into
`.hat_timelapse/rendered/`. Each frame gets its complete settings, so the look
from your first keyframe, every animated value, and the holy-grail and
deflicker offsets are all applied. It runs several frames at a time.

- It **resumes**: frames already developed are skipped. Use `--overwrite`
  after changing edits.
- For **HDR** or compositing, render 32-bit float: `hat_timelapse render
  --bit-depth 32 --overwrite`.

### With Lightroom

Select all frames, then **Metadata → Read Metadata from Files**, then
**File → Export**:

- Format: **TIFF, 16-bit, sRGB** (or JPEG, quality 100, sRGB). Use sRGB:
  Hat Timelapse treats 8 and 16-bit files as sRGB, so other colour spaces
  would shift the colours.
- File naming: keep the **original file name**
- Export to a folder, for example `~/Exports/sunset`

Then give that folder to the next steps with `--from ~/Exports/sunset`.

## 10. Pan and zoom

Pan and zoom are applied when the video is made, so they cost nothing to
change. Add motion keyframes by frame number:

```bash
hat_timelapse motion add 0   --zoom 1.0                     # start: full frame
hat_timelapse motion add 899 --zoom 1.5 --cx 0.6 --cy 0.4   # end: 1.5× closer, right of and above centre
hat_timelapse motion list
hat_timelapse motion clear
```

- `--zoom` is 1.0 or more. 1.0 is the full frame at the video's aspect ratio.
- `--cx` and `--cy` are the centre of the view, from 0 (left or top) to 1
  (right or bottom).
- Motion between keys follows the same smooth curve as edits.

Zooming crops into the image, so zoom no further than your source resolution
allows. A 6720-pixel-wide frame can zoom to about 1.7× in 4K (3840 wide)
before it is enlarged.

## 11. Make the video

```bash
hat_timelapse video --size 4k --codec h264
```

The finished file appears in the project folder, for example
`2026-11-08-sunset_3840x2160_rec709_h264.mp4`.

### Size

`--size` takes `hd` (1280×720), `fhd` (1920×1080), `2.7k`, `4k` (3840×2160),
`dci4k` (4096×2160), `5k`, `6k`, `8k` (7680×4320), `dci8k`, `12k`, or any
`WIDTHxHEIGHT`.

### Format

| `--codec` | Use it for |
| --- | --- |
| `h264` | Sharing, web, social media. Plays everywhere. |
| `hevc` | Smaller files at the same quality; HDR. Plays on Apple devices and modern TVs. |
| `av1` | Smallest files; YouTube; HDR. |
| `prores422-hq` | Editing in Final Cut, Premiere, Resolve or FilmCraft. |
| `prores4444` / `prores4444-xq` | Highest-quality masters. |
| `dnxhr-hq` / `dnxhr-hqx` / `dnxhr-444` | Editing in Avid and Resolve; Windows-friendly. HQX and 444 carry HDR. |

### Other options

| Option | Effect |
| --- | --- |
| `--fps 25` | Frame rate (default 25; also 24, 30, 50, 60…). |
| `--quality 16` | For H.264, HEVC and AV1. Lower is better quality and bigger files (12–20 is sensible). |
| `--motion-blur 3` | Blend 3 frames for smoother motion: clouds, traffic, water. |
| `--colour` | Colour space; see below. |
| `--from DIR` | Use a Lightroom export instead of LightCraft's renders. |
| `--out FILE` | Choose the file name. |

### Colour and HDR

| `--colour` | When |
| --- | --- |
| `rec709` (default) | Normal video for screens, web and broadcast. |
| `display-p3` | Wider colour for Apple devices. |
| `rec2020` | Wide-colour SDR masters. |
| `hdr-pq` | HDR10. Use with `hevc`, `av1`, ProRes or `dnxhr-hqx`/`444`. |
| `hdr-hlg` | HDR for broadcast and YouTube. |

For HDR, develop in 32-bit float first, so highlights keep their range:

```bash
hat_timelapse render --bit-depth 32 --overwrite
hat_timelapse video --size 4k --codec hevc --colour hdr-pq
```

You can make several versions from one render:

```bash
hat_timelapse video --size 4k --codec h264
hat_timelapse video --size 4k --codec prores422-hq
hat_timelapse video --size fhd --codec h264 --motion-blur 3
```

## 12. Export for EffectCraft or FilmCraft

```bash
hat_timelapse export --size 4k
```

This writes a numbered 16-bit TIFF sequence (`frame_000001.tif` …) into
`export/3840x2160-rec709/`, with `sequence.json` recording the frame rate,
size and colour space. Import the first frame as an image sequence in
EffectCraft or FilmCraft.

For compositing with full highlight range, export linear 32-bit float:

```bash
hat_timelapse render --bit-depth 32 --overwrite
hat_timelapse export --size 4k --colour linear
```

## 13. Long-term and construction timelapses

A camera on site for months captures nights, weekends, rain and people walking
past. Filters mark frames as left out; nothing is deleted, and
`hat_timelapse filter reset` brings everything back.

```bash
hat_timelapse filter weekdays sat sun     # drop weekends
hat_timelapse filter time 7 18            # keep 07:00–18:00 only
hat_timelapse filter outliers --max-ev 1  # drop frames far brighter or darker than their neighbours
hat_timelapse filter every 1d             # keep one frame per day: the one nearest that day's typical brightness
hat_timelapse status                      # how many frames remain
```

Periods for `every` can be `30m`, `6h`, `1d` or `1w`. Then deflicker with a
wider window, and blend frames for a smoother result:

```bash
hat_timelapse deflicker --window 31
hat_timelapse render
hat_timelapse video --size 4k --codec h264 --motion-blur 3 --fps 30
```

## 14. Many sequences at once: batch

Put each sequence in its own folder under one parent:

```
~/Timelapses/2026-11/
    site-north/
    site-south/
    harbour-sunset/
```

Then:

```bash
hat_timelapse batch ~/Timelapses/2026-11 --size 4k --codec h264 --also prores422-hq --keep-going
```

By default, batch runs `init, holy-grail, interpolate, deflicker, render,
video` on each folder. Choose your own steps with `--steps`, for example
`--steps deflicker,render,video`. `--also` adds more formats in the same pass.
`--keep-going` carries on past a folder that fails and lists failures at the
end. Finished frames are reused, so an interrupted batch can simply be run
again. Batch renders 32-bit float automatically when `--colour` is HDR.

Edit keyframes before batching (steps 4 and 6). Batch is for when the edits
are done, or for long-term sequences that need no hand edits.

## 15. Going back and changing things

Every step can be re-run in any order. Your own keyframe values are never lost:
Hat Timelapse keeps its holy-grail and deflicker offsets separate and removes
them before re-reading your keyframes.

To change a keyframe after running Hat Timelapse:

1. In your editor, select the keyframe and **read metadata from file** first
   (LightCraft: Photo → Read Metadata from File; Lightroom: Metadata → Read
   Metadata from Files). Your editor then starts from what Hat Timelapse
   wrote.
2. Make the change and save it (⌘S).
3. Run `hat_timelapse interpolate`, then `deflicker` if you use it, then
   `render --overwrite` and `video`.

Skipping step 1 can leave your editor holding older values that overwrite the
exposure corrections.

**Files Hat Timelapse creates in the project folder**

| File or folder | What it is |
| --- | --- |
| `hat_timelapse.json` | The project: frames, keyframes, corrections, motion. Plain text. |
| `*.xmp` | Edit settings for each frame, shared with LightCraft and Lightroom. |
| `.hat_timelapse/rendered/` | Developed frames. Safe to delete; `render` recreates them. |
| `.hat_timelapse/previews/` | Small previews for deflicker. Safe to delete. |
| `.hat_timelapse/framed/` | Frames sized for video. Safe to delete. |
| `export/` | TIFF sequences from `export`. |

## 16. Recipes

**Simple daytime sequence**

```bash
hat_timelapse init --keyframes 2
# edit both keyframes in LightCraft and save
hat_timelapse interpolate
hat_timelapse deflicker
hat_timelapse render
hat_timelapse video --size 4k --codec h264
```

**Sunset into night, with shutter and ISO changes during the shoot**

```bash
hat_timelapse init --keyframes 5
hat_timelapse keyframes label
hat_timelapse holy-grail
# edit the keyframes; brighten the late ones as it gets dark
hat_timelapse interpolate
hat_timelapse render --previews
hat_timelapse deflicker --source previews
hat_timelapse render --previews
hat_timelapse deflicker --source previews
hat_timelapse render
hat_timelapse video --size 4k --codec h264 --motion-blur 2
```

**HDR master and a web version**

```bash
hat_timelapse render --bit-depth 32
hat_timelapse video --size 4k --codec hevc --colour hdr-pq
hat_timelapse video --size fhd --codec h264
```

**Construction site, one frame per working day**

```bash
hat_timelapse init --keyframes 1
hat_timelapse filter weekdays sat sun
hat_timelapse filter time 10 14
hat_timelapse filter outliers --max-ev 1
hat_timelapse filter every 1d
hat_timelapse deflicker --window 31
hat_timelapse render
hat_timelapse video --size 4k --codec h264 --motion-blur 3 --fps 30
```
