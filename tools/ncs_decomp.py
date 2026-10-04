"""Decompress BL4 Nexus .ncs files.

Format (per monokrome/bl4, crates bl4-ncs/src/data.rs):

  NCS header, 16 bytes:
    [0]      version byte (0x01)
    [1..4]   b"NCS"
    [4..8]   compression flag, LE u32 (0 = stored)
    [8..12]  decompressed size, LE u32
    [12..16] compressed size, LE u32

  Inner payload at offset 16:
    0x00  Oodle magic 0xb7756362, BIG-endian
    0x08  format flags
    0x0c  block count, BIG-endian
    0x40  block size table, block_count * u32 BIG-endian
    ...   concatenated Oodle blocks, each decompressing to <= 256KB

My earlier attempt failed because I fed this whole inner header straight to
OodleLZ_Decompress instead of skipping to the block data.
"""

import ctypes
import os
import struct
import sys

DLL = os.environ.get("OODLE_DLL", os.path.join(os.path.expanduser("~"), ".cargo", "bin", "oo2core_9_win64.dll"))
BLOCK_DECOMP_SIZE = 0x40000
INNER_HEADER_MIN = 0x40

_oodle = ctypes.WinDLL(DLL)
_oodle.OodleLZ_Decompress.restype = ctypes.c_int64
_oodle.OodleLZ_Decompress.argtypes = [
    ctypes.c_char_p, ctypes.c_int64,
    ctypes.c_void_p, ctypes.c_int64,
    ctypes.c_int32, ctypes.c_int32, ctypes.c_int32,
    ctypes.c_void_p, ctypes.c_int64,
    ctypes.c_void_p, ctypes.c_void_p,
    ctypes.c_void_p, ctypes.c_int64,
    ctypes.c_int32,
]


def oodle_block(comp, decomp_size):
    buf = ctypes.create_string_buffer(decomp_size + 64)
    n = _oodle.OodleLZ_Decompress(
        comp, len(comp), buf, decomp_size,
        1, 0, 0, None, 0, None, None, None, 0, 3,
    )
    if n <= 0:
        return None
    return buf.raw[:n]


def decompress(path):
    data = open(path, "rb").read()
    if len(data) < 16 or data[1:4] != b"NCS":
        return None, "not an NCS file"

    flag, dsize, csize = struct.unpack_from("<III", data, 4)
    if flag == 0:
        return data[16:16 + csize], "stored"

    inner = data[16:16 + csize]
    if len(inner) < INNER_HEADER_MIN + 4:
        return None, "inner payload too short"
    if struct.unpack_from(">I", inner, 0)[0] != 0xB7756362:
        return None, "bad oodle magic"

    nblocks = struct.unpack_from(">I", inner, 0x0C)[0]
    if nblocks == 0 or nblocks > 4096:
        return None, f"implausible block count {nblocks}"

    sizes = [
        struct.unpack_from(">I", inner, INNER_HEADER_MIN + 4 * i)[0]
        for i in range(nblocks)
    ]
    pos = INNER_HEADER_MIN + 4 * nblocks

    out = bytearray()
    for i, bsize in enumerate(sizes):
        block = inner[pos:pos + bsize]
        pos += bsize
        want = min(BLOCK_DECOMP_SIZE, dsize - len(out))
        got = oodle_block(block, want)
        if got is None:
            return None, f"oodle failed on block {i}/{nblocks} (size {bsize}, want {want})"
        out += got

    return bytes(out), f"{nblocks} block(s)"


def main():
    src, dst = sys.argv[1], sys.argv[2]
    os.makedirs(dst, exist_ok=True)
    ok = fail = 0
    errs = {}
    for fn in sorted(os.listdir(src)):
        if not fn.endswith(".ncs"):
            continue
        out, note = decompress(os.path.join(src, fn))
        if out is None:
            fail += 1
            errs[note] = errs.get(note, 0) + 1
            continue
        open(os.path.join(dst, fn + ".bin"), "wb").write(out)
        ok += 1
    print(f"decompressed {ok}, failed {fail}")
    for k, v in sorted(errs.items(), key=lambda x: -x[1])[:5]:
        print(f"  {v:4}  {k}")


if __name__ == "__main__":
    main()
