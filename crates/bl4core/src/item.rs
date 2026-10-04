//! Item analysis: what a serial is (name, type, rarity, element, parts) and
//! whether the game would accept it.

use crate::db::{allowed_pools, multi_slot, pool_slots, Category, Db, Part};
use crate::serial::{PartRef, Serial};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Unusual but loads fine (e.g. could not drop this way)
    Info,
    /// Not obtainable in normal play; the game still loads it
    Warning,
    /// The game will reject or strip this item
    Error,
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub sev: Severity,
    pub msg: String,
    /// index into `part_refs()` the issue is about, if any
    pub part: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct PartInfo {
    pub r: PartRef,
    pub key: String,
    pub slot: String,
    /// human label: title / firmware name / description
    pub label: String,
    pub text: Vec<String>,
    pub known: bool,
    pub from_mod: bool,
}

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub category: u32,
    pub kind: String,
    pub type_name: String,
    pub manufacturer: String,
    pub name: String,
    pub rarity: Rarity,
    pub element: Vec<String>,
    pub level: Option<u32>,
    pub parts: Vec<PartInfo>,
    pub issues: Vec<Issue>,
    /// item-level effect lines (legendary red text, manufacturer perks)
    pub text: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rarity {
    Unknown,
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Pearlescent,
}

impl Rarity {
    pub fn label(self) -> &'static str {
        match self {
            Rarity::Unknown => "?",
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Pearlescent => "Pearlescent",
        }
    }
    pub fn rgb(self) -> [u8; 3] {
        match self {
            Rarity::Unknown => [160, 160, 160],
            Rarity::Common => [200, 200, 200],
            Rarity::Uncommon => [76, 200, 76],
            Rarity::Rare => [64, 150, 255],
            Rarity::Epic => [180, 90, 255],
            Rarity::Legendary => [255, 160, 30],
            Rarity::Pearlescent => [100, 235, 230],
        }
    }
    fn from_comp(key: &str) -> Rarity {
        let k = key.trim_start_matches("base_");
        if k.starts_with("comp_01") {
            Rarity::Common
        } else if k.starts_with("comp_02") {
            Rarity::Uncommon
        } else if k.starts_with("comp_03") {
            Rarity::Rare
        } else if k.starts_with("comp_04") {
            Rarity::Epic
        } else if k.starts_with("comp_05") {
            Rarity::Legendary
        } else if k.starts_with("comp_06") || k.contains("pearl") {
            Rarity::Pearlescent
        } else {
            Rarity::Unknown
        }
    }
}

pub const MAX_LEVEL: u32 = 60;

const ELEMENTS: &[(&str, &str)] = &[
    ("fire", "Incendiary"),
    ("shock", "Shock"),
    ("corrosive", "Corrosive"),
    ("cryo", "Cryo"),
    ("radiation", "Radiation"),
    ("kinetic", "Kinetic"),
];

/// Strip the game's rich-text markup (`[secondary]..[/secondary]`).
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '[' => in_tag = true,
            ']' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn part_label(db: &Db, p: &Part) -> String {
    if let Some(fw) = &p.fw {
        let n = fw.trim_start_matches("fw_").replace('_', " ");
        return format!("Firmware: {}", title_case(&n));
    }
    if let Some(t) = &p.title {
        return t.clone();
    }
    if let Some(t) = &p.prefix {
        return t.clone();
    }
    if let Some(first) = p.text.first() {
        let pl = plain(first);
        if let Some((head, _)) = pl.split_once(" - ") {
            if head.len() < 40 {
                return head.to_string();
            }
        }
    }
    if let Some(e) = element_of(&p.k) {
        return e.to_string();
    }
    if let Some(pas) = p.passive.first() {
        if let Some(n) = &pas.node {
            return format!("{} ({} pt)", n, p.points.unwrap_or(1));
        }
    }
    if let Some(d) = &p.desc {
        return d.clone();
    }
    let _ = db;
    p.k.clone()
}

fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn element_of(key: &str) -> Option<&'static str> {
    let k = key.to_lowercase();
    if k.starts_with("part_secondary_elem") {
        return None;
    }
    for (needle, label) in ELEMENTS {
        if k == format!("part_{needle}") || k.ends_with(&format!("_{needle}")) && k.starts_with("part_elem") {
            return Some(label);
        }
    }
    None
}

