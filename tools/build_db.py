"""Build the editor's game database (data/bl4db.json.gz) from extracted NCS JSON.

usage:
  python build_db.py [--installed build/gamedata/installed] [--vanilla build/gamedata/vanilla]
                     [--out data/bl4db.json.gz]

Inputs come from extract_game_data.py. "installed" is the game as it will
load (including mod paks); "vanilla" excludes mod paks and is used only to
flag what mods add, so the editor can say "valid only with mod X".
"""
import gzip
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEED = ROOT / "research" / "save-fields-data"


def arg(name, default):
    a = sys.argv[1:]
    return Path(a[a.index(name) + 1]) if name in a else default


INSTALLED = arg("--installed", ROOT / "build" / "gamedata" / "installed")
VANILLA = arg("--vanilla", ROOT / "build" / "gamedata" / "vanilla")
OUT = arg("--out", ROOT / "data" / "bl4db.json.gz")


# ------------------------------------------------------------------ helpers

def load_tables(d: Path, base: str):
    """All chunks of one table, ordered by chunk number: [(chunk, json)]."""
    out = []
    for f in sorted((d / "json").glob(f"{base}_c*.json")):
        m = re.fullmatch(re.escape(base) + r"_c(\d+|x)\.json", f.name, re.I)
        if not m:
            continue
        try:
            j = json.load(open(f, encoding="utf-8"))
        except Exception:
            continue
        out.append((m.group(1) if m else "x", j))
    return out


def entries(d: Path, base: str, table=None):
    """Yield (chunk, record_tags, entry) for every entry of every chunk."""
    for chunk, j in load_tables(d, base):
        for tname, tab in j.get("tables", {}).items():
            if table and tname != table:
                continue
            for rec in tab.get("records", []):
                rtags = []
                for t in rec.get("tags", []):
                    if t.get("__tag") == "d":
                        rtags = t.get("list", [])
                for e in rec.get("entries", []):
                    yield chunk, rtags, e


def ref(s):
    """"inv'Weapon_PS'" -> "Weapon_PS"."""
    if isinstance(s, str) and "'" in s:
        return s[s.index("'") + 1:s.rindex("'")]
    return s


def text(s):
    """Localized string "Namespace, GUID, Text" -> "Text"."""
    if not isinstance(s, str):
        return None
    parts = s.split(", ", 2)
    if len(parts) == 3 and re.fullmatch(r"[0-9A-F]{32}", parts[1]):
        return parts[2]
    return s


def taglist(v, k):
    out = []
    for t in v.get(k) or []:
        if isinstance(t, dict):
            out += list(t.keys())
        elif isinstance(t, str):
            out.append(t)
    return out


def as_list(x):
    if x is None:
        return []
    return x if isinstance(x, list) else [x]


# ------------------------------------------------------------------ names

