# Feature coverage and roadmap

Hat Timelapse aims to cover the full professional timelapse workflow. This list
tracks what is built, what is next and what still needs checking.

## Built (v0.1)

| Area | Feature |
| --- | --- |
| Sequence | Folder scan in natural order; EXIF exposure and capture time (native, plus exiftool fallback) |
| Keyframes | Automatic or manual keyframes; keyframe labels in sidecars |
| Edits | Interpolation of 18 Camera Raw parameters, linear or monotone smooth; all other settings copied from the first keyframe; LightCraft's own settings read and kept in step |
| Pan/zoom | Crop keyframes (Lightroom route) and Hat Timelapse motion keyframes (any route) |
| Holy grail | EXIF-based compensation of shutter, aperture and ISO changes, with step report |
| Deflicker | Brightness from originals or rendered previews, Gaussian smoothing, region of interest, multi-pass refinement |
| Long-term | Time-of-day, weekday, one-per-period (steadiest frame) and brightness-outlier filters |
| Develop | LightCraft CLI driver, resumable and parallel; Lightroom export route |
| Framing | Aspect crop, pan/zoom, Lanczos scaling, any size up to 8K and beyond |
| Colour | Rec.709, Display P3, Rec.2020, linear float; conversions in 32-bit float |
| HDR | PQ (HDR10 metadata) and HLG from scene-linear float TIFF |
| Video | H.264, HEVC, AV1, ProRes 422 HQ / 4444 / 4444 XQ, DNxHR HQ / HQX / 444, motion blur |
| Hand-off | 16-bit or float TIFF sequence with `sequence.json` for EffectCraft and FilmCraft |
| Batch | Many projects, chosen steps, several codecs per pass, keep-going, resumable |

## Confirmed with LightCraft 0.20.0 (macOS, Canon EOS R CR3)

- Control names match LightCraft's Camera Raw mapping. White balance is
  `wb.temp` and `wb.tint`.
- `render --opt bitDepth=16` gives 16-bit TIFF and `bitDepth=32` gives linear
  float TIFF, both at full sensor resolution (6720×4480).
- Develop settings apply both through `--set control=value` and through
  `--settings <file.xmp>`.

## Still open

- **Crop through the CLI.** Crop is a command in LightCraft (`crop.set`), not a
  slider, so the `Crop*` parameters are not passed to LightCraft. Pan and zoom
  use Hat Timelapse's own framing stage instead.

## Next

- **Desktop app** (egui): a frame strip, keyframe editor,
  and a brightness graph showing measured, target and corrected curves, with
  draggable curves for exposure and white balance.
- **Live LightCraft link** through its JSON control channel or MCP server.
  Keyframe edits would then flow without saving sidecars, and previews would
  render in-process for faster deflicker passes.
- **EffectCraft project export**: write an `.ecproj` with the sequence already
  imported at the right frame rate.
- **Masked deflicker**: brightness measured inside a mask, such as foreground
  only, for scenes with a changing sky.
- **Gradual white balance from the scene** for sunsets, driven by measured
  colour temperature as well as keyframes.
- **Gap and interval detection** from capture times, with frame duplication or
  blending to fill missed shots.
- **Exposure ramping analysis** for bulb-ramping controllers that write
  fractional shutter values.
- **Proxy workflow**: render low-resolution previews first, then the full set
  on approval.
- **GPU framing** through wgpu for 8K and 12K throughput.
- **Signed release builds** for macOS (universal), Windows and Linux (AppImage,
  .deb, AUR).
