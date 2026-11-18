#!/usr/bin/env python3
"""End-to-end test: runs the installed plug-in inside PhotoCraft and checks the pixels it stored.

    tools/launch-photocraft.sh &        # once
    ./build.sh && tools/e2e.py [IMAGE…]

For each test image and each parameter set it exports the document before and after
`plugin.run`, applies the same filter natively (`cargo run --example apply`) to the "before"
pixels, and requires the two to agree to within rounding (half a level at the document's depth,
plus a hair), with alpha unchanged. Every run is undone afterwards.

The default images are generated into target/photocraft-test: an 8-bit RGBA image (colour swatches,
ramps, partly and fully transparent areas), a 16-bit RGB image of smooth gradients whose values are
not 8-bit values scaled up, and the same gradients as a 32-bit float TIFF. 32-bit results are
compared after clamping to 0–1 (the TIFF export clamps; the document itself keeps values outside 0–1,
as the Photoshop version did) and at 16-bit precision (ImageMagick reads them at its Q16 precision). Extra IMAGE paths are relative to the read roots
(target/photocraft-test, then this project), e.g. a copy of TestImage01.psd placed in the former.

Needs Python 3 with numpy and Pillow, and ImageMagick (for reading 16-bit PNGs).
"""
import json, os, subprocess, sys

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pc import ROOT, WORK, Client  # noqa: E402

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
    return ["test-rgba8.png", "test-rgb16.png", "test-rgb32.tif"]


def read_image(path):
    """(samples as float64 H×W×C, bit depth). ImageMagick, so 16- and 32-bit images keep their precision."""
    w, h, depth, alpha = subprocess.run(["identify", "-format", "%w %h %z %A", path], capture_output=True, text=True, check=True).stdout.split()
    w, h, depth = int(w), int(h), int(depth)
    fmt = "rgba" if alpha.lower() in ("true", "blend") else "rgb"
    if depth == 32:
        cmd = ["convert", path, "-define", "quantum:format=floating-point", "-depth", "32", "-endian", "LSB", f"{fmt}:-"]
        raw = subprocess.run(cmd, capture_output=True, check=True).stdout
        return np.frombuffer(raw, dtype="<f4").reshape(h, w, len(fmt)).astype(np.float64), depth
    raw = subprocess.run(["convert", path, "-depth", "16", "-endian", "LSB", f"{fmt}:-"], capture_output=True, check=True).stdout
    return np.frombuffer(raw, dtype="<u2").reshape(h, w, len(fmt)).astype(np.float64) / 65535.0, depth


def native(before, params):
    """The filter applied by the native build of the same code."""
    h, w, ch = before.shape
    src, dst = os.path.join(WORK, "native-in.f32"), os.path.join(WORK, "native-out.f32")
    before.astype("<f4").tofile(src)
    subprocess.run(["cargo", "run", "-q", "--release", "--example", "apply", "--", src, dst, str(w), str(h), str(ch), json.dumps(params)], cwd=ROOT, check=True)
    return np.fromfile(dst, dtype="<f4").reshape(h, w, ch).astype(np.float64)


def main(extra):
    c = Client()
    m = c.install()
    print(f"installed {m['name']} {m['version']}")
    failures = 0
    for image in make_images() + extra:
        c.open(image)
        stem = os.path.splitext(os.path.basename(image))[0]
        ext = ".tif" if image.endswith(".tif") else ".png"  # PNG has no 32-bit
        before, depth = read_image(c.save(f"{stem}-before{ext}"))
        levels = 255.0 if depth <= 8 else 65535.0
        visible = before[..., 3] > 0 if before.shape[2] == 4 else np.ones(before.shape[:2], bool)
        print(f"\n{image}: {before.shape[1]}×{before.shape[0]}, {depth}-bit, {before.shape[2]} channels")
        for i, params in enumerate(CASES):
            c.run_filter(params)
            after, _ = read_image(c.save(f"{stem}-after{i}{ext}"))
            c.execute("edit.undo")
            expected = np.clip(native(before, params), 0, 1)
            diff = np.abs(after[..., :3] - expected[..., :3])[visible] * levels
            alpha_kept = before.shape[2] < 4 or np.array_equal(after[..., 3], before[..., 3])
            ok = diff.max() <= 0.51 and alpha_kept
            failures += not ok
            print(f"  {'ok  ' if ok else 'FAIL'} {json.dumps(params)}: largest difference {diff.max():.3f} levels"
                  f" (mean {diff.mean():.3f}){'' if alpha_kept else ', ALPHA CHANGED'}")
    print("\nall passed" if not failures else f"\n{failures} FAILED")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
