//! The game database, generated from the installed game's NCS tables by
//! `tools/build_db.py` and embedded in the executable (gzip JSON). A newer
//! `bl4db.json.gz` placed next to the executable overrides the embedded one,
//! so the editor can follow game patches without a rebuild.

use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;

use crate::serial::PartRef;

static EMBEDDED: &[u8] = include_bytes!("../../../data/bl4db.json.gz");

#[derive(Debug, Deserialize, Default)]
pub struct Manufacturer {
    pub name: String,
    #[serde(default)]
    pub desc: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct SlotRule {
    #[serde(default)]
    pub min: Option<u32>,
    #[serde(default)]
    pub max: Option<u32>,
    #[serde(default)]
    pub parts: Vec<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct TagRule {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub min: Option<u32>,
    #[serde(default)]
    pub max: Option<u32>,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Comp {
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub basetags: Vec<String>,
    #[serde(default)]
    pub slots: HashMap<String, SlotRule>,
    #[serde(default)]
    pub tagrules: Vec<TagRule>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Category {
    pub id: u32,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mfr: Option<String>,
    #[serde(default)]
    pub parttypes: Vec<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub comps: HashMap<String, Comp>,
    #[serde(default)]
    pub asp: Option<serde_json::Value>,
    #[serde(default)]
    pub r#mod: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Passive {
    #[serde(default)]
    pub graph: Option<String>,
    #[serde(default)]
    pub node: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Part {
    pub c: u32,
    pub i: u32,
    pub k: String,
    pub s: String,
    #[serde(default)]
    pub st: String,
    #[serde(default)]
    pub add: Vec<String>,
    #[serde(default)]
    pub dep: Vec<String>,
    #[serde(default)]
    pub excl: Vec<String>,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub mgs: Option<String>,
    #[serde(default)]
    pub noglobal: bool,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub text: Vec<String>,
    #[serde(default)]
    pub fw: Option<String>,
    #[serde(default)]
    pub passive: Vec<Passive>,
    #[serde(default)]
    pub points: Option<u32>,
    #[serde(default)]
    pub fx: Option<serde_json::Value>,
    #[serde(default)]
    pub tpl: Option<serde_json::Value>,
    #[serde(default)]
    pub mods: Option<serde_json::Value>,
    #[serde(default)]
    pub beh: Option<serde_json::Value>,
    #[serde(default)]
    pub asp: Option<serde_json::Value>,
    #[serde(default)]
    pub r#mod: bool,
}

pub type Table = HashMap<String, HashMap<String, serde_json::Value>>;

#[derive(Debug, Deserialize, Clone)]
pub struct Cosmetic {
    pub group: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub part: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MissionInfo {
    #[serde(default)]
    pub set: String,
    #[serde(default)]
    pub r#type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub n_objectives: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SduNode {
    pub name: String,
    pub cost: u32,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub effect: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct Meta {
    #[serde(default)]
    pub mod_paks: Vec<String>,
    #[serde(default)]
    pub sources: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct RawDb {
    #[serde(default)]
    meta: Meta,
    #[serde(default)]
    manufacturers: HashMap<String, Manufacturer>,
    #[serde(default)]
    names: HashMap<String, String>,
    categories: Vec<Category>,
    parts: Vec<Part>,
    #[serde(default)]
    cosmetics: HashMap<String, Cosmetic>,
    #[serde(default)]
    missions: HashMap<String, MissionInfo>,
    #[serde(default)]
    progress_graphs: HashMap<String, serde_json::Value>,
    #[serde(default)]
    sdu: Vec<SduNode>,
    #[serde(default)]
    tables: HashMap<String, Table>,
    #[serde(default)]
    attributes: HashMap<String, serde_json::Value>,
    #[serde(default)]
    aspects: HashMap<String, serde_json::Value>,
    #[serde(default)]
    bases: HashMap<String, serde_json::Value>,
    #[serde(default)]
    containers: HashMap<String, u32>,
}

pub struct Db {
    pub meta: Meta,
    pub manufacturers: HashMap<String, Manufacturer>,
    pub names: HashMap<String, String>,
    pub categories: HashMap<u32, Category>,
    pub parts: HashMap<PartRef, Part>,
    /// parts of each category, by key (lower case)
    pub part_by_key: HashMap<(u32, String), PartRef>,
    pub cat_parts: HashMap<u32, Vec<PartRef>>,
    pub cosmetics: HashMap<String, Cosmetic>,
    pub missions: HashMap<String, MissionInfo>,
    pub progress_graphs: HashMap<String, serde_json::Value>,
    pub sdu: Vec<SduNode>,
    /// game data tables: table -> row -> column -> value
    pub tables: HashMap<String, Table>,
    /// attribute definitions (balance formulas) by name
    pub attributes: HashMap<String, serde_json::Value>,
    /// inventory aspect templates by name
    pub aspects: HashMap<String, serde_json::Value>,
    /// aspects of item types and their base types
    pub bases: HashMap<String, serde_json::Value>,
    /// base capacities (backpack, bank)
    pub containers: HashMap<String, u32>,
    /// where the data came from (embedded or a file path)
    pub origin: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

impl Db {
    pub fn from_gz(bytes: &[u8], origin: &str) -> Result<Db, DbError> {
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes).read_to_end(&mut raw)?;
        let r: RawDb = serde_json::from_slice(&raw)?;
        let mut parts = HashMap::new();
        let mut part_by_key = HashMap::new();
        let mut cat_parts: HashMap<u32, Vec<PartRef>> = HashMap::new();
        for p in r.parts {
            let pr = PartRef { cat: p.c, idx: p.i };
            part_by_key.insert((p.c, p.k.clone()), pr);
            cat_parts.entry(p.c).or_default().push(pr);
            parts.insert(pr, p);
        }
        for v in cat_parts.values_mut() {
            v.sort();
        }
        Ok(Db {
            meta: r.meta,
            manufacturers: r.manufacturers,
            names: r.names,
            categories: r.categories.into_iter().map(|c| (c.id, c)).collect(),
            parts,
            part_by_key,
            cat_parts,
            cosmetics: r.cosmetics,
            missions: r.missions,
            progress_graphs: r.progress_graphs,
            sdu: r.sdu,
            tables: r.tables,
            attributes: r.attributes,
            aspects: r.aspects,
            bases: r.bases,
            containers: r.containers,
            origin: origin.to_string(),
        })
    }

    /// The database next to the executable if present, else the embedded one.
    pub fn load() -> Db {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let p = dir.join("bl4db.json.gz");
                if let Ok(bytes) = std::fs::read(&p) {
                    if let Ok(db) = Db::from_gz(&bytes, &p.display().to_string()) {
                        return db;
                    }
                }
            }
        }
        Db::embedded()
    }

    pub fn embedded() -> Db {
        Db::from_gz(EMBEDDED, "embedded").expect("embedded database is valid")
    }

    pub fn part(&self, r: PartRef) -> Option<&Part> {
        self.parts.get(&r)
    }

    pub fn category(&self, id: u32) -> Option<&Category> {
        self.categories.get(&id)
    }

    pub fn manufacturer_name(&self, key: &str) -> String {
        self.manufacturers
            .get(&key.to_lowercase())
            .map(|m| m.name.clone())
            .unwrap_or_else(|| key.to_string())
    }

    /// Item categories a user can create (weapons and gear), sorted by kind and name.
    pub fn item_categories(&self) -> Vec<&Category> {
        let mut v: Vec<&Category> = self
            .categories
            .values()
            .filter(|c| {
                matches!(c.kind.as_str(), "weapon" | "heavy" | "grenade" | "shield" | "repkit" | "enhancement" | "classmod")
            })
            .collect();
        v.sort_by(|a, b| (kind_order(&a.kind), a.name.clone()).cmp(&(kind_order(&b.kind), b.name.clone())));
        v
    }
}

pub fn kind_order(k: &str) -> u8 {
    match k {
        "weapon" => 0,
        "heavy" => 1,
        "shield" => 2,
        "grenade" => 3,
        "repkit" => 4,
        "enhancement" => 5,
        "classmod" => 6,
        _ => 9,
    }
}

pub fn kind_label(k: &str) -> &'static str {
    match k {
        "weapon" => "Weapon",
        "heavy" => "Heavy Weapon",
        "shield" => "Shield",
        "grenade" => "Ordnance",
        "repkit" => "Repkit",
        "enhancement" => "Enhancement",
        "classmod" => "Class Mod",
        "pool" => "Shared pool",
        _ => "Other",
    }
}

/// Slots a kind takes from a shared pool (measured on game-generated items).
pub fn pool_slots(kind: &str, pool: u32) -> Option<&'static [&'static str]> {
    Some(match (kind, pool) {
        ("weapon", 1) | ("heavy", 1) => &["body_ele", "secondary_ele", "pearl_elem", "pearl_stat"],
        (_, 234) => &["stat_group1", "stat_group2", "stat_group3", "firmware", "special_passive"],
        (_, 243) | (_, 245) | (_, 246) | (_, 237) | (_, 248) | (_, 247) | (_, 244) => return None,
        _ => return None,
    })
}

/// Slots that hold several parts on a normal item; every other slot holds one.
pub fn multi_slot(kind: &str, slot: &str) -> bool {
    matches!(slot, "barrel_acc" | "body_acc" | "scope_acc" | "passive_points" | "core_augment" | "stat_augment")
        || (kind == "classmod" && slot == "stat_group1")
}

/// Shared part pools an item kind may reference with `{cat:n}` tokens.
pub fn allowed_pools(kind: &str) -> &'static [u32] {
    match kind {
        "weapon" => &[1],
        "heavy" => &[244, 245, 1],
        "grenade" => &[245],
        "shield" => &[246, 237, 248],
        "repkit" => &[243],
        "enhancement" => &[247],
        "classmod" => &[234],
        _ => &[],
    }
}
