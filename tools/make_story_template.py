"""Extract the game's own "story skip" mission records into data/story_template.yaml.

Input: the decrypted YAML of a character the game created with the
"Ultimate Vault Hunter" story skip (testdata/yaml/7.yaml here). Keeps every
missionset_main_* set exactly as the game wrote it, plus the story globals.
usage: python make_story_template.py <skip_save.yaml> [out]
"""
import re
import sys
from pathlib import Path

src = Path(sys.argv[1]).read_text(encoding="utf-8").split("\n")
out = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(__file__).resolve().parent.parent / "data" / "story_template.yaml"
KEEP_GLOBALS = {"movegrant_glide", "movegrant_grapplegrabber", "movegrant_ordonitegloves", "lockdownlifted",
                "introlights1", "introlights2", "introlights3", "introlights4", "movegrant_echolocation",
                "prologue_completed", "mainmissioncomplete", "repkit_unlocked"}

res = ["missions: ", "  local_sets: "]
i = src.index("  local_sets: ")
cur_keep = False
for line in src[i + 1:]:
    if not line.startswith("    "):
        break
    m = re.match(r"^    (\S+):", line)
    if m:
        cur_keep = m.group(1).startswith("missionset_main_")
    if cur_keep:
        res.append(line)
res.append("globals: ")
g = src.index("globals: ")
for line in src[g + 1:]:
    if not line.startswith("  "):
        break
    k = line.strip().split(":")[0]
    if k in KEEP_GLOBALS:
        res.append(line)
out.write_text("\n".join(res), encoding="utf-8")
print(out, len(res), "lines")
