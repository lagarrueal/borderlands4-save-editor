"""Index-driven, read-only NCS extractor for BL4 legacy paks (pak v11).

Unlike repak (panics on ~60 BL4 paks) this parses only what it needs:
footer -> primary index -> full directory index -> encoded entries.
Handles pak-level Oodle-compressed entries and unencoded entries.
Never writes anywhere near the game folder.

usage:
  pak_ncs.py list <pak>
  pak_ncs.py newest <out_dir> [--include-mods]   # newest copy of every NCS
"""
import ctypes
import json
import os
import re
import struct
import sys
from pathlib import Path

GAME = Path(os.environ.get("BL4_GAME", r"C:\Program Files (x86)\Steam\steamapps\common\Borderlands 4"))
PAKS = GAME / "OakGame" / "Content" / "Paks"
PATCH_ROOT = Path(os.environ.get("BL4_PATCH_ROOT", str(Path.home() / "Documents" / "My Games" / "Borderlands 4" / "Saved" / "PersistentDownloadDir" / "Gearbox" / "Patch")))
OODLE_DLL = os.environ.get("OODLE_DLL", str(Path.home() / ".cargo" / "bin" / "oo2core_9_win64.dll"))


def rstr(b, o):
    n, = struct.unpack_from("<i", b, o)
    o += 4
    if n < 0:
        s = b[o:o - 2 * n - 2].decode("utf-16-le", "ignore")
        o += -2 * n
    else:
        s = b[o:o + n - 1].decode("utf-8", "ignore")
        o += n
    return s, o


def list_entries(pak: Path, ext=".ncs"):
    """Return {path: (offset, usize, csize, method)} for every matching entry."""
    with open(pak, "rb") as f:
        f.seek(0, 2)
        size = f.tell()
        f.seek(max(0, size - 1024))
        tail = f.read()
        pos = tail.rfind(b"\xe1\x12\x6f\x5a")
        if pos < 0:
            raise ValueError("no footer")
        ver, idx_off, idx_size = struct.unpack_from("<IQQ", tail, pos + 4)
        if idx_off + idx_size > size:
            raise ValueError("bad index offset")
        f.seek(idx_off)
        idx = f.read(idx_size)
        o = 0
        mount, o = rstr(idx, o)
        count, = struct.unpack_from("<i", idx, o); o += 4
        o += 8  # path hash seed
        has_ph, = struct.unpack_from("<i", idx, o); o += 4
        if has_ph:
            o += 8 + 8 + 20
        has_fdi, = struct.unpack_from("<i", idx, o); o += 4
        if not has_fdi:
            raise ValueError("no full directory index")
        fdi_off, fdi_size = struct.unpack_from("<QQ", idx, o); o += 16 + 20
        enc_size, = struct.unpack_from("<i", idx, o); o += 4
        enc = idx[o:o + enc_size]
        o += enc_size
        # non-encoded entries: i32 count, then full FPakEntry records
        unenc = []
        try:
            ucount, = struct.unpack_from("<i", idx, o); o += 4
            for _ in range(ucount):
                eo, es, eu, em = struct.unpack_from("<QQQI", idx, o); o += 28 + 20
                if em:
                    nb, = struct.unpack_from("<i", idx, o); o += 4 + 16 * nb
                o += 1 + 4
                unenc.append((eo, eu, es, em))
        except Exception:
            pass
        f.seek(fdi_off)
        fdi = f.read(fdi_size)

    def decode(off):
        v, = struct.unpack_from("<I", enc, off)
        q = off + 4
        if (v & 0x3F) == 0x3F:
            q += 4  # explicit compression block size
        method = (v >> 23) & 0x3F
        off32 = (v >> 31) & 1
        usize32 = (v >> 30) & 1
        size32 = (v >> 29) & 1
        if off32:
            offset, = struct.unpack_from("<I", enc, q); q += 4
        else:
            offset, = struct.unpack_from("<Q", enc, q); q += 8
        if usize32:
            usize, = struct.unpack_from("<I", enc, q); q += 4
        else:
            usize, = struct.unpack_from("<Q", enc, q); q += 8
        if method:
            fmt = "<I" if size32 else "<Q"
            csize, = struct.unpack_from(fmt, enc, q)
        else:
            csize = usize
        return offset, usize, csize, method

    p = 0
    ndirs, = struct.unpack_from("<i", fdi, p); p += 4
    out = {}
    for _ in range(ndirs):
        d, p = rstr(fdi, p)
        nf, = struct.unpack_from("<i", fdi, p); p += 4
        for _ in range(nf):
            fn, p = rstr(fdi, p)
            e, = struct.unpack_from("<i", fdi, p); p += 4
            if ext and not fn.lower().endswith(ext):
                continue
            if e >= 0:
                out[mount + d + fn] = decode(e)
            else:
                k = -e - 1
                out[mount + d + fn] = unenc[k] if 0 <= k < len(unenc) else None
    return out


