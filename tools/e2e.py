#!/usr/bin/env python3
"""End-to-end test: runs the installed plug-in inside PhotoCraft and checks the pixels it stored.

    tools/launch-photocraft.sh &        # once
    ./build.sh && tools/e2e.py [IMAGE…]

For each test image and each parameter set it exports the document before and after
`plugin.run`, applies the same filter natively (`cargo run --example apply`) to the "before"
pixels, and requires the two to agree to within rounding (half a level at the document's depth,
plus a hair), with alpha unchanged. Every run is undone afterwards.

The default images are generated into target/photocraft-test:

* RGB: an 8-bit RGBA image (colour swatches, ramps, partly and fully transparent areas), a 16-bit
  RGB image of smooth gradients whose values are not 8-bit values scaled up, and the same gradients
  as a 32-bit float TIFF.
* Grayscale: 8-bit with and without transparent areas, 16-bit, and 32-bit float.
* Lab, 8- and 16-bit: the RGB gradients converted to Lab, and sweeps over the whole a/b square at
  several lightnesses, most of it outside the sRGB gamut.

32-bit results are compared after clamping to 0–1 (the TIFF export clamps; the document itself keeps
values outside 0–1, as the Photoshop version did) and at 16-bit precision (ImageMagick reads them at
its Q16 precision). Lab documents are exported as PSD and read from the layer's own pixels: the
merged image PhotoCraft writes next to them is clipped to sRGB. 16-bit Lab PSDs store a* and b* in
steps of 1/256 (Photoshop's scale), not the document's 1/257: the 16-bit Lab test image uses whole
a* and b* values, which both scales hold exactly, so "before" is exact, and the result loses up to
half a step when saved, so it compares to within 1 level (instead of half a level). Extra IMAGE paths are relative to the read roots
(target/photocraft-test, then this project), e.g. a copy of TestImage01.psd placed in the former.

Needs Python 3 with numpy and Pillow, and ImageMagick (for reading 16-bit PNGs).
"""
import json, os, subprocess, sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pc import ROOT, WORK, Client  # noqa: E402
from psd import read_psd_layer, write_psd  # noqa: E402

MODES = {1: "gray", 3: "rgb", 9: "lab"}

CASES = [
    {},
    {"saturation": 180, "warmth": 40},
    {"saturation": 60, "liveliness": -50, "gamma": -30, "contrast": -40, "warmth": -20, "hueShift": -90},
    {"saturation": 200, "liveliness": 100, "gamma": 100, "contrast": 100, "warmth": 100, "hueShift": 180},
    {"saturation": 0, "liveliness": -100, "gamma": -100, "contrast": -100, "warmth": -100, "hueShift": -180},
    {"saturation": 130, "liveliness": 70, "gamma": -10, "contrast": 55, "warmth": -35, "hueShift": 33},
    {"contrast": 1, "hueShift": 1},
]


