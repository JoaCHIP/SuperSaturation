"""Reads and writes the composite (merged) image of a PSD, for testing Lab documents exactly.

    samples, depth, mode = read_psd(path)
    write_psd(path, samples, depth, mode)

`samples` is float64 H×W×C in the plug-in ABI's units (0–1; Lab as L*/100, (a* + 128)/255,
(b* + 128)/255), `mode` the PSD colour mode number (1 Grayscale, 3 RGB, 9 Lab). Reading handles raw
and PackBits (RLE) composites at 8, 16 and 32 bits; writing makes a layerless PSD with a raw
composite, which PhotoCraft opens as a one-layer document.

16-bit Lab PSDs keep a* and b* on Photoshop's scale, `32768 + 256 · a*`, not over 0–65535; both
functions convert.
"""
import struct

import numpy as np


def _packbits(data, pos, size):
    out = bytearray()
    while len(out) < size:
        n = data[pos]
        pos += 1
        if n < 128:
            out += data[pos:pos + n + 1]
            pos += n + 1
        elif n > 128:
            out += bytes([data[pos]]) * (257 - n)
            pos += 1
    return bytes(out[:size]), pos


def read_psd(path):
    data = open(path, "rb").read()
    sig, version, _, channels, h, w, depth, mode = struct.unpack(">4sH6sHIIHH", data[:26])
    if sig != b"8BPS":
        raise ValueError(f"{path}: not a PSD")
    pos = 26
    pos += 4 + struct.unpack(">I", data[pos:pos + 4])[0]  # colour mode data
    pos += 4 + struct.unpack(">I", data[pos:pos + 4])[0]  # image resources
    size_len = 8 if version == 2 else 4
    pos += size_len + int.from_bytes(data[pos:pos + size_len], "big")  # layer and mask information
    compression = struct.unpack(">H", data[pos:pos + 2])[0]
    pos += 2
    row_bytes = w * depth // 8
    if compression == 0:
        planes = data[pos:pos + channels * h * row_bytes]
    elif compression == 1:
        count_size = 4 if version == 2 else 2
        pos += channels * h * count_size  # the per-row byte counts: rows are decoded in sequence
        rows = []
        for _ in range(channels * h):
            row, pos = _packbits(data, pos, row_bytes)
            rows.append(row)
        planes = b"".join(rows)
    else:
        raise ValueError(f"{path}: composite compression {compression} not supported")
    dtype = {8: ">u1", 16: ">u2", 32: ">f4"}[depth]
    a = np.frombuffer(planes, dtype=dtype).reshape(channels, h, w).transpose(1, 2, 0).astype(np.float64)
    if depth == 8:
        a /= 255.0
    elif depth == 16:
        a /= 65535.0
        if mode == LAB:
            a[..., 1:3] = ((a[..., 1:3] * 65535.0 - 32768.0) / 256.0 + 128.0) / 255.0
    return a, depth, mode


LAB = 9


def write_psd(path, samples, depth, mode):
    h, w, channels = samples.shape
    a = np.array(samples, dtype=np.float64)
    if depth == 32:
        planes = a.astype(">f4")
    else:
        if depth == 16 and mode == LAB:
            a[..., 1:3] = (32768.0 + 256.0 * (a[..., 1:3] * 255.0 - 128.0)) / 65535.0
        top = 255.0 if depth == 8 else 65535.0
        planes = np.round(np.clip(a, 0, 1) * top).astype(">u1" if depth == 8 else ">u2")
    out = struct.pack(">4sH6sHIIHH", b"8BPS", 1, bytes(6), channels, h, w, depth, mode)
    out += struct.pack(">III", 0, 0, 0)  # colour mode data, image resources, layer and mask info
    out += struct.pack(">H", 0) + planes.transpose(2, 0, 1).tobytes()
    open(path, "wb").write(out)


