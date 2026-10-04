"""Extract the NCS game-data tables the editor needs from an installed BL4.

Read-only on the game folder. For each NCS file name, takes the copy from the
highest-priority pak (pakchunkN-Windows_M_P: highest M wins; a mod pak
`Name_PRIO_P` outranks every base pak), decompresses it and dumps it to JSON
with the bl4 NCS parser (monokrome/bl4 `bl4 ncs show --json`).

usage:
  python extract_game_data.py <out_dir> [--vanilla] [--bl4 path\\to\\bl4.exe]

Output layout:
  <out_dir>/bin/<table>_c<chunk>.bin   decompressed payloads
  <out_dir>/json/<table>_c<chunk>.json parsed tables (only the ones we use)
  <out_dir>/sources.json               which pak each file came from
"""
import json
import re
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pakncs  # noqa: E402
from ncs_decomp import decompress  # noqa: E402

# Tables (by base name) converted to JSON. Everything else stays as .bin.
TABLES = [
    "inv", "inv_name_part", "inv_name_strategy", "inv_custom", "ui_stat", "ui_stat_group",
    "Manufacturer", "Rarity", "gbx_ue_data_table", "attribute", "Firmware",
    "GbxActorPart", "GbxActor", "hover_drive", "Mission", "missionset", "progress_graph",
    "challenge", "xp_progression", "profile_skip_reward_def", "display_data",
    "unlockable", "usable_consumer", "inv_stat", "Capital", "Resident", "ItemPoolList", "itempool",
]

DEFAULT_BL4 = HERE.parent.parent / "third_party" / "bl4" / "target" / "release" / "bl4.exe"


def newest_files(include_mods):
    newest = {}
    errors = []
    for pak in pakncs.all_paks(include_mods):
        try:
            ents = pakncs.list_entries(pak)
        except Exception as e:  # tiny stub paks have no index
            if pak.stat().st_size > 1000:
                errors.append(f"{pak.name}: {e}")
            continue
        v = pakncs.pak_version(pak)
        for path, ent in ents.items():
            name = path.rsplit("/", 1)[-1]
            if name not in newest or v > newest[name][0]:
                newest[name] = (v, pak, path, ent)
    return newest, errors


def main():
    args = sys.argv[1:]
    out = Path(args[0])
    include_mods = "--vanilla" not in args
    bl4 = Path(args[args.index("--bl4") + 1]) if "--bl4" in args else DEFAULT_BL4
    (out / "ncs").mkdir(parents=True, exist_ok=True)
    (out / "bin").mkdir(parents=True, exist_ok=True)
    (out / "json").mkdir(parents=True, exist_ok=True)

    newest, errors = newest_files(include_mods)
    sources = {}
    for name, (v, pak, path, ent) in sorted(newest.items()):
        if ent is None:  # deleted by a newer pak
            continue
        try:
            data = pakncs.read_entry(pak, ent)
        except Exception as e:
            errors.append(f"{name}: {e}")
            continue
        if data[:4] != b"\x01NCS":
            continue
        stem = name.replace("Nexus-Data-", "")[:-4]
        ncs_path = out / "ncs" / (stem + ".ncs")
        ncs_path.write_bytes(data)
        payload, how = decompress(str(ncs_path))
        if payload is None:
            errors.append(f"{name}: {how}")
            continue
        m = re.match(r"pakchunk(\d+)-", pak.name, re.I)
        chunk = m.group(1) if m else re.sub(r"\D", "", stem)[-2:] or "x"
        base = stem[: -len(chunk)] if stem.endswith(chunk) else stem
        binname = f"{base}_c{chunk}.bin"
        (out / "bin" / binname).write_bytes(payload)
        sb = struct.unpack_from("<I", payload, 8)[0]
        table = [s for s in payload[16:16 + sb].split(b"\0") if s][0].decode("utf-8", "replace")
        sources[binname] = {"file": name, "pak": pak.name, "table": table, "mod": not pak.name.lower().startswith("pakchunk")}
    json.dump(sources, open(out / "sources.json", "w"), indent=1, sort_keys=True)
    print(f"{len(sources)} NCS payloads ({'with' if include_mods else 'without'} mods)")

    wanted = {t.lower() for t in TABLES}
    n = 0
    for binname in sorted(sources):
        base = binname.rsplit("_c", 1)[0]
        if base.lower() not in wanted:
            continue
        dst = out / "json" / binname.replace(".bin", ".json")
        with open(dst, "wb") as f:
            r = subprocess.run([str(bl4), "ncs", "show", str(out / "bin" / binname), "--json"],
                               stdout=f, stderr=subprocess.PIPE)
        if r.returncode != 0:
            errors.append(f"json {binname}: {r.stderr.decode(errors='replace')[:200]}")
        n += 1
    print(f"{n} tables parsed to JSON")
    for e in errors:
        print("WARN", e)


if __name__ == "__main__":
    main()
