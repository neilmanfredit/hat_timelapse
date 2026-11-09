# Feature coverage and roadmap

Hat Timelapse aims to cover the full professional timelapse workflow. This list
tracks what is built, what is next and what still needs checking.

## Built (v0.1)

| Area | Feature |
| --- | --- |
| Sequence | Folder scan in natural order; EXIF exposure and capture time (native, plus exiftool fallback) |
| Keyframes | Automatic or manual keyframes; keyframe labels in sidecars |
| Edits | Interpolation of 18 Camera Raw parameters, linear or monotone smooth |
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

## To confirm with a LightCraft build

1. **Control names.** `params.rs` maps Camera Raw names to LightCraft controls.
   `light.exposure`, `light.highlights`, `light.shadows` and `effects.clarity`
   appear in LightCraft's documentation. The others (`wb.temperature`,
   `color.vibrance` and so on) are best guesses and need checking against
   `lightcraft-cli`'s command registry.
2. **Render template.** Confirm the arguments for 16-bit TIFF and 32-bit float
   TIFF output, and whether `render` applies an existing sidecar. If it does,
   Hat Timelapse could rely on sidecars alone.
3. **Crop through the CLI.** Find out whether LightCraft exposes crop as a
   control. If so, map the `Crop*` parameters too.

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