pub fn analyze(db: &Db, s: &Serial) -> ItemInfo {
    let cat_id = s.category();
    let cat: Option<&Category> = db.category(cat_id);
    let refs = s.part_refs();
    let mut issues = Vec::new();
    let mut parts = Vec::new();
    let kind = cat.map(|c| c.kind.clone()).unwrap_or_else(|| "unknown".into());

    if cat.is_none() {
        issues.push(Issue { sev: Severity::Error, msg: format!("Unknown item category {cat_id}"), part: None });
    }

    let pools: HashSet<u32> = allowed_pools(&kind).iter().copied().collect();
    let mut resolved: Vec<Option<&Part>> = Vec::new();
    for (n, (_, r)) in refs.iter().enumerate() {
        let p = db.part(*r);
        resolved.push(p);
        match p {
            Some(p) => {
                if r.cat != cat_id && !pools.contains(&r.cat) {
                    issues.push(Issue {
                        sev: Severity::Error,
                        msg: format!("Part {} comes from category {} which this item type cannot use", p.k, r.cat),
                        part: Some(n),
                    });
                }
                if r.cat != cat_id && pools.contains(&r.cat) {
                    if let Some(slots) = pool_slots(&kind, r.cat) {
                        if !slots.contains(&p.s.as_str()) {
                            issues.push(Issue {
                                sev: Severity::Warning,
                                msg: format!("{} ({}) is not a slot this item type takes from the shared pool", p.k, p.s),
                                part: Some(n),
                            });
                        }
                    }
                    if p.s == "inv_comp" {
                        issues.push(Issue { sev: Severity::Warning, msg: format!("{} is a rarity component of another pool", p.k), part: Some(n) });
                    }
                }
                if p.st != "Active" && !p.st.is_empty() {
                    issues.push(Issue {
                        sev: Severity::Warning,
                        msg: format!("Part {} is marked {} in the game data", p.k, p.st),
                        part: Some(n),
                    });
                }
                if p.r#mod {
                    issues.push(Issue {
                        sev: Severity::Warning,
                        msg: format!(
                            "Part {} only exists with a mod installed ({}); without it the game hides the item",
                            p.k,
                            db.meta.mod_paks.join(", ")
                        ),
                        part: Some(n),
                    });
                }
                parts.push(PartInfo {
                    r: *r,
                    key: p.k.clone(),
                    slot: p.s.clone(),
                    label: part_label(db, p),
                    text: p.text.clone(),
                    known: true,
                    from_mod: p.r#mod,
                });
            }
            None => {
                issues.push(Issue {
                    sev: Severity::Error,
                    msg: format!(
                        "Unknown part {{{}:{}}} - the game will park this item in unknown_items (it disappears)",
                        r.cat, r.idx
                    ),
                    part: Some(n),
                });
                parts.push(PartInfo {
                    r: *r,
                    key: format!("[{}:{}]", r.cat, r.idx),
                    slot: "?".into(),
                    label: "unknown part".into(),
                    text: vec![],
                    known: false,
                    from_mod: false,
                });
            }
        }
    }

    // rarity comp
    let comps: Vec<(usize, &Part)> = resolved
        .iter()
        .enumerate()
        .filter_map(|(n, p)| p.filter(|p| p.s == "inv_comp" && p.c == cat_id).map(|p| (n, p)))
        .collect();
    let rarity = comps.first().map(|(_, p)| Rarity::from_comp(&p.k)).unwrap_or(Rarity::Unknown);
    if cat.is_some() && comps.is_empty() && kind != "other" {
        issues.push(Issue { sev: Severity::Warning, msg: "No rarity component".into(), part: None });
    }
    if comps.len() > 1 {
        issues.push(Issue {
            sev: Severity::Warning,
            msg: format!("{} rarity components (normally exactly one)", comps.len()),
            part: Some(comps[1].0),
        });
    }

    // level
    let level = s.level();
    match level {
        Some(l) if l == 0 || l > MAX_LEVEL => issues.push(Issue {
            sev: Severity::Error,
            msg: format!("Level {l} is outside 1..{MAX_LEVEL}"),
            part: None,
        }),
        None => issues.push(Issue { sev: Severity::Info, msg: "Item header has no standard level field".into(), part: None }),
        _ => {}
    }

    // composition rules of the rarity comp
    if let (Some(c), Some((_, comp_part))) = (cat, comps.first()) {
        check_composition(db, c, comp_part, &resolved, &mut issues);
    }
    check_tags(&resolved, &mut issues);
    // one part per slot, except accessory-type slots
    let mut per_slot: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (n, p) in resolved.iter().enumerate() {
        if let Some(p) = p {
            if p.s != "inv_comp" {
                per_slot.entry(p.s.as_str()).or_default().push(n);
            }
        }
    }
    for (slot, idxs) in per_slot {
        if idxs.len() > 1 && !multi_slot(&kind, slot) {
            issues.push(Issue {
                sev: Severity::Warning,
                msg: format!("{} {slot} parts (normal items have one)", idxs.len()),
                part: Some(idxs[1]),
            });
        }
    }

    // naming
    let type_name = cat.and_then(|c| c.name.clone()).unwrap_or_else(|| format!("Category {cat_id}"));
    let manufacturer = cat.and_then(|c| c.mfr.clone()).map(|m| db.manufacturer_name(&m)).unwrap_or_default();
    let mut best_title: Option<(&str, i32)> = None;
    let mut prefix: Option<&str> = None;
    for p in resolved.iter().flatten() {
        if let Some(t) = &p.title {
            // legendary/unique names outrank barrel names
            let pri = if p.s == "inv_comp" || p.k.contains("legendary") || p.k.contains("unique") { 3 } else { 1 };
            if best_title.map(|b| pri > b.1).unwrap_or(true) {
                best_title = Some((t, pri));
            }
        }
        if prefix.is_none() {
            if let Some(t) = &p.prefix {
                prefix = Some(t);
            }
        }
    }
    let name = match (prefix, best_title) {
        (Some(p), Some((t, _))) if kind != "weapon" => format!("{p} {t}"),
        (_, Some((t, _))) => t.to_string(),
        (Some(p), None) => format!("{p} {type_name}"),
        _ => type_name.clone(),
    };
    let element: Vec<String> = resolved
        .iter()
        .flatten()
        .filter_map(|p| element_of(&p.k).map(|s| s.to_string()))
        .collect();
    let mut text = vec![];
    if let Some(c) = cat {
        if let Some(ui) = c.asp.as_ref().and_then(|a| a.get("ui")).and_then(|u| u.as_array()) {
            let _ = ui; // manufacturer perk lines are resolved by the build script into names only
        }
    }
    for p in resolved.iter().flatten() {
        for t in &p.text {
            if !text.contains(t) {
                text.push(t.clone());
            }
        }
    }
    issues.sort_by(|a, b| b.sev.cmp(&a.sev));
    ItemInfo {
        category: cat_id,
        kind,
        type_name,
        manufacturer,
        name,
        rarity,
        element,
        level,
        parts,
        issues,
        text,
    }
}

