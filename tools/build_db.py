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


UI_ARGS = {}


def build_uistats(d):
    """ui_stat key -> display text. Placeholder arguments ({mod}, {damage}, ...)
    are recorded in UI_ARGS[text] so the editor can fill in numbers."""
    ui = {}
    for _, _, e in entries(d, "ui_stat"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        sv = v.get("statvalue") or v.get("statlabel") or {}
        t = text(sv.get("formattext")) if isinstance(sv, dict) else None
        if t and ref(v.get("displaygroup") or "") == "RedText":
            t = f"[redtext]{t}[/redtext]"
        if t:
            ui[e["key"].lower()] = t
            args = []
            am = sv.get("argsmap") if isinstance(sv, dict) else None
            for pair in ((am or {}).get("pairs") or {}).values():
                if not isinstance(pair, dict):
                    continue
                val = pair.get("value") or {}
                if not isinstance(val, dict):
                    continue
                a = {"k": pair.get("key")}
                if val.get("attributedef"):
                    a["a"] = ref(val["attributedef"]).lower()
                if str(val.get("bdisplayaspercentage", "")).lower() == "true":
                    a["pct"] = True
                if str(val.get("bdisplayplussign", "")).lower() == "true":
                    a["plus"] = True
                if str(val.get("buseformattext", "")).lower() == "true" and val.get("formattext"):
                    a["fmt"] = text(val["formattext"])
                if val.get("constant") is not None:
                    a["c"] = val.get("constant")
                args.append(a)
            if args:
                UI_ARGS[t] = args
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
            # an omitted modifiertype is the enum default: EGbxAttributeModifierType
            # is {ScaleAdd, PreAdd, PostAdd, ScaleMultiply, OverrideBaseValue}
            # (Borderlands4.exe), so 0 = ScaleAdd, i.e. x (1 + sum)
            x = {"a": ref(ef.get("attributetomodify")), "op": ef.get("modifiertype") or "ScaleAdd"}
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
                if a.get("usemodebitmask"):
                    x["mode"] = a["usemodebitmask"]
                mods.append(x)
        b = a.get("behavior")
        if isinstance(b, dict) and parent and "fire" in parent.lower():
            for k in ("damage", "firerate", "spread", "projectilespershot", "automaticburstcount", "accuracyimpulse",
                      "burstfiredelay", "shotammocost"):
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


def build_skill_names(d):
    """(graph, node key) -> skill display alias. Class skill graphs inherit
    node keys ("Trunk - Row 2 - 1") from template graphs and name them by
    pair id."""
    graphs = {}
    for _, _, e in entries(d, "progress_graph"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        g = graphs.setdefault(e["key"].lower(), {"parent": None, "pairs": {}})
        if v.get("parent"):
            g["parent"] = ref(v["parent"]).lower()
        nodes = v.get("nodes") if isinstance(v.get("nodes"), dict) else {}
        for pid, pair in (nodes.get("pairs") or {}).items():
            if not isinstance(pair, dict):
                continue
            rec = g["pairs"].setdefault(pid, {})
            if pair.get("key"):
                rec["key"] = pair["key"]
            val = pair.get("value") or {}
            if isinstance(val, dict) and val.get("alias"):
                rec["alias"] = val["alias"]
    out = {}
    for gname, g in graphs.items():
        keys, aliases = {}, {}
        cur, seen = gname, set()
        while cur and cur in graphs and cur not in seen:
            seen.add(cur)
            for pid, rec in graphs[cur]["pairs"].items():
                if "key" in rec:
                    keys.setdefault(pid, rec["key"])
                if "alias" in rec:
                    aliases.setdefault(pid, rec["alias"])
            cur = graphs[cur]["parent"]
        m = {keys[pid]: aliases[pid] for pid in keys if pid in aliases}
        if m:
            out[gname] = m
    return out


def build_items(d, names, uistats, firmware, skills=None):
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
                lines = [uistats[u] for u in ui if u in uistats]
                if lines:
                    c["text"] = lines
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
                    if skills:
                        sk = [skills.get((x["graph"] or "").lower(), {}).get(x["node"]) for x in p["passive"]]
                        sk = [x for x in sk if x]
                        if sk:
                            p["title"] = " / ".join(sk)
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


STRUCT_DEFAULTS = Path(__file__).resolve().parent / "struct_defaults.json"


def build_tables(d):
    """All gbx_ue_data_tables as {table: {row: {column: value}}} (lower case,
    GUID suffixes stripped from column names).

    NCS stores only the cells that differ from the row struct's default value
    (of the 334 struct-typed tables that omit cells, 328 never hold a cell
    equal to its default), so every table whose row struct is known also gets
    a `__default__` row with the struct's defaults (struct_defaults.json, read
    from the cooked Struct_*.uasset UserDefinedStructs), e.g.
    Struct_Weapon_Barrel_Init.AccImpulse_Value = 0.2."""
    sdef = json.load(open(STRUCT_DEFAULTS, encoding="utf-8")) if STRUCT_DEFAULTS.exists() else {}
    out = {}
    for _, _, e in entries(d, "gbx_ue_data_table"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        rows = {}
        for r in v.get("data") or []:
            if not isinstance(r, dict) or not r.get("row_name"):
                continue
            cols = {}
            for k, val in (r.get("row_value") or {}).items():
                # nested struct cells ({"dps": {"value": "0.2"}}) -> their value
                if isinstance(val, dict) and isinstance(val.get("value"), str):
                    val = val["value"]
                if isinstance(val, str):
                    try:
                        cols[norm_col(k)] = float(val)
                    except ValueError:
                        cols[norm_col(k)] = val
            rows[r["row_name"].lower()] = cols
        struct = (v.get("row_struct") or "").split(".")[-1].rstrip("'").lower()
        if struct in sdef:
            rows["__default__"] = sdef[struct]
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


def deep_merge(dst, src):
    for k, v in src.items():
        if isinstance(v, dict) and isinstance(dst.get(k), dict):
            deep_merge(dst[k], v)
        else:
            dst[k] = v
    return dst


def build_stat_defs(d):
    """inv_stat definitions (WeaponStatsDef / GadgetStatsDef), parent chain merged.

    Each part's `statmodifiers` (stattagname + points: the "Damage mod",
    "Reload speed mod", ... parts) is turned into attribute modifiers by the
    item type's inv_stat: for every entry whose `stat` is the tag,
      value = points * StatToAttributeModifierScalar * BaseMultiplier
              * Rarity_Balance[rarity].Stat_Scale   (negated if bZeroIsBetter)
    applied with `modifiertype` (default ScaleAdd) to `definition`.
    -> {inv_stat name: [{n, stat, a, op, sc, bm?, neg?, round?}]}"""
    raw = {}
    for _, _, e in entries(d, "inv_stat"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        attrs = {}
        for a in as_list((v.get("attributes") or {}).get("attribute")):
            if isinstance(a, dict):
                for n, body in a.items():
                    if isinstance(body, dict):
                        attrs[n.lower()] = body
        parent = ref(v.get("parent")).lower() if v.get("parent") else None
        raw[e["key"].lower()] = (parent, attrs)

    def merged(name, seen=()):
        if name not in raw or name in seen:
            return {}
        parent, attrs = raw[name]
        out = merged(parent, seen + (name,)) if parent else {}
        out = json.loads(json.dumps(out))
        for k, b in attrs.items():
            deep_merge(out.setdefault(k, {}), b)
        return out

    out = {}
    for name in raw:
        lst = []
        for n, b in merged(name).items():
            if not b.get("stat") or not b.get("definition"):
                continue
            x = {"n": n, "stat": b["stat"].lower(), "a": ref(b["definition"]).lower(),
                 "op": b.get("modifiertype") or "ScaleAdd"}
            sc = dt_ref(b.get("stattoattributemodifierscalar"))
            if not sc or "row" not in sc:
                continue
            x["sc"] = sc
            bm = dt_ref(b.get("basemultiplier"))
            if bm and "row" in bm and "col" in bm:
                x["bm"] = bm
            if str(b.get("bzeroisbetter", "")).lower() == "true":
                x["neg"] = True
            if b.get("roundingmode"):
                x["round"] = b["roundingmode"]
            lst.append(x)
        out[name] = lst
    return out


def build_bases(d):
    """Aspects of every inv entry by key, so stats can walk the base-type chain."""
    out = {}
    statdefs = build_stat_defs(d)
    for _, _, e in entries(d, "inv"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        if not v:
            continue
        a = compact_aspects(v)
        rec = out.setdefault(e["key"].lower(), {})
        if v.get("basetype"):
            rec["base"] = ref(v["basetype"]).lower()
        if v.get("stats"):
            # the item type's stat definition (jak_ar -> jakobs_weapon), resolved
            rec["stats"] = ref(v["stats"]).lower()
            rec["statdefs"] = statdefs.get(rec["stats"], [])
        for k in ("fx", "tpl", "beh", "mods"):
            if k in a:
                rec[k] = rec.get(k, []) + a[k] if isinstance(a[k], list) else a[k]
        # parent aspect names (template aspects without own data)
        pas = [ref(x.get("parent")).lower() for x in as_list(v.get("aspects")) if isinstance(x, dict) and x.get("parent")]
        if pas:
            rec["asp"] = sorted(set(rec.get("asp", []) + pas))
    return {k: v for k, v in out.items() if v}


def mission_type(key):
    k = key.lower()
    for pre, t in (("mission_main", "Main"), ("mission_side", "Side"), ("contract", "Contract"), ("micro", "Micro"),
                   ("mission_dlc", "DLC"), ("zoneactivity", "Activity"), ("mission_zoneactivity", "Activity")):
        if k.startswith(pre):
            return t
    if "_side_" in k:
        return "Side"
    return "Other"


def build_missions(d):
    out = {}
    for chunk, _, e in entries(d, "Mission"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        if not v:
            continue
        name = text((v.get("ux_display") or {}).get("text")) if isinstance(v.get("ux_display"), dict) else None
        nobj = 0
        for os_ in as_list(v.get("objective_sets")):
            nobj += len(json.dumps(os_).split('"objective":')) - 1
        out[e["key"].lower()] = {
            "set": (ref(v.get("missionset")) or "").lower() if v.get("missionset") else "",
            "type": mission_type(e["key"]),
            "name": name or "",
            "n_objectives": nobj,
            "src": f"Mission{chunk}",
        }
    return out


def build_cosmetics(d):
    out = {}
    for chunk, _, e in entries(d, "GbxActorPart"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        u = ref(v.get("unlockable"))
        if not u or "." not in u:
            continue
        out[u] = {"group": u.split(".")[0], "kind": "unlockable", "source": f"GbxActorPart{chunk}", "entry": e["key"],
                  "part": v.get("gbxactorpart"), "name": text(v.get("description"))}
    for chunk, _, e in entries(d, "inv_custom"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        u = ref(v.get("unlockable"))
        if not u or "." not in u:
            continue
        out[u] = {"group": u.split(".")[0], "kind": "weapon_skin", "source": f"inv_custom{chunk}", "entry": e["key"],
                  "part": v.get("inv_custom"), "name": text(v.get("displayname") or v.get("uiname"))}
    for chunk, _, e in entries(d, "GbxActor"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        u = ref(v.get("unlockedby"))
        if not u or "." not in u:
            continue
        out.setdefault(u, {"group": u.split(".")[0], "kind": "vehicle", "source": f"GbxActor{chunk}", "entry": e["key"],
                           "part": None, "name": u.split(".", 1)[1].replace("_", " ")})
    for chunk, _, e in entries(d, "hover_drive"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        for fld, suffix in (("unlockedby", ""), ("unlockedbycharacter", "_character")):
            u = ref(v.get(fld))
            if not u or "." not in u:
                continue
            g = u.split(".")[0] + suffix
            out.setdefault(u if not suffix else u + "#character", {"group": g, "kind": "hoverdrive", "source": f"hover_drive{chunk}",
                           "entry": e["key"], "part": v.get("hover_drive"), "name": u.split(".", 1)[1].replace("_", " ")})
    return out


def build_sdu(d):
    out = []
    for _, _, e in entries(d, "progress_graph"):
        if e["key"].lower() != "sdu_upgrades":
            continue
        v = e["value"] if isinstance(e["value"], dict) else {}
        nodes = (v.get("nodes") or {}).get("pairs") or {}
        for pair in nodes.values():
            if not isinstance(pair, dict):
                continue
            val = pair.get("value") or {}
            if not isinstance(val, dict) or not val.get("maxprogresspoints"):
                continue  # per-character slot rewards (no token cost)
            eff = []
            for m in as_list((val.get("itemdata") or {}).get("attributemodifiers")):
                if isinstance(m, dict):
                    eff.append(f"{ref(m.get('attribute'))} {m.get('modifiertype')} {(m.get('attributeinit') or {}).get('constant', '')}")
            cond = val.get("condition") or {}
            out.append({"name": pair.get("key"), "cost": int(float(val["maxprogresspoints"])),
                        "requires": cond.get("noderefname") if isinstance(cond, dict) else None, "effect": "; ".join(eff)})
    order = lambda n: (n["name"].rsplit("_", 1)[0], n["name"])
    return sorted(out, key=order)


def build_stations(d):
    """Checkpoint names `Map_P.Station` for fast-travel and respawn stations."""
    out = []
    for _, _, e in entries(d, "Map"):
        v = e["value"] if isinstance(e["value"], dict) else {}
        mapname = v.get("map") if isinstance(v.get("map"), str) else None
        for de in e.get("dep_entries", []):
            if de.get("dep_table_name") != "station":
                continue
            dv = de["value"] if isinstance(de["value"], dict) else {}
            ty = (ref(dv.get("typedef")) or "").lower()
            if ty not in ("fast_travel", "respawn"):
                continue
            st = dv.get("station") or de["key"]
            m = mapname
            if not m and isinstance(dv.get("dest"), str) and "." in ref(dv["dest"]):
                m = ref(dv["dest"]).split(".")[0]
            if not m:
                m = e["key"][:-2].title().replace(" ", "") + "_P" if e["key"].endswith("_p") else e["key"]
            out.append({"cp": f"{m}.{st}", "type": ty})
    uniq = {x["cp"]: x for x in out}
    return sorted(uniq.values(), key=lambda x: (x["type"] != "fast_travel", x["cp"]))


def main():
    names = build_names(INSTALLED)
    uistats = build_uistats(INSTALLED)
    mfrs = build_manufacturers(INSTALLED)
    firmware = build_firmware(INSTALLED)
    skills = build_skill_names(INSTALLED)
    cats, parts = build_items(INSTALLED, names, uistats, firmware, skills)

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
    sdu = build_sdu(INSTALLED)
    f = SEED / "sdu_nodes.tsv"
    if sdu:
        f = Path("/nonexistent")
    if f.exists():
        for line in open(f, encoding="utf-8"):
            if line.startswith("#") or not line.strip():
                continue
            cols = line.rstrip("\n").split("\t")
            if len(cols) >= 2 and cols[1].isdigit():
                sdu.append({"name": cols[0], "cost": int(cols[1]), "requires": cols[2] if len(cols) > 2 and cols[2] != "{}" else None,
                            "effect": cols[3] if len(cols) > 3 else ""})

    aspects = build_aspect_defs(INSTALLED)
    bases = build_bases(INSTALLED)
    for src in list(parts.values()) + list(bases.values()):
        for t in src.get("tpl") or []:
            a = (t.get("asp") or "").lower()
            if a in aspects:
                aspects[a]["template"] = True
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
        "cosmetics": build_cosmetics(INSTALLED) or seed.get("cosmetics_catalogue", {}),
        "missions": build_missions(INSTALLED) or seed.get("missions_catalogue", {}),
        "progress_graphs": seed.get("progress_graphs", {}),
        "sdu": sdu,
        "tables": build_tables(INSTALLED),
        "attributes": build_attributes(INSTALLED),
        "aspects": aspects,
        "bases": bases,
        "containers": containers,
        "uiargs": UI_ARGS,
        "stations": build_stations(INSTALLED),
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