def build_names(d):
    names = {}
    for _, _, e in entries(d, "inv_name_part"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        t = text(v.get("partname"))
        if t is not None:
            names[e["key"].lower()] = {"t": t, "p": float(v.get("priority") or 0)}
    return names


def build_uistats(d):
    ui = {}
    for _, _, e in entries(d, "ui_stat"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        sv = v.get("statvalue") or v.get("statlabel") or {}
        t = text(sv.get("formattext")) if isinstance(sv, dict) else None
        if t:
            ui[e["key"].lower()] = t
    return ui


def build_manufacturers(d):
    out = {}
    for _, _, e in entries(d, "Manufacturer"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        out[e["key"].lower()] = {"name": text(v.get("displayname")) or e["key"],
                                 "desc": text(v.get("description"))}
    return out


def build_firmware(d):
    out = {}
    for _, _, e in entries(d, "Firmware"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        name = None
        for k in ("displayname", "name", "firmwarename"):
            if isinstance(v.get(k), str):
                name = text(v[k])
                break
        desc = None
        for k in ("description", "desc"):
            if isinstance(v.get(k), str):
                desc = text(v[k])
        out[e["key"].lower()] = {"name": name, "desc": desc}
    return out


# ------------------------------------------------------------------ items

WEAPON_TYPES = {"ps": "Pistol", "sg": "Shotgun", "ar": "Assault Rifle", "sm": "SMG", "sr": "Sniper Rifle"}
POOLS = {"weapon", "classmod", "armor_shield", "repair_kit", "heavy_weapon_gadget",
         "grenade_gadget", "shield", "enhancement", "energy_shield"}


def kind_of(key, basetype):
    k = key.lower()
    if k in POOLS:
        return "pool"
    m = re.fullmatch(r"[a-z]{3}_(ps|sg|ar|sm|sr)", k)
    if m:
        return "weapon"
    if k.endswith("_hw"):
        return "heavy"
    if k.endswith("_grenade_gadget"):
        return "grenade"
    if k.endswith("_shield"):
        return "shield"
    if k.endswith("_repair_kit"):
        return "repkit"
    if k.endswith("_enhancement"):
        return "enhancement"
    if k.startswith("classmod_"):
        return "classmod"
    if k.endswith("_hover_drive"):
        return "hoverdrive"
    return "other"


def selection_rules(v):
    slots = {}
    pr = v.get("parttypeselectionrules")
    if isinstance(pr, dict):
        for pair in (pr.get("pairs") or {}).values():
            k = pair.get("key")
            val = pair.get("value") or {}
            r = {}
            if isinstance(val, dict):
                pc = val.get("partcount")
                if isinstance(pc, dict):
                    if pc.get("min") is not None:
                        r["min"] = int(float(pc["min"]))
                    if pc.get("max") is not None:
                        r["max"] = int(float(pc["max"]))
                parts = val.get("parts")
                if parts:
                    r["parts"] = [p.get("part").lower() for p in parts if isinstance(p, dict) and p.get("part")]
            slots[k.lower()] = r
    tags = []
    for tr in as_list(v.get("parttagselectionrules")):
        if not isinstance(tr, dict):
            continue
        t = {"tags": taglist(tr, "tags")}
        if tr.get("min") is not None:
            t["min"] = int(float(tr["min"]))
        if tr.get("max") is not None:
            t["max"] = int(float(tr["max"]))
        tags.append(t)
    return slots, tags


def dt_ref(mv):
    """Compact a modifier value: constant / data-table cell / attribute."""
    if not isinstance(mv, dict):
        return None
    out = {}
    if mv.get("constant") not in (None, "0.000000") or ("constant" in mv and len(mv) == 1):
        try:
            out["k"] = float(mv["constant"])
        except (TypeError, ValueError):
            pass
    dv = mv.get("datatablevalue")
    if isinstance(dv, dict):
        dt = ref(dv.get("datatable"))
        if dt and dt.lower() != "none":
            out["dt"] = dt
            if dv.get("rowname") and dv["rowname"] != "None":
                out["row"] = dv["rowname"]
            if dv.get("columnname") and dv["columnname"] != "None":
                out["col"] = dv["columnname"]
        elif dv.get("rowname") and dv["rowname"] != "None":
            out["row"] = dv["rowname"]
    a = ref(mv.get("attribute"))
    if a and a.lower() != "none":
        out["attr"] = a
    if mv.get("postscale") not in (None, "1.000000"):
        try:
            out["ps"] = float(mv["postscale"])
        except (TypeError, ValueError):
            pass
    return out or None


def compact_aspects(v):
    """Stat-relevant data of a part: attribute effects, data-table templates,
    stat modifiers, fire behaviour values, UI stat lines and naming."""
    fx = []
    tpl = []
    mods = []
    beh = {}
    ui = []
    title = []
    prefix = []
    name_row = None
    for a in as_list(v.get("aspects")):
        if not isinstance(a, dict):
            continue
        parent = ref(a.get("parent"))
        aet = a.get("attributeeffecttemplate")
        if isinstance(aet, dict):
            mv = dt_ref(aet.get("modifiervalue"))
            if mv:
                tpl.append({"asp": parent, **mv})
        effs = list(as_list(a.get("attributeeffects")))
        for um in as_list(a.get("usemodeattributeeffects")):
            if isinstance(um, dict):
                effs += [dict(x, _mode=um.get("usemodebitmask")) for x in as_list(um.get("attributeeffects")) if isinstance(x, dict)]
        for ef in effs:
            if not isinstance(ef, dict):
                continue
            x = {"a": ref(ef.get("attributetomodify")), "op": ef.get("modifiertype") or "ScaleMultiply"}
            mv = dt_ref(ef.get("modifiervalue"))
            if mv:
                x.update(mv)
            if ef.get("_mode"):
                x["mode"] = ef["_mode"]
            fx.append(x)
        for sm in as_list(a.get("statmodifiers")):
            if isinstance(sm, dict):
                x = {"stat": sm.get("stattagname")}
                mv = dt_ref(sm.get("modifiervalue"))
                if mv:
                    x.update(mv)
                mods.append(x)
        b = a.get("behavior")
        if isinstance(b, dict) and parent and "fire" in parent.lower():
            for k in ("damage", "firerate", "spread", "projectilespershot", "automaticburstcount", "accuracyimpulse"):
                if k in b:
                    val = b[k]
                    if isinstance(val, dict):
                        r = dt_ref(val)
                        if r:
                            beh[k] = r
                    else:
                        try:
                            beh[k] = {"k": float(val)}
                        except (TypeError, ValueError):
                            pass
            beh["_asp"] = parent
        for u in as_list(a.get("uistatstoinclude")):
            ui.append(ref(u).lower())
        for t in as_list(a.get("titlepartlist")):
            title.append(ref(t).lower())
        for t in as_list(a.get("prefixpartlist")):
            prefix.append(ref(t).lower())
        if a.get("datatablerowname") and "Naming" in str(a.get("structtype", "")):
            name_row = a["datatablerowname"]
    out = {}
    if fx:
        out["fx"] = fx
    if tpl:
        out["tpl"] = tpl
    if mods:
        out["mods"] = mods
    if beh:
        out["beh"] = beh
    if ui:
        out["ui"] = ui
    if title:
        out["title"] = title
    if prefix:
        out["prefix"] = prefix
    if name_row:
        out["name_row"] = name_row
    return out


def build_items(d, names, uistats, firmware):
    cats = {}
    key_to_cat = {}
    allentries = []
    for chunk, rtags, e in entries(d, "inv"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        si = v.get("serialindex")
        if si and si.get("_scope") == "Root":
            cid = int(si["index"])
            c = cats.setdefault(cid, {"id": cid, "comps": {}})
            mfr = ref(v.get("manufacturer"))
            c.update({
                "key": e["key"].lower(),
                "base": ref(v.get("basetype")),
                "name": text(v.get("itembasename")),
                "mfr": mfr.lower() if isinstance(mfr, str) else None,
                "naming": ref(v.get("namingstrategydef")),
                "parttypes": [p.lower() for p in v.get("parttypes") or []],
                "status": si.get("status"),
                "chunk": chunk,
                "kind": kind_of(e["key"], ref(v.get("basetype"))),
            })
            asp = compact_aspects(v)
            ui = [ref(u).lower() for u in as_list(v.get("uistats"))]
            if ui:
                asp["ui"] = ui + asp.get("ui", [])
            if asp:
                c["asp"] = asp
            key_to_cat[e["key"].lower()] = cid
        allentries.append((chunk, rtags, e))

    parts = {}
    for chunk, rtags, e in allentries:
        cid = key_to_cat.get(e["key"].lower())
        if cid is None:
            continue
        for de in e.get("dep_entries", []):
            dv = de["value"] if isinstance(de["value"], dict) else {}
            slot = de["dep_table_name"].lower()
            key = de["key"].lower()
            if slot == "inv_comp":
                slots, tags = selection_rules(dv)
                comp = {"base": ref(dv.get("basecomposition")), "basetags": taglist(dv, "basetags")}
                if slots:
                    comp["slots"] = slots
                if tags:
                    comp["tagrules"] = tags
                prev = cats[cid]["comps"].get(key)
                if prev:  # an extension record refines the comp
                    for k2, v2 in comp.items():
                        if v2:
                            if isinstance(v2, dict) and isinstance(prev.get(k2), dict):
                                prev[k2].update(v2)
                            else:
                                prev[k2] = v2
                else:
                    cats[cid]["comps"][key] = comp
            si = dv.get("serialindex")
            if not si:
                continue
            idx = int(si["index"])
            p = {"c": cid, "i": idx, "k": key, "s": slot, "st": si.get("status") or "Active", "chunk": chunk}
            for fld, src in (("add", "addtags"), ("dep", "dependencytags"), ("excl", "exclusiontags")):
                t = taglist(dv, src)
                if t:
                    p[fld] = [x.lower() for x in t]
            if slot == "inv_comp":
                bt = taglist(dv, "basetags")
                if bt:
                    p["add"] = sorted(set(p.get("add", []) + [x.lower() for x in bt]))
            if dv.get("debugdisplaydescription"):
                p["desc"] = dv["debugdisplaydescription"]
            mgs = dv.get("mingamestage")
            if isinstance(mgs, dict) and mgs.get("attribute"):
                p["mgs"] = ref(mgs["attribute"])
            if str(dv.get("bexcludefromglobalpool", "")).lower() == "true":
                p["noglobal"] = True
            asp = compact_aspects(dv)
            # firmware display name
            for a in as_list(dv.get("aspects")):
                if isinstance(a, dict) and a.get("firmware"):
                    fw = ref(a["firmware"]).lower()
                    p["fw"] = fw
                if isinstance(a, dict) and a.get("passives"):
                    pas = a["passives"]
                    p["passive"] = [{"graph": ref(x.get("progressgraph")), "node": x.get("nodename")} for x in as_list(pas) if isinstance(x, dict)]
                    p["points"] = int(a.get("points") or 1)
            # resolved display strings
            tt = [names[t] for t in asp.get("title", []) if t in names]
            if tt:
                p["title"] = max(tt, key=lambda x: x["p"])["t"]
            pf = [names[t] for t in asp.get("prefix", []) if t in names]
            if pf:
                p["prefix"] = max(pf, key=lambda x: x["p"])["t"]
            lines = [uistats[u] for u in asp.get("ui", []) if u in uistats]
            if lines:
                p["text"] = lines
            for k in ("fx", "tpl", "mods", "beh", "name_row"):
                if k in asp:
                    p[k] = asp[k]
            pas = [ref(x.get("parent")).lower() for x in as_list(dv.get("aspects")) if isinstance(x, dict) and x.get("parent")]
            if pas:
                p["asp"] = sorted(set(pas))
            k2 = f"{cid}:{idx}"
            if k2 in parts and parts[k2]["k"] != key:
                p["conflict"] = parts[k2]["k"]
            parts[k2] = p
    return cats, parts


COL_GUID = re.compile(r"_\d+_[0-9a-f]{32}$", re.I)


def norm_col(c):
    return COL_GUID.sub("", c.lower())


def build_tables(d):
    """All gbx_ue_data_tables as {table: {row: {column: value}}} (lower case,
    GUID suffixes stripped from column names)."""
    out = {}
    for _, _, e in entries(d, "gbx_ue_data_table"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        rows = {}
        for r in v.get("data") or []:
            if not isinstance(r, dict) or not r.get("row_name"):
                continue
            cols = {}
            for k, val in (r.get("row_value") or {}).items():
                if isinstance(val, str):
                    try:
                        cols[norm_col(k)] = float(val)
                    except ValueError:
                        cols[norm_col(k)] = val
            rows[r["row_name"].lower()] = cols
        out[e["key"].lower()] = rows
    return out


def build_attributes(d):
    out = {}
    for _, _, e in entries(d, "attribute"):
        if isinstance(e["value"], dict):
            out[e["key"].lower()] = e["value"]
    return out


def build_aspect_defs(d):
    """inv_aspect definitions (templates parts point at with `parent`)."""
    out = {}
    for _, _, e in entries(d, "Resident", "inv_aspect"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        a = compact_aspects({"aspects": [dict(v, parent=None)]})
        rec = {k: a[k] for k in ("fx", "beh", "mods") if k in a}
        if v.get("parent"):
            rec["parent"] = ref(v["parent"]).lower()
        out[e["key"].lower()] = rec
    return out


def build_bases(d):
    """Aspects of every inv entry by key, so stats can walk the base-type chain."""
    out = {}
    for _, _, e in entries(d, "inv"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        if not v:
            continue
        a = compact_aspects(v)
        rec = out.setdefault(e["key"].lower(), {})
        if v.get("basetype"):
            rec["base"] = ref(v["basetype"]).lower()
        for k in ("fx", "tpl", "beh", "mods"):
            if k in a:
                rec[k] = rec.get(k, []) + a[k] if isinstance(a[k], list) else a[k]
        # parent aspect names (template aspects without own data)
        pas = [ref(x.get("parent")).lower() for x in as_list(v.get("aspects")) if isinstance(x, dict) and x.get("parent")]
        if pas:
            rec["asp"] = sorted(set(rec.get("asp", []) + pas))
    return {k: v for k, v in out.items() if v}


def main():
    names = build_names(INSTALLED)
    uistats = build_uistats(INSTALLED)
    mfrs = build_manufacturers(INSTALLED)
    firmware = build_firmware(INSTALLED)
    cats, parts = build_items(INSTALLED, names, uistats, firmware)

    # what mods add: compare with the vanilla build
    if (VANILLA / "json").exists():
        vcats, vparts = build_items(VANILLA, build_names(VANILLA), {}, {})
        for k, p in parts.items():
            if k not in vparts or vparts[k]["k"] != p["k"]:
                p["mod"] = True
        for cid, c in cats.items():
            if cid not in vcats:
                c["mod"] = True
    srcs = json.load(open(INSTALLED / "sources.json"))
    mod_paks = sorted({v["pak"] for v in srcs.values() if v.get("mod")})

    # catalogs seeded by the save-fields research (same game build)
    seed = {}
    for name in ("cosmetics_catalogue.json", "missions_catalogue.json", "progress_graphs.json"):
        f = SEED / name
        if f.exists():
            seed[name.split(".")[0]] = json.load(open(f, encoding="utf-8"))
    sdu = []
    f = SEED / "sdu_nodes.tsv"
    if f.exists():
        for line in open(f, encoding="utf-8"):
            if line.startswith("#") or not line.strip():
                continue
            cols = line.rstrip("\n").split("\t")
            if len(cols) >= 2 and cols[1].isdigit():
                sdu.append({"name": cols[0], "cost": int(cols[1]), "requires": cols[2] if len(cols) > 2 and cols[2] != "{}" else None,
                            "effect": cols[3] if len(cols) > 3 else ""})

    containers = {}
    for _, _, e in entries(INSTALLED, "inventory_container"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        for si in as_list(v.get("slotinfo")):
            if isinstance(si, dict) and si.get("numslots"):
                containers[e["key"].lower()] = int(si["numslots"])
    db = {
        "meta": {
            "sources": {k: v["pak"] for k, v in srcs.items() if k.split("_c")[0].lower() in ("inv", "inv_name_part", "ui_stat")},
            "mod_paks": mod_paks,
        },
        "manufacturers": mfrs,
        "firmware": firmware,
        "names": {k: v["t"] for k, v in names.items()},
        "categories": [cats[k] for k in sorted(cats)],
        "parts": [parts[k] for k in sorted(parts, key=lambda s: tuple(map(int, s.split(":"))))],
        "cosmetics": seed.get("cosmetics_catalogue", {}),
        "missions": seed.get("missions_catalogue", {}),
        "progress_graphs": seed.get("progress_graphs", {}),
        "sdu": sdu,
        "tables": build_tables(INSTALLED),
        "attributes": build_attributes(INSTALLED),
        "aspects": build_aspect_defs(INSTALLED),
        "bases": build_bases(INSTALLED),
        "containers": containers,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    raw = json.dumps(db, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    with gzip.open(OUT, "wb", compresslevel=9) as f:
        f.write(raw)
    nmod = sum(1 for p in parts.values() if p.get("mod"))
    kinds = defaultdict(int)
    for c in cats.values():
        kinds[c["kind"]] += 1
    print(f"{len(cats)} categories {dict(kinds)}; {len(parts)} parts ({nmod} from mods {mod_paks}); "
          f"{len(names)} names; {len(uistats)} ui stats; json {len(raw)/1e6:.1f} MB -> {OUT.stat().st_size/1e6:.2f} MB gz")


if __name__ == "__main__":
    main()
