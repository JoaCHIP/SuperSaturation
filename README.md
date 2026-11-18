# SuperSaturation for PhotoCraft

By **JoaCHIP**. A port of the [SuperSaturation](https://github.com/JoaCHIP/SuperSaturation)
Photoshop filter to a PhotoCraft WebAssembly plug-in.

SuperSaturation adjusts color in its own HSB-like color space, which keeps
colors looking more natural the built-in adjustments. Some controls overlap
with built-in ones on paper; they look different in practice.

| Slider | Range | Neutral | What it does |
|---|---|---|---|
| Saturation | 0 to 200 | 100 | More or less color. |
| Liveliness | -100 to 100 | 0 | More color in muted areas, without pushing areas that are already strongly colored. |
| Gamma | -100 to 100 | 0 | A gamma curve on the brightness channel only: brighter or darker while keeping colors accurate. |
| Contrast | -100 to 100 | 0 | A sine-shaped S-curve on the brightness channel only. |
| Warmth | -100 to 100 | 0 | Leans the image towards orange (warm) or towards blue (cold) colors. |
| Hue Shift | -180 to 180° | 0 | Rotates every hue around the color wheel. |

## Install

```sh
./build.sh                  # → supersaturation.wasm
```

In PhotoCraft, choose **Filter › Plug-ins › Install Plug-in…** and pick `supersaturation.wasm`. It
then appears as **Filter › Plug-ins › SuperSaturation…**. A plug-in installed that way lasts until
PhotoCraft quits. To have it load at every start, copy the `.wasm` into a folder and set that folder
in **Edit › Preferences › Plug-ins** (*Use Additional Plug-ins Folder*).

Building needs Rust with the WebAssembly target: `rustup target add wasm32-unknown-unknown`.

## Color modes

RGB, at 8 and 16 bits per channel, as in the Photoshop version. 32-bit RGB works too, through the
same code: PhotoCraft hands plug-ins 0-1 floats at every depth. As in the Photoshop version, 32-bit
results are not clamped, so strong settings can leave values above 1 or below 0 there (8- and
16-bit documents clamp them when storing).

Other modes (Grayscale, CMYK, Lab, etc) and alpha channels / Quick Mask are refused for now:
PhotoCraft reports "pc_filter returned error code 1" and leaves the image untouched. Adding one is
a branch in `filter()` in `src/lib.rs` that converts each pixel to RGB, calls `process_pixel()` and
converts back. `ColorMode`'s documentation there says how for each mode.

| Code | Meaning |
|---|---|
| 1 | The color mode is not supported (yet). |
| 2 | Too few channels for the color mode. |
| 3 | The pixel buffer is smaller than its stated size. |
| 4 | The parameter JSON could not be read. |

---

# Technical Stuff

PhotoCraft runs plug-ins in an interpreter, so the transform is tuned for instruction count without
changing its results (see the notes at the top of `src/algorithm.rs` and `src/mathf.rs`): sliders
at their neutral position cost nothing, the hue is rotated with a vector instead of `atan2` and
three `cos`, and `powf` and `sqrtf` are interpreter-friendly versions accurate to one `f32` ulp.

A 6000 × 4000 (24 MP) 8-bit image on a 16-thread machine, including PhotoCraft's own work:

| Sliders moved | Time |
|---|---|
| none, or only Saturation / Hue Shift | 1.8 s |
| Gamma and Contrast | 4.2 s |
| all six | 6.1 s |

On large documents the dialog's live preview runs on a reduced copy (Preferences › Performance ›
Low Resolution Previews), so it is quicker than applying the filter.

## Tests

```sh
cargo test                  # unit tests + comparison with the original C++
```

`reference/generate.sh` regenerates `tests/data/reference.bin` from the original C++ (needs g++).

End-to-end tests drive a real PhotoCraft over its control channel. They need Python 3 with numpy and
Pillow, and ImageMagick:

```sh
tools/launch-photocraft.sh &    # a separate PhotoCraft with the control server on
./build.sh
tools/e2e.py                    # runs the plug-in in PhotoCraft and checks every pixel
tools/bench.py                  # timings on a 24 MP image
tools/pc.py dialog              # opens the dialog; tools/pc.py screenshot captures the window
```

`tools/e2e.py` installs the fresh build, generates test images (8-bit RGBA with transparent areas,
16-bit RGB, 32-bit float RGB), runs the filter with several slider combinations, exports each
result, and requires it to match the native build of the same code to within rounding, with alpha
unchanged.
Pass extra images (relative to `target/photocraft-test` or this folder) to test those too.

## Layout

| Path | What |
|---|---|
| `src/manifest.json` | Id, name, author and sliders: PhotoCraft builds the dialog from it. |
| `src/lib.rs` | The plug-in ABI exports, pixel layouts and color-mode dispatch. |
| `src/algorithm.rs` | The SuperSaturation transform. |
| `src/mathf.rs` | Float maths tuned for the interpreter. |
| `src/params.rs` | Reads the slider values from the host's JSON. |
| `reference/` | The original C++ transform, for the comparison test. |
| `tests/` | The comparison with the original. |
| `examples/apply.rs` | Applies the filter natively to raw pixels (used by `tools/e2e.py`). |
| `tools/` | Scripts that drive PhotoCraft for end-to-end tests and timings. |

The plug-in contract is PhotoCraft's `docs/plugins.md` (ABI v1).

## Licence

MIT, as the original. See `LICENSE`.
