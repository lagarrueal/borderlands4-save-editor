"""Pack the item-card icons the editor shows into data/icons.bin.

Source: PNGs decoded from the game's UI textures (uiresources/_Shared/assets),
produced by:  retoc to-legacy --no-shaders --version UE5_5 -f uiresources <dir with
hard links to global.* and pakchunk0-Windows_0_P.*> <legacy_dir>
then        python tex2png.py <each .uexp> <png>   (see batch loop in README).

usage: python pack_icons.py <png_root/uiresources/_Shared/assets> [out=data/icons.bin]
Format: u32 count, then per icon: u16 name length, utf-8 name, u32 size, png bytes.
"""
import struct
import sys
from pathlib import Path

src = Path(sys.argv[1])
out = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(__file__).resolve().parent.parent / "data" / "icons.bin"
DIRS = {
    "ico_ui_art_item_card_type": "type",
    "ui_art_manufacturer_logos": "mfr",
    "ico_ui_art_elemental_damage": "elem",
    "ico_ui_art_firmwares/item_card_size": "fw",
    "ico_ui_art_item_augments": "aug",
}
items = []
for d, prefix in DIRS.items():
    for f in sorted((src / d).rglob("*.png")):
        name = f.stem.lower()
        if prefix == "mfr" and "logomark" not in name:
            continue
        items.append((f"{prefix}/{name}", f.read_bytes()))
with open(out, "wb") as fh:
    fh.write(struct.pack("<I", len(items)))
    for name, data in items:
        nb = name.encode()
        fh.write(struct.pack("<H", len(nb)) + nb + struct.pack("<I", len(data)) + data)
print(len(items), "icons ->", out, out.stat().st_size // 1024, "KB")