_oodle = None


def oodle(comp, n):
    global _oodle
    if _oodle is None:
        _oodle = ctypes.WinDLL(OODLE_DLL)
        _oodle.OodleLZ_Decompress.restype = ctypes.c_int64
        _oodle.OodleLZ_Decompress.argtypes = [
            ctypes.c_char_p, ctypes.c_int64, ctypes.c_void_p, ctypes.c_int64,
            ctypes.c_int32, ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int64,
            ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int64, ctypes.c_int32]
    buf = ctypes.create_string_buffer(n + 64)
    r = _oodle.OodleLZ_Decompress(comp, len(comp), buf, n, 1, 0, 0, None, 0, None, None, None, 0, 3)
    if r != n:
        raise ValueError(f"oodle block failed ({r} != {n})")
    return buf.raw[:n]


def read_entry(pak: Path, ent):
    offset, usize, csize, method = ent
    with open(pak, "rb") as f:
        f.seek(offset)
        h = f.read(8192)
        # FPakEntry header: Offset u64, Size u64, USize u64, Method u32, Hash[20]
        hmethod, = struct.unpack_from("<I", h, 24)
        if hmethod == 0:
            start = offset + 53  # + flags(1) + blocksize(4)
            f.seek(start)
            return f.read(usize)
        nb, = struct.unpack_from("<i", h, 48)
        blocks = [struct.unpack_from("<QQ", h, 52 + 16 * i) for i in range(nb)]
        q = 52 + 16 * nb + 1
        bsize, = struct.unpack_from("<I", h, q)
        out = bytearray()
        for (s, e) in blocks:
            f.seek(offset + s)
            comp = f.read(e - s)
            out += oodle(comp, min(bsize, usize - len(out)))
        return bytes(out)


def pak_version(p: Path):
    m = re.search(r"pakchunk(\d+)-[Ww]indows_(\d+)_P", p.name)
    if m:
        return (int(m.group(2)), int(m.group(1)))
    m = re.search(r"_(\d+)_P\.pak$", p.name)
    return (int(m.group(1)), -1) if m else (-1, -1)


def all_paks(include_mods):
    paks = sorted(PAKS.glob("*.pak"))
    if PATCH_ROOT.exists():
        paks += sorted(PATCH_ROOT.glob("*/*.pak"))
    if not include_mods:
        paks = [p for p in paks if p.name.lower().startswith("pakchunk")]
    return paks


def main():
    if sys.argv[1] == "list":
        for k, v in list_entries(Path(sys.argv[2]), None if "--all" in sys.argv else ".ncs").items():
            print(k, v)
        return
    if sys.argv[1] == "newest":
        out = Path(sys.argv[2])
        include_mods = "--include-mods" in sys.argv
        out.mkdir(parents=True, exist_ok=True)
        newest = {}
        errors = []
        for pak in all_paks(include_mods):
            try:
                ents = list_entries(pak)
            except Exception as e:
                if pak.stat().st_size > 1000:
                    errors.append((pak.name, str(e)))
                continue
            for path, ent in ents.items():
                # ent None == FDI location i32::MIN: a *deleted-file record*.
                # A newer patch pak deletes the file; it must shadow older
                # copies. (This is also exactly what makes repak panic.)
                name = path.rsplit("/", 1)[-1]
                v = pak_version(pak)
                if name not in newest or v > newest[name][0]:
                    newest[name] = (v, pak, path, ent)
        manifest = {}
        deleted = {}
        for name, (v, pak, path, ent) in sorted(newest.items()):
            if ent is None:
                deleted[name] = pak.name
                continue
            try:
                data = read_entry(pak, ent)
            except Exception as e:
                errors.append((pak.name, f"{name}: {e}"))
                continue
            if data[:4] != b"\x01NCS":
                errors.append((pak.name, f"{name}: no NCS magic"))
                continue
            (out / name.replace("Nexus-Data-", "")).write_bytes(data)
            manifest[name] = {"pak": pak.name, "path": path, "size": len(data),
                              "pak_compressed": bool(ent[3])}
        json.dump({"files": manifest, "deleted": deleted}, open(out / "_sources.json", "w"), indent=1)
        print(f"wrote {len(manifest)} NCS files to {out}; {len(deleted)} deleted by newer paks")
        for e in errors:
            print("ERROR", e)


if __name__ == "__main__":
    main()
