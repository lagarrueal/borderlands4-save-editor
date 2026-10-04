"""Decode a cooked UE5.5 Texture2D (legacy .uexp [+ .ubulk] from `retoc to-legacy`) to PNG.

Layout observed in BL4 UI textures (UE 5.5, inline mips, bulk-data meta moved to the package header):
  ... i32 SizeX | i32 SizeY | u32 PackedData | FString PixelFormat ("PF_BC7")
      | i32 FirstMipToSerialize | i32 NumMips
      | NumMips x { u32 bCooked | <inline payload if any> | i32 SizeX | i32 SizeY | i32 SizeZ }
Payload size is derived from the mip dims and the format. If the payload is not inline
(texture with .ubulk), mips are taken from the .ubulk in order (largest first is at the end
of the ubulk? -> we just try both ends).
Usage: tex2png.py <file.uexp> <out.png> [--all]
"""
import io
import struct
import sys
from pathlib import Path

from PIL import Image

FORMATS = {  # name -> (block w, bytes per block, dds builder)
    b"PF_BC7": (4, 16, "BC7"),
    b"PF_DXT1": (4, 8, "DXT1"),
    b"PF_DXT5": (4, 16, "DXT5"),
    b"PF_BC4": (4, 8, "BC4"),
    b"PF_BC5": (4, 16, "BC5"),
    b"PF_B8G8R8A8": (1, 4, "BGRA"),
    b"PF_G8": (1, 1, "L"),
}


def payload_size(fmt, w, h):
    bw, bpb, _ = FORMATS[fmt]
    return ((w + bw - 1) // bw) * ((h + bw - 1) // bw) * bpb


def to_image(fmt, w, h, data):
    kind = FORMATS[fmt][2]
    if kind == "BGRA":
        return Image.frombytes("RGBA", (w, h), data, "raw", "BGRA")
    if kind == "L":
        return Image.frombytes("L", (w, h), data)
    # Build a DDS in memory and let Pillow's BCn decoder do the work.
    DDSD = 0x1 | 0x2 | 0x4 | 0x1000 | 0x80000
    if kind in ("DXT1", "DXT5"):
        pf = struct.pack("<II4sIIIII", 32, 0x4, kind.encode(), 0, 0, 0, 0, 0)
        dx10 = b""
    else:
        dxgi = {"BC7": 98, "BC4": 80, "BC5": 83}[kind]
        pf = struct.pack("<II4sIIIII", 32, 0x4, b"DX10", 0, 0, 0, 0, 0)
        dx10 = struct.pack("<IIIII", dxgi, 3, 0, 1, 0)
    hdr = struct.pack("<4sIIIIIII", b"DDS ", 124, DDSD, h, w, len(data), 0, 1)
    hdr += b"\0" * 44 + pf + struct.pack("<IIIII", 0x1000, 0, 0, 0, 0)
    return Image.open(io.BytesIO(hdr + dx10 + data)).convert("RGBA")


def find_textures(d: bytes):
    """Yield (fmt, mips[(w,h,offset_or_None,size)]) for every platform-data block."""
    pos = 0
    while True:
        i = d.find(b"PF_", pos)
        if i < 0:
            return
        pos = i + 3
        n = struct.unpack_from("<i", d, i - 4)[0]
        if not 3 < n < 32:
            continue
        name = d[i:i + n - 1]
        if name not in FORMATS:
            print("unsupported format", name, file=sys.stderr)
            continue
        sx, sy, packed = struct.unpack_from("<iiI", d, i - 16)
        o = i + n
        first, nmips = struct.unpack_from("<ii", d, o)
        o += 8
        mips = []
        for _ in range(nmips):
            _cooked = struct.unpack_from("<I", d, o)[0]
            o += 4
            # Try: inline payload sized for (w,h) followed by matching dims.
            found = False
            w, h = sx >> len(mips), sy >> len(mips)
            w, h = max(w, 1), max(h, 1)
            sz = payload_size(name, w, h)
            if o + sz + 12 <= len(d):
                mw, mh, mz = struct.unpack_from("<iii", d, o + sz)
                if (mw, mh) == (w, h):
                    mips.append((w, h, o, sz))
                    o += sz + 12
                    found = True
            if not found:
                mw, mh, mz = struct.unpack_from("<iii", d, o)
                mips.append((mw, mh, None, payload_size(name, mw, mh)))
                o += 12
        yield name, sx, sy, mips


def main():
    src = Path(sys.argv[1])
    out = Path(sys.argv[2])
    d = src.read_bytes()
    ubulk = src.with_suffix(".ubulk")
    bulk = ubulk.read_bytes() if ubulk.exists() else b""
    for k, (fmt, sx, sy, mips) in enumerate(find_textures(d)):
        w, h, off, sz = mips[0]
        if off is not None:
            data = d[off:off + sz]
        else:
            # Non-inline: mip 0 is first in the .ubulk in UE5 cooked data.
            data = bulk[:sz]
            if len(data) < sz:
                print(f"export {k}: mip0 {w}x{h} not inline and no .ubulk", file=sys.stderr)
                continue
        img = to_image(fmt, w, h, data)
        target = out if k == 0 else out.with_name(f"{out.stem}_{k}{out.suffix}")
        img.save(target)
        print(f"export {k}: {fmt.decode()} {sx}x{sy} mips={len(mips)} inline={off is not None} -> {target}")
        if "--all" not in sys.argv:
            break


if __name__ == "__main__":
    main()