fn check_composition(db: &Db, cat: &Category, comp: &Part, resolved: &[Option<&Part>], issues: &mut Vec<Issue>) {
    let Some(rules) = cat.comps.get(&comp.k) else { return };
    // inherited rules from the base composition (e.g. Weapon.base_comp_05_legendary)
    let mut slots = rules.slots.clone();
    if let Some(base) = &rules.base {
        if let Some((tcat, tcomp)) = base.split_once('.') {
            let tcat = tcat.to_lowercase();
            if let Some(bc) = db.categories.values().find(|c| c.key == tcat) {
                if let Some(br) = bc.comps.get(&tcomp.to_lowercase()) {
                    for (k, v) in &br.slots {
                        slots.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
            }
        }
    }
    let own: Vec<(usize, &Part)> = resolved
        .iter()
        .enumerate()
        .filter_map(|(n, p)| p.filter(|p| p.c == cat.id && p.s != "inv_comp").map(|p| (n, p)))
        .collect();
    let mut count: BTreeMap<&str, u32> = BTreeMap::new();
    for (_, p) in &own {
        *count.entry(p.s.as_str()).or_default() += 1;
    }
    for (slot, rule) in &slots {
        let n = count.get(slot.as_str()).copied().unwrap_or(0);
        if !rule.parts.is_empty() {
            for (idx, p) in own.iter().filter(|(_, p)| &p.s == slot) {
                if !rule.parts.iter().any(|x| x.eq_ignore_ascii_case(&p.k)) {
                    issues.push(Issue {
                        sev: Severity::Warning,
                        msg: format!("{} is not one of the {} parts this rarity can roll", p.k, slot),
                        part: Some(*idx),
                    });
                }
            }
        }
        if let Some(max) = rule.max {
            if n > max {
                issues.push(Issue { sev: Severity::Warning, msg: format!("{n} {slot} parts (this rarity rolls at most {max})"), part: None });
            }
        }
        if let Some(min) = rule.min {
            if n < min && n > 0 {
                issues.push(Issue { sev: Severity::Info, msg: format!("{n} {slot} parts (this rarity rolls at least {min})"), part: None });
            }
        }
    }
}

fn check_tags(resolved: &[Option<&Part>], issues: &mut Vec<Issue>) {
    // Exclusions are applied in roll order: a part may not be added when an
    // earlier part already added one of its exclusion tags.
    let mut seen: HashSet<&str> = HashSet::new();
    for (n, p) in resolved.iter().enumerate() {
        let Some(p) = p else { continue };
        for t in &p.excl {
            if seen.contains(t.as_str()) {
                issues.push(Issue {
                    sev: Severity::Warning,
                    msg: format!("{} conflicts with an earlier part (exclusion tag '{t}')", p.k),
                    part: Some(n),
                });
                break;
            }
        }
        for t in &p.add {
            seen.insert(t);
        }
    }
    let all: HashSet<&str> = resolved.iter().flatten().flat_map(|p| p.add.iter().map(|s| s.as_str())).collect();
    for (n, p) in resolved.iter().enumerate() {
        let Some(p) = p else { continue };
        if !p.dep.is_empty() && !p.dep.iter().any(|d| all.contains(d.as_str())) {
            issues.push(Issue {
                sev: Severity::Warning,
                msg: format!("{} needs a part providing {}", p.k, p.dep.join(" or ")),
                part: Some(n),
            });
        }
    }
}

/// Build a new item of category `cat` with rarity component `comp` from
/// scratch: the comp's listed parts first, then one part for each slot the
/// comp requires. Returns the serial; validate it with `analyze`.
pub fn build_item(db: &Db, cat: u32, comp: &str, level: u32, seed: u64) -> Option<Serial> {
    use crate::serial::Tok;
    let c = db.category(cat)?;
    let weapon = c.kind == "weapon";
    let mut toks = vec![
        if weapon { Tok::Int(cat as u64) } else { Tok::bit(cat as u64) },
        Tok::Soft,
        Tok::Int(0),
        Tok::Soft,
        Tok::Int(1),
        Tok::Soft,
        Tok::Int(level as u64),
        Tok::Sep,
        Tok::Int(2),
        Tok::Soft,
        Tok::Int(seed),
        Tok::Sep,
        Tok::Sep,
    ];
    toks.push(Tok::Sep);
    toks.push(Tok::Sep);
    let mut s = Serial { toks, byte_len: 0 };
    let comp_ref = *db.part_by_key.get(&(cat, comp.to_lowercase()))?;
    s.add_part(comp_ref);
    let rules = c.comps.get(&comp.to_lowercase()).cloned().unwrap_or_default();
    let parts = db.cat_parts.get(&cat).cloned().unwrap_or_default();
    // slots with explicit part lists or a required count
    let mut slots: Vec<String> = c.parttypes.clone();
    for k in rules.slots.keys() {
        if !slots.contains(k) {
            slots.push(k.clone());
        }
    }
    const DEFAULT_ONE: &[&str] = &["body", "barrel", "magazine", "grip", "scope", "class_mod_body", "payload", "primary_augment"];
    for slot in &slots {
        let rule = rules.slots.get(slot);
        let want = rule
            .and_then(|r| r.min)
            .unwrap_or(if DEFAULT_ONE.contains(&slot.as_str()) || rule.map(|r| !r.parts.is_empty()).unwrap_or(false) { 1 } else { 0 });
        let mut picked = 0;
        let listed: Vec<&String> = rule.map(|r| r.parts.iter().collect()).unwrap_or_default();
        let candidates: Vec<PartRef> = if !listed.is_empty() {
            listed.iter().filter_map(|k| db.part_by_key.get(&(cat, k.to_lowercase())).copied()).collect()
        } else {
            parts
                .iter()
                .copied()
                .filter(|r| db.part(*r).map(|p| &p.s == slot && p.st == "Active" && !p.k.contains("unique") && !p.k.contains("legendary")).unwrap_or(false))
                .collect()
        };
        for r in candidates {
            if picked >= want {
                break;
            }
            s.add_part(r);
            picked += 1;
        }
    }
    let enc = s.encode();
    Serial::decode(&enc).ok()
}