def read_psd_layer(path, index=0):
    """The pixels of layer `index` (bottom first) over the whole canvas, in the same units as
    read_psd, with alpha last when the layer has a transparency channel. Pixels outside the layer's
    bounds are 0 (transparent)."""
    data = open(path, "rb").read()
    _, version, _, _, h, w, depth, mode = struct.unpack(">4sH6sHIIHH", data[:26])
    pos = 26
    pos += 4 + struct.unpack(">I", data[pos:pos + 4])[0]
    pos += 4 + struct.unpack(">I", data[pos:pos + 4])[0]
    big = version == 2
    ln = 8 if big else 4
    pos += ln  # layer and mask section length
    info_len = int.from_bytes(data[pos:pos + ln], "big")
    pos += ln
    count = abs(struct.unpack(">h", data[pos:pos + 2])[0]) if info_len else 0
    pos += 2
    if count == 0:
        # 16- and 32-bit layers are in a tagged block (Lr16 / Lr32) with the same layout.
        tag = data.find(b"8BIMLr16" if depth == 16 else b"8BIMLr32")
        if tag < 0:
            raise IndexError(index)
        pos = tag + 8 + ln
        count = abs(struct.unpack(">h", data[pos:pos + 2])[0])
        pos += 2
    layers = []
    for _ in range(count):
        top, left, bottom, right, nch = struct.unpack(">iiiiH", data[pos:pos + 18])
        pos += 18
        chans = []
        for _ in range(nch):
            cid = struct.unpack(">h", data[pos:pos + 2])[0]
            clen = int.from_bytes(data[pos + 2:pos + 2 + ln], "big")
            chans.append((cid, clen))
            pos += 2 + ln
        pos += 12  # blend signature, key, opacity, clipping, flags, filler
        extra = struct.unpack(">I", data[pos:pos + 4])[0]
        pos += 4 + extra
        layers.append(((top, left, bottom, right), chans))
    dtype = {8: ">u1", 16: ">u2", 32: ">f4"}[depth]
    for i, ((top, left, bottom, right), chans) in enumerate(layers):
        lh, lw = bottom - top, right - left
        planes = {}
        for cid, clen in chans:
            start = pos
            compression = struct.unpack(">H", data[pos:pos + 2])[0]
            p = pos + 2
            row_bytes = lw * depth // 8
            if compression == 0:
                raw = data[p:p + lh * row_bytes]
            elif compression == 1:
                p += lh * (4 if big else 2)
                rows = []
                for _ in range(lh):
                    row, p = _packbits(data, p, row_bytes)
                    rows.append(row)
                raw = b"".join(rows)
            elif compression in (2, 3):
                import zlib
                raw = zlib.decompress(data[p:start + clen])
                if compression == 3:  # prediction: deltas per row, on bytes (8/16-bit) or byte planes (32-bit)
                    raw = _unpredict(raw, lw, lh, depth)
            else:
                raise ValueError(f"layer compression {compression}")
            pos = start + clen
            if i == index and lh > 0 and lw > 0:
                planes[cid] = np.frombuffer(raw, dtype=dtype).reshape(lh, lw)
        if i == index:
            ids = [0] if mode == 1 else [0, 1, 2]
            ids += [-1] if -1 in planes else []
            out = np.zeros((h, w, len(ids)))
            for k, cid in enumerate(ids):
                out[top:bottom, left:right, k] = planes[cid]
            if depth == 8:
                out /= 255.0
            elif depth == 16:
                out /= 65535.0
                if mode == LAB:
                    out[..., 1:3] = ((out[..., 1:3] * 65535.0 - 32768.0) / 256.0 + 128.0) / 255.0
            return out, depth, mode
    raise IndexError(index)


def _unpredict(raw, w, h, depth):
    if depth == 16:
        a = np.frombuffer(raw, dtype=">u2").reshape(h, w)
        return np.cumsum(a, axis=1, dtype=np.uint16).astype(">u2").tobytes()
    if depth == 32:
        b = np.cumsum(np.frombuffer(raw, dtype=np.uint8).reshape(h, 4 * w), axis=1, dtype=np.uint8)
        return b.reshape(h, 4, w).transpose(0, 2, 1).copy().tobytes()
    return np.cumsum(np.frombuffer(raw, dtype=np.uint8).reshape(h, w), axis=1, dtype=np.uint8).tobytes()