def make_images():
    os.makedirs(WORK, exist_ok=True)
    # 8-bit RGBA: hue × brightness swatches, a grey ramp, and an alpha ramp down to fully transparent.
    w, h = 256, 192
    x = np.linspace(0, 1, w)[None, :]
    y = np.linspace(0, 1, h)[:, None]
    hue = x * 6
    rgb = np.stack([np.clip(np.abs(hue - 3) - 1, 0, 1), np.clip(2 - np.abs(hue - 2), 0, 1), np.clip(2 - np.abs(hue - 4), 0, 1)], -1)
    img = np.broadcast_to(rgb, (h, w, 3)) * (0.15 + 0.85 * (1 - y))[..., None]
    img = img * 0.8 + 0.2 * np.broadcast_to(x, (h, w))[..., None] * (y > 0.5)[..., None]
    img[:24] = np.broadcast_to(x, (24, w))[..., None]  # grey ramp
    alpha = np.ones((h, w))
    alpha[:, -48:] = np.linspace(1, 0, 48)[None, :]  # fades out to fully transparent
    rgba = np.dstack([img, alpha])
    Image.fromarray(np.round(rgba * 255).astype(np.uint8), "RGBA").save(os.path.join(WORK, "test-rgba8.png"))

    # 16-bit RGB: hue across, fading to white in the top half and to black in the bottom half.
    w, h = 512, 384
    x = np.linspace(0, 1, w)[None, :]
    hue = x * 6
    rgb = np.stack([np.clip(np.abs(hue - 3) - 1, 0, 1), np.clip(2 - np.abs(hue - 2), 0, 1), np.clip(2 - np.abs(hue - 4), 0, 1)], -1)
    rgb = np.broadcast_to(rgb, (h // 2, w, 3))
    t = np.linspace(0, 1, h // 2)[:, None, None]
    img = np.concatenate([rgb + (1 - rgb) * t * 0.9, rgb * (1 - t)], 0)
    raw = os.path.join(WORK, "test-rgb16.raw")
    np.round(np.clip(img, 0, 1) * 65535).astype("<u2").tofile(raw)
    rgb16 = os.path.join(WORK, "test-rgb16.png")
    subprocess.run(["convert", "-size", f"{w}x{h}", "-depth", "16", "-endian", "LSB", "rgb:" + raw, rgb16], check=True)
    os.remove(raw)
    subprocess.run(["convert", rgb16, "-depth", "32", "-define", "quantum:format=floating-point", os.path.join(WORK, "test-rgb32.tif")], check=True)
    gradients = img

    # Grayscale: ramps across, a finer ramp and a dark-to-bright ripple down; an alpha ramp to 0.
    w, h = 256, 128
    x = np.linspace(0, 1, w)[None, :]
    y = np.linspace(0, 1, h)[:, None]
    grey = np.clip(x * (0.6 + 0.4 * np.cos(y * 9)) + 0.05 * np.sin(x * 40) * y, 0, 1)
    Image.fromarray(np.round(grey * 255).astype(np.uint8), "L").save(os.path.join(WORK, "test-gray8.png"))
    alpha = np.ones((h, w))
    alpha[:, -48:] = np.linspace(1, 0, 48)[None, :]
    Image.fromarray(np.round(np.dstack([grey, alpha]) * 255).astype(np.uint8), "LA").save(os.path.join(WORK, "test-graya8.png"))
    w, h = 512, 96
    grey16 = np.clip(np.linspace(0, 1, w)[None, :] ** (1 + np.linspace(0, 1, h)[:, None]), 0, 1)
    raw = os.path.join(WORK, "test-gray16.raw")
    np.round(grey16 * 65535).astype("<u2").tofile(raw)
    gray16 = os.path.join(WORK, "test-gray16.png")
    subprocess.run(["convert", "-size", f"{w}x{h}", "-depth", "16", "-endian", "LSB", "gray:" + raw, gray16], check=True)
    os.remove(raw)
    subprocess.run(["convert", gray16, "-depth", "32", "-define", "quantum:format=floating-point", os.path.join(WORK, "test-gray32.tif")], check=True)

    # Lab: the RGB gradients in Lab, above a/b sweeps over −128..127 at five lightnesses.
    top = srgb_to_lab(gradients[::2, ::2])
    sweeps = []
    for light in (0.08, 0.3, 0.5, 0.7, 0.95):
        a, b = np.meshgrid(np.linspace(0, 1, top.shape[1]), np.linspace(1, 0, 40))
        sweeps.append(np.dstack([np.full_like(a, light), a, b]))
    lab = np.concatenate([top] + sweeps, 0)
    write_psd(os.path.join(WORK, "test-lab8.psd"), lab, 8, 9)
    lab[..., 1:] = (np.round(lab[..., 1:] * 255 - 128) + 128) / 255  # whole a*, b*: see the docstring
    write_psd(os.path.join(WORK, "test-lab16.psd"), lab, 16, 9)

    return ["test-rgba8.png", "test-rgb16.png", "test-rgb32.tif", "test-gray8.png", "test-graya8.png", "test-gray16.png", "test-gray32.tif",
            "test-lab8.psd", "test-lab16.psd"]


def srgb_to_lab(rgb):
    """Encoded sRGB → Lab in the plug-in's units, with PhotoCraft's formulas (D50, Bradford)."""
    m = np.array([[0.436074, 0.385064, 0.143080], [0.222504, 0.716878, 0.060618], [0.013932, 0.097104, 0.714173]])
    lin = np.where(rgb <= 0.04045, rgb / 12.92, ((rgb + 0.055) / 1.055) ** 2.4)
    xyz = lin @ m.T / np.array([0.96422, 1.0, 0.82521])
    f = np.where(xyz > (6 / 29) ** 3, np.cbrt(xyz), xyz / (3 * (6 / 29) ** 2) + 4 / 29)
    lab = np.dstack([116 * f[..., 1] - 16, 500 * (f[..., 0] - f[..., 1]), 200 * (f[..., 1] - f[..., 2])])
    return np.dstack([lab[..., 0] / 100, (lab[..., 1] + 128) / 255, (lab[..., 2] + 128) / 255])


def read_image(path):
    """(samples as float64 H×W×C, bit depth, mode name). PSDs (Lab) from the layer's pixels,
    grey PNGs with Pillow, the rest with ImageMagick, so 16- and 32-bit images keep their precision."""
    if path.endswith(".psd"):
        a, depth, mode = read_psd_layer(path)
        return a, depth, MODES[mode]
    if path.endswith(".png"):
        im = Image.open(path)
        if im.mode in ("L", "LA", "I;16", "I;16B", "I"):
            a = np.array(im).astype(np.float64)
            depth = 8 if im.mode in ("L", "LA") else 16
            a = (a / (255.0 if depth == 8 else 65535.0)).reshape(a.shape[0], a.shape[1], -1)
            return a, depth, "gray"
    w, h, depth, alpha, space = subprocess.run(["identify", "-format", "%w %h %z %A %[colorspace]", path], capture_output=True, text=True, check=True).stdout.split()
    w, h, depth = int(w), int(h), int(depth)
    gray = space.lower() in ("gray", "lineargray")
    fmt = ("gray" if gray else "rgb") + ("a" if alpha.lower() in ("true", "blend") and not gray else "")
    channels = 1 if gray else len(fmt)
    if depth == 32:
        cmd = ["convert", path, "-define", "quantum:format=floating-point", "-depth", "32", "-endian", "LSB", f"{fmt}:-"]
        raw = subprocess.run(cmd, capture_output=True, check=True).stdout
        return np.frombuffer(raw, dtype="<f4").reshape(h, w, channels).astype(np.float64), depth, "gray" if gray else "rgb"
    raw = subprocess.run(["convert", path, "-depth", "16", "-endian", "LSB", f"{fmt}:-"], capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype="<u2").reshape(h, w, channels).astype(np.float64) / 65535.0, depth, "gray" if gray else "rgb"


def native(before, params, mode):
    """The filter applied by the native build of the same code."""
    h, w, ch = before.shape
    src, dst = os.path.join(WORK, "native-in.f32"), os.path.join(WORK, "native-out.f32")
    before.astype("<f4").tofile(src)
    subprocess.run(["cargo", "run", "-q", "--release", "--example", "apply", "--", src, dst, str(w), str(h), str(ch), json.dumps(params), mode], cwd=ROOT, check=True)
    return np.fromfile(dst, dtype="<f4").reshape(h, w, ch).astype(np.float64)


def main(extra):
    c = Client()
    m = c.install()
    print(f"installed {m['name']} {m['version']}")
    failures = 0
    for image in make_images() + extra:
        c.open(image)
        stem = os.path.splitext(os.path.basename(image))[0]
        ext = os.path.splitext(image)[1] if image.endswith((".tif", ".psd")) else ".png"  # PNG has no 32-bit
        before, depth, mode = read_image(c.save(f"{stem}-before{ext}"))
        colours = 1 if mode == "gray" else 3
        levels = 255.0 if depth <= 8 else 65535.0
        tolerance = 1.01 if (mode, depth) == ("lab", 16) else 0.51
        has_alpha = before.shape[2] > colours
        visible = before[..., -1] > 0 if has_alpha else np.ones(before.shape[:2], bool)
        print(f"\n{image}: {before.shape[1]}×{before.shape[0]}, {mode}, {depth}-bit, {before.shape[2]} channels")
        for i, params in enumerate(CASES):
            c.run_filter(params)
            after, _, _ = read_image(c.save(f"{stem}-after{i}{ext}"))
            c.execute("edit.undo")
            expected = np.clip(native(before, params, mode), 0, 1)
            diff = np.abs(after[..., :colours] - expected[..., :colours])[visible] * levels
            alpha_kept = not has_alpha or np.array_equal(after[..., -1], before[..., -1])
            ok = diff.max() <= tolerance and alpha_kept
            failures += not ok
            print(f"  {'ok  ' if ok else 'FAIL'} {json.dumps(params)}: largest difference {diff.max():.3f} levels"
                  f" (mean {diff.mean():.3f}){'' if alpha_kept else ', ALPHA CHANGED'}")
    print("\nall passed" if not failures else f"\n{failures} FAILED")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
