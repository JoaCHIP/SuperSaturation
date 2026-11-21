#!/usr/bin/env python3
"""Times the plug-in inside PhotoCraft on generated 6000 × 4000 (24 MP) 8-bit images.

    tools/launch-photocraft.sh &        # once
    ./build.sh && tools/bench.py [rgb] [gray] [lab]     # default: all three

Each timing is one `plugin.run` through the control channel (so it includes PhotoCraft's own
work: converting the layer to bands, writing back, the undo step), then undone.
"""
import os, sys, time

import numpy as np
from PIL import Image

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pc import WORK, Client  # noqa: E402
from psd import write_psd  # noqa: E402

CASES = [
    ("neutral", {}),
    ("saturation", {"saturation": 150}),
    ("gamma + contrast", {"gamma": 20, "contrast": 30}),
    ("every slider", {"saturation": 150, "liveliness": 40, "gamma": 20, "contrast": 30, "warmth": 20, "hueShift": 15}),
]


def make_image(mode):
    """The 24 MP test image for `mode`, generated once: smooth colour gradients with noise."""
    name = {"rgb": "bench-24mp.png", "gray": "bench-24mp-gray.png", "lab": "bench-24mp-lab.psd"}[mode]
    path = os.path.join(WORK, name)
    if not os.path.exists(path):
        os.makedirs(WORK, exist_ok=True)
        w, h = 6000, 4000
        x = np.linspace(0, 1, w, dtype=np.float32)[None, :]
        y = np.linspace(0, 1, h, dtype=np.float32)[:, None]
        img = np.stack([x * np.ones_like(y), y * np.ones_like(x), (1 - x) * (1 - y) * 0.8], -1)
        img += np.random.default_rng(1).normal(0, 0.05, img.shape).astype(np.float32)
        img = np.clip(img, 0, 1)
        if mode == "rgb":
            Image.fromarray((img * 255).astype(np.uint8)).save(path, compress_level=1)
        elif mode == "gray":
            Image.fromarray((img.mean(-1) * 255).astype(np.uint8), "L").save(path, compress_level=1)
        else:  # L across, a and b over most of their range
            write_psd(path, np.stack([img[..., 0], 0.15 + 0.7 * img[..., 1], 0.15 + 0.7 * img[..., 2]], -1), 8, 9)
    return name


def main(modes):
    c = Client()
    c.install()
    for mode in modes or ["rgb", "gray", "lab"]:
        c.open(make_image(mode))
        print(mode)
        for name, params in CASES:
            times = []
            for _ in range(2):
                t = time.time()
                c.run_filter(params)
                times.append(time.time() - t)
                c.execute("edit.undo")
            print(f"  {name:18} {min(times):6.2f} s")


if __name__ == "__main__":
    main(sys.argv[1:])
