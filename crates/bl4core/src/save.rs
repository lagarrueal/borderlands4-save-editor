//! A save file on disk and every edit the editor performs on it.
//!
//! Paths and rules come from measuring game-written saves (see
//! research/save-fields.md): e.g. a level edit writes level and XP together,
//! equipped items keep their backpack twin in sync, the backpack keeps
//! contiguous slot numbers, and capacity counts only unequipped items.

use crate::crypto;
use crate::yaml::{self, Map, Node, Scalar, Seq};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Crypto(#[from] crypto::CryptoError),
    #[error("{0}")]
    Yaml(#[from] yaml::ParseError),
    #[error("not valid UTF-8")]
    Utf8,
    #[error("{0}")]
    Msg(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    Character,
    Profile,
}

pub struct SaveFile {
    pub path: PathBuf,
    pub steam_id: u64,
    pub kind: SaveKind,
    pub doc: Node,
    /// YAML text as loaded (to detect changes)
    pub original_yaml: String,
    pub backup: Option<PathBuf>,
}

impl SaveFile {
    /// Decrypt and parse. The Steam ID defaults to the one in the save path.
    pub fn open(path: &Path, steam_id: Option<u64>) -> Result<SaveFile, SaveError> {
        let data = std::fs::read(path)?;
        // candidates: explicit, from the path, then every Steam ID with BL4 saves on this PC
        let mut cands: Vec<u64> = steam_id.into_iter().chain(crypto::steam_id_from_path(path)).collect();
        for id in known_steam_ids() {
            if !cands.contains(&id) {
                cands.push(id);
            }
        }
        if cands.is_empty() {
            return Err(SaveError::Msg(
                "Steam ID unknown: the save is not inside a SaveGames\\<steamid> folder. Enter it in the Steam ID box.".into(),
            ));
        }
        let mut found = None;
        for sid in &cands {
            if let Ok(y) = crypto::decrypt(&data, *sid) {
                found = Some((*sid, y));
                break;
            }
        }
        let (sid, yaml_bytes) = found.ok_or_else(|| {
            SaveError::Msg("Cannot decrypt: wrong Steam ID (enter the Steam ID that owns this save) or not a BL4 save.".into())
        })?;
        let text = String::from_utf8(yaml_bytes).map_err(|_| SaveError::Utf8)?;
        let doc = yaml::parse(&text)?;
        let kind = if doc.get("domains").is_some() || path.file_stem().map(|s| s == "profile").unwrap_or(false) {
            SaveKind::Profile
        } else {
            SaveKind::Character
        };
        Ok(SaveFile { path: path.to_path_buf(), steam_id: sid, kind, doc, original_yaml: text, backup: None })
    }

    pub fn yaml(&self) -> String {
        self.doc.emit()
    }

    pub fn is_modified(&self) -> bool {
        self.yaml() != self.original_yaml
    }

    /// Copy the file as it is on disk into `<save dir>/bl4editor_backups/`.
    pub fn make_backup(&mut self) -> Result<PathBuf, SaveError> {
        let p = backup_file(&self.path)?;
        self.backup = Some(p.clone());
        Ok(p)
    }

    pub fn encrypted(&self) -> Result<Vec<u8>, SaveError> {
        Ok(crypto::encrypt(self.yaml().as_bytes(), self.steam_id)?)
    }

    /// Write atomically (temp file + rename). Verifies the bytes decrypt back
    /// to the same YAML before touching the real file.
    pub fn write(&mut self) -> Result<(), SaveError> {
        let text = self.yaml();
        let bytes = crypto::encrypt(text.as_bytes(), self.steam_id)?;
        let check = crypto::decrypt(&bytes, self.steam_id)?;
        if check != text.as_bytes() || yaml::parse(&text).is_err() {
            return Err(SaveError::Msg("internal check failed: refusing to write".into()));
        }
        let tmp = self.path.with_extension("sav.bl4editor-tmp");
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, &self.path)?;
        self.original_yaml = text;
        Ok(())
    }
}

pub fn backup_file(path: &Path) -> Result<PathBuf, SaveError> {
    let dir = path.parent().unwrap_or(Path::new(".")).join("bl4editor_backups");
    std::fs::create_dir_all(&dir)?;
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("save");
    let mut dst = dir.join(format!("{stem}_{}.sav", timestamp()));
    let mut n = 1;
    while dst.exists() {
        dst = dir.join(format!("{stem}_{}_{n}.sav", timestamp()));
        n += 1;
    }
    std::fs::copy(path, &dst)?;
    Ok(dst)
}

/// Local timestamp `YYYYMMDD-HHMMSS` for backup names.
pub fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// UTC timestamp (fallback, also used by tests).
pub fn timestamp_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}-{:02}{:02}{:02}", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// True while Borderlands4.exe runs. The game keeps the loaded character and
/// the profile in memory and overwrites edits on its next save.
pub fn game_running() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        if let Ok(out) = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq Borderlands4.exe", "/NH", "/FO", "CSV"])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            return String::from_utf8_lossy(&out.stdout).to_lowercase().contains("borderlands4.exe");
        }
        false
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Steam IDs that have Borderlands 4 saves on this PC.
pub fn known_steam_ids() -> Vec<u64> {
    let mut out = vec![];
    for d in default_save_dirs() {
        if let Some(id) = crypto::steam_id_from_path(&d.join("x.sav")) {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// Default save folder: Documents\My Games\Borderlands 4\Saved\SaveGames\<id>\Profiles\client
pub fn default_save_dirs() -> Vec<PathBuf> {
    let mut out = vec![];
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let mut roots = vec![];
    if let Some(h) = &home {
        roots.push(h.join("Documents").join("My Games").join("Borderlands 4").join("Saved").join("SaveGames"));
        roots.push(h.join("OneDrive").join("Documents").join("My Games").join("Borderlands 4").join("Saved").join("SaveGames"));
    }
    for r in roots {
        if let Ok(rd) = std::fs::read_dir(&r) {
            for e in rd.flatten() {
                let p = e.path().join("Profiles").join("client");
                if p.is_dir() {
                    out.push(p);
                }
            }
        }
    }
    out
}

// ====================================================================== scalars

fn get_i64(doc: &Node, path: &str) -> Option<i64> {
    doc.get(path).and_then(|n| n.as_i64())
}

fn get_str(doc: &Node, path: &str) -> Option<String> {
    doc.get(path).and_then(|n| n.as_str()).map(|s| s.to_string())
}

pub fn min_xp(level: u32, mult: f64) -> i64 {
    if level <= 1 {
        0
    } else {
        (mult * ((level as f64).powf(2.8) + 7.33)).floor() as i64
    }
}

pub const CHAR_XP_MULT: f64 = 60.0;
pub const SPEC_XP_MULT: f64 = 80.0;
pub const MAX_CHAR_LEVEL: u32 = 60;
pub const MAX_SPEC_LEVEL: u32 = 700;

// ====================================================================== character

pub struct Character<'a>(pub &'a mut Node);

impl Character<'_> {
    pub fn name(&self) -> String {
        get_str(self.0, "state.char_name").unwrap_or_default()
    }
    pub fn set_name(&mut self, n: &str) {
        self.0.set_scalar("state.char_name", n);
    }
    pub fn class(&self) -> String {
        get_str(self.0, "state.class").unwrap_or_default()
    }

    fn xp_index(&self, ty: &str) -> Option<usize> {
        self.0
            .get("state.experience")?
            .as_seq()?
            .items
            .iter()
            .position(|it| it.get("type").and_then(|t| t.as_str()) == Some(ty))
    }

    /// (level, points) for "Character" or "Specialization"
    pub fn xp(&self, ty: &str) -> Option<(u32, i64)> {
        let i = self.xp_index(ty)?;
        let e = self.0.get(&format!("state.experience[{i}]"))?;
        Some((e.get("level")?.as_i64()? as u32, e.get("points").and_then(|p| p.as_i64()).unwrap_or(0)))
    }

    /// Set a level the way the game stores it: level, minimum XP for that
    /// level and the matching skill-point pool.
    pub fn set_level(&mut self, ty: &str, level: u32) -> Result<(), SaveError> {
        let (mult, max, pool) = match ty {
            "Character" => (CHAR_XP_MULT, MAX_CHAR_LEVEL, "characterprogresspoints"),
            _ => (SPEC_XP_MULT, MAX_SPEC_LEVEL, "specializationtokenpool"),
        };
        let level = level.clamp(1, max);
        let i = match self.xp_index(ty) {
            Some(i) => i,
            None => {
                let seq = self.0.ensure("state.experience");
                if seq.is_null() {
                    *seq = Node::Seq(Seq::default());
                }
                let s = seq.as_seq_mut().unwrap();
                let mut m = Map::default();
                m.insert("type", Node::str(ty));
                m.insert("level", Node::str("1"));
                m.insert("points", Node::str("0"));
                s.items.push(Node::Map(m));
                s.items.len() - 1
            }
        };
        let base = format!("state.experience[{i}]");
        self.0.set_scalar(&format!("{base}.level"), level.to_string());
        self.0.set_scalar(&format!("{base}.points"), min_xp(level, mult).to_string());
        let pp = format!("progression.point_pools.{pool}");
        if self.0.get(&pp).is_some() || ty == "Character" {
            self.0.set_scalar(&pp, (level - 1).to_string());
        }
        Ok(())
    }

    pub fn currency(&self, k: &str) -> Option<String> {
        get_str(self.0, &format!("state.currencies.{k}"))
    }
    pub fn set_currency(&mut self, k: &str, v: i64) {
        self.0.set_scalar(&format!("state.currencies.{k}"), v.max(0).to_string());
    }

    pub fn difficulty(&self) -> String {
        get_str(self.0, "state.player_difficulty").unwrap_or_else(|| "Normal".into())
    }
    pub fn set_difficulty(&mut self, d: &str) {
        self.0.set_scalar("state.player_difficulty", d);
    }
    pub fn true_mode(&self) -> bool {
        self.0.get("state.true_mode").and_then(|n| n.as_bool()).unwrap_or(false)
    }
    pub fn set_true_mode(&mut self, on: bool) {
        self.0.set_scalar("state.true_mode", if on { "true" } else { "false" });
    }

    pub fn uvh(&self) -> (u32, u32) {
        (
            get_i64(self.0, "globals.highest_unlocked_vault_hunter_level").unwrap_or(0) as u32,
            get_i64(self.0, "globals.vault_hunter_level").unwrap_or(0) as u32,
        )
    }

    pub fn story_complete(&self) -> bool {
        self.0
            .get("missions.local_sets.missionset_main_cityepilogue.status")
            .and_then(|s| s.as_str())
            == Some("completed")
            || self.0.get("globals.mainmissioncomplete").and_then(|n| n.as_bool()) == Some(true)
    }

    /// Unlock UVH ranks 1..=max and select `active` (0 = off). Writes the
    /// rank-up challenge stats the game uses to gate each rank.
    pub fn unlock_uvh(&mut self, max: u32, active: u32) {
        let max = max.min(7);
        self.0.set_scalar("globals.highest_unlocked_vault_hunter_level", max.to_string());
        self.0.set_scalar("globals.vault_hunter_level", active.min(max).to_string());
        let ranks: &[&[&str]] = &[
            &["mission_uvh_1a", "mission_uvh_1b", "mission_uvh_1c", "uvh_1_finalchallenge"],
            &["mission_uvh_2a", "mission_uvh_2b", "mission_uvh_2c", "mission_uvh_2d", "uvh_2_finalchallenge"],
            &["mission_uvh_3a", "mission_uvh_3b", "mission_uvh_3c", "mission_uvh_3d", "uvh_3_finalchallenge"],
            &["mission_uvh_4a", "mission_uvh_4b", "mission_uvh_4c", "mission_uvh_4d", "uvh_4_finalchallenge"],
            &["mission_uvh_5a", "mission_uvh_5b", "mission_uvh_5c", "uvh_5_finalchallenge"],
            &["mission_uvh_6a"],
        ];
        for (r, stats) in ranks.iter().enumerate() {
            if (r as u32) < max {
                for s in *stats {
                    self.0.set_scalar(&format!("stats.challenge.{s}"), "1");
                }
            }
        }
        if max >= 5 {
            self.0.set_scalar("stats.achievements.03_uvh_5", "1");
        }
        if max >= 7 {
            self.0.set_scalar("stats.dlc_challenge.uvh_7", "1");
        }
        if max >= 1 {
            for m in [
                "micro_uvh_blackmarkettutorial",
                "micro_uvh_firmwaretransfertutorial",
                "micro_uvh_trueboss",
                "micro_uvh_trait",
            ] {
                let base = format!("missions.local_sets.missionset_main_postgame.missions.{m}");
                if self.0.get(&format!("{base}.status")).and_then(|s| s.as_str()) != Some("completed") {
                    self.0.set_scalar(&format!("{base}.status"), "completed");
                    self.0.set_scalar(&format!("{base}.ui_flags"), "1");
                }
            }
        }
    }

    pub fn ammo(&self) -> Vec<(String, i64)> {
        self.0
            .get("state.ammo")
            .and_then(|a| a.as_map())
            .map(|m| m.entries.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect())
            .unwrap_or_default()
    }
    pub fn set_ammo(&mut self, k: &str, v: i64) {
        self.0.set_scalar(&format!("state.ammo.{k}"), v.max(0).to_string());
    }

    /// Equipment slots unlocked by SDU/level rewards: weapon 3/4, repkit,
    /// enhancement, class mod.
    pub fn unlock_equip_slots(&mut self) {
        let rewards = [
            ("pgraph.sdu_upgrades.Weapon_Slot_03", 2),
            ("pgraph.sdu_upgrades.Weapon_Slot_04", 3),
            ("pgraph.sdu_upgrades.RepKit_Slot", 6),
            ("pgraph.sdu_upgrades.Enhancement_Slot", 7),
            ("pgraph.sdu_upgrades.Class_Mod_Slot", 8),
        ];
        for (r, slot) in rewards {
            push_unique(self.0.ensure("state.unique_rewards"), r);
            push_unique(self.0.ensure("state.inventory.equip_slots_unlocked"), &slot.to_string());
        }
        if let Some(Node::Seq(s)) = self.0.get_mut("state.inventory.equip_slots_unlocked") {
            s.items.sort_by_key(|n| n.as_i64().unwrap_or(0));
        }
        self.0.set_scalar("globals.repkit_unlocked", "TRUE");
    }

    /// Cosmetics currently equipped: (group, slot part, cosmetic part)
    pub fn equipped_cosmetics(&self) -> Vec<(String, String, String)> {
        let mut out = vec![];
        for g in ["character", "echo4", "vehicle"] {
            if let Some(s) = get_str(self.0, &format!("state.gbxactorparts.{g}")) {
                for (slot, part) in parse_gap(&s) {
                    out.push((g.to_string(), slot, part));
                }
            }
        }
        out
    }

    pub fn set_cosmetic(&mut self, group: &str, slot: &str, part: Option<&str>) {
        let path = format!("state.gbxactorparts.{group}");
        let cur = get_str(self.0, &path).unwrap_or_else(|| "gap".into());
        let mut v = parse_gap(&cur);
        v.retain(|(s, _)| !s.eq_ignore_ascii_case(slot));
        if let Some(p) = part {
            v.push((slot.to_string(), p.to_string()));
        }
        let mut s = String::from("gap");
        for (sl, p) in v {
            s.push_str(&format!(",{sl}[{p}]"));
        }
        let n = self.0.ensure(&path);
        *n = Node::Scalar(Scalar { val: s, style: yaml::Style::Single });
    }
}

fn parse_gap(s: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    for seg in s.split(',').skip(1) {
        if let Some((slot, rest)) = seg.split_once('[') {
            out.push((slot.to_string(), rest.trim_end_matches(']').to_string()));
        }
    }
    out
}

fn push_unique(n: &mut Node, v: &str) {
    if n.is_null() {
        *n = Node::Seq(Seq::default());
    }
    if let Node::Seq(s) = n {
        if !s.items.iter().any(|x| x.as_str() == Some(v)) {
            s.items.push(Node::str(v));
        }
    }
}

// ====================================================================== missions

#[derive(Debug, Clone)]
pub struct MissionState {
    pub set: String,
    pub mission: String,
    pub status: String,
    pub objectives: Vec<(String, String)>,
}

pub fn mission_sets(doc: &Node) -> Vec<(String, String, Vec<MissionState>)> {
    let mut out = vec![];
    let Some(sets) = doc.get("missions.local_sets").and_then(|n| n.as_map()) else { return out };
    for (set, v) in &sets.entries {
        let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("active").to_string();
        let mut ms = vec![];
        if let Some(m) = v.get("missions").and_then(|m| m.as_map()) {
            for (mk, mv) in &m.entries {
                let objectives = mv
                    .get("objectives")
                    .and_then(|o| o.as_map())
                    .map(|o| {
                        o.entries
                            .iter()
                            .map(|(k, v)| (k.clone(), v.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                ms.push(MissionState {
                    set: set.clone(),
                    mission: mk.clone(),
                    status: mv.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                    objectives,
                });
            }
        }
        out.push((set.clone(), status, ms));
    }
    out
}

/// Mark a mission completed (status, ui flag, drop live objectives).
pub fn complete_mission(doc: &mut Node, set: &str, mission: &str) {
    let base = format!("missions.local_sets.{set}.missions.{mission}");
    doc.set_scalar(&format!("{base}.status"), "completed");
    doc.set_scalar(&format!("{base}.ui_flags"), "1");
    if let Some(Node::Map(m)) = doc.get_mut(&base) {
        m.remove("objectives");
    }
}

/// Mark a whole mission set completed, with every mission the game data lists for it.
pub fn complete_set(doc: &mut Node, set: &str, missions: &[String]) {
    for m in missions {
        complete_mission(doc, set, m);
    }
    doc.set_scalar(&format!("missions.local_sets.{set}.status"), "completed");
}

/// Set one objective's status inside an active mission (the in-mission checkpoint).
pub fn set_objective(doc: &mut Node, set: &str, mission: &str, objective: &str, status: &str) {
    doc.set_scalar(&format!("missions.local_sets.{set}.missions.{mission}.objectives.{objective}.status"), status);
}

/// Remove a set's progress so the game treats it as not started.
pub fn reset_set(doc: &mut Node, set: &str) {
    if let Some(Node::Map(m)) = doc.get_mut("missions.local_sets") {
        m.remove(set);
    }
}

/// Main story order (mission set ids) and globals granted along the way.
pub const MAIN_STORY: &[&str] = &[
    "missionset_main_prisonprologue",
    "missionset_main_beach",
    "missionset_main_grasslands1",
    "missionset_main_grasslands2a",
    "missionset_main_grasslands2b",
    "missionset_main_mountains1",
    "missionset_main_shatteredlands1",
    "missionset_main_grasslands3",
    "missionset_main_mountains2a",
    "missionset_main_shatteredlands2",
    "missionset_main_searchforlilith",
    "missionset_main_mountains2b",
    "missionset_main_shatteredlands3",
    "missionset_main_elpis",
    "missionset_main_mountains3",
    "missionset_main_city1",
    "missionset_main_city1_b",
    "missionset_main_city2",
    "missionset_main_city3",
    "missionset_main_cityepilogue",
];

pub const STORY_GLOBALS: &[&str] = &[
    "prologue_completed",
    "movegrant_grapplegrabber",
    "movegrant_echolocation",
    "repkit_unlocked",
    "movegrant_glide",
    "mainmissioncomplete",
    "lockdownlifted",
    "movegrant_ordonitegloves",
];

// ====================================================================== inventory

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Container {
    Backpack,
    Equipped,
    LostLoot,
    Bank,
    Unknown,
}

impl Container {
    pub fn label(&self) -> &'static str {
        match self {
            Container::Backpack => "Backpack",
            Container::Equipped => "Equipped",
            Container::LostLoot => "Lost Loot",
            Container::Bank => "Bank",
            Container::Unknown => "Unknown items",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InvItem {
    pub container: Container,
    /// YAML path of the item's map (holding `serial`)
    pub path: String,
    /// slot_N number (backpack/bank) or equip slot / list index
    pub slot: usize,
    pub serial: String,
    pub flags: Option<i64>,
    pub state_flags: Option<i64>,
}

impl InvItem {
    pub fn equipped(&self) -> bool {
        self.flags == Some(1) || self.container == Container::Equipped
    }
    pub fn favorite(&self) -> bool {
        self.state_flags.unwrap_or(0) & 2 != 0
    }
    pub fn junk(&self) -> bool {
        self.state_flags.unwrap_or(0) & 4 != 0
    }
}

const BACKPACK: &str = "state.inventory.items.backpack";
const EQUIPPED: &str = "state.inventory.equipped_inventory.equipped";
const LOSTLOOT: &str = "state.lostloot.items";
const BANK: &str = "domains.local.shared.inventory.items.bank";

fn slot_num(k: &str) -> Option<usize> {
    k.strip_prefix("slot_")?.parse().ok()
}

fn item_from(n: &Node, container: Container, path: String, slot: usize) -> Option<InvItem> {
    let serial = n.get("serial")?.as_str()?.to_string();
    Some(InvItem {
        container,
        path,
        slot,
        serial,
        flags: n.get("flags").and_then(|f| f.as_i64()),
        state_flags: n.get("state_flags").and_then(|f| f.as_i64()),
    })
}

pub fn list_items(doc: &Node) -> Vec<InvItem> {
    let mut out = vec![];
    if let Some(bp) = doc.get(BACKPACK).and_then(|n| n.as_map()) {
        let mut v: Vec<(usize, &Node)> = bp.entries.iter().filter_map(|(k, n)| slot_num(k).map(|s| (s, n))).collect();
        v.sort_by_key(|x| x.0);
        for (s, n) in v {
            if let Some(it) = item_from(n, Container::Backpack, format!("{BACKPACK}.slot_{s}"), s) {
                out.push(it);
            }
        }
        if let Some(Node::Seq(u)) = bp.get("unknown_items") {
            for (i, n) in u.items.iter().enumerate() {
                if let Some(it) = item_from(n, Container::Unknown, format!("{BACKPACK}.unknown_items[{i}]"), i) {
                    out.push(it);
                }
            }
        }
    }
    if let Some(eq) = doc.get(EQUIPPED).and_then(|n| n.as_map()) {
        for (k, n) in &eq.entries {
            if let (Some(s), Some(Node::Seq(list))) = (slot_num(k), Some(n)) {
                for (i, it) in list.items.iter().enumerate() {
                    if let Some(x) = item_from(it, Container::Equipped, format!("{EQUIPPED}.slot_{s}[{i}]"), s) {
                        out.push(x);
                    }
                }
            }
        }
    }
    if let Some(Node::Seq(l)) = doc.get(LOSTLOOT) {
        for (i, n) in l.items.iter().enumerate() {
            if let Some(it) = item_from(n, Container::LostLoot, format!("{LOSTLOOT}[{i}]"), i) {
                out.push(it);
            }
        }
    }
    if let Some(bank) = doc.get(BANK).and_then(|n| n.as_map()) {
        let mut v: Vec<(usize, &Node)> = bank.entries.iter().filter_map(|(k, n)| slot_num(k).map(|s| (s, n))).collect();
        v.sort_by_key(|x| x.0);
        for (s, n) in v {
            if let Some(it) = item_from(n, Container::Bank, format!("{BANK}.slot_{s}"), s) {
                out.push(it);
            }
        }
    }
    out
}

/// Replace an item's serial. An equipped item's backpack twin (or the
/// equipped copy of a backpack item) is updated too.
pub fn set_item_serial(doc: &mut Node, item: &InvItem, new_serial: &str) {
    let old = item.serial.clone();
    doc.set_scalar(&format!("{}.serial", item.path), new_serial);
    if matches!(item.container, Container::Backpack | Container::Equipped) {
        for other in list_items(doc) {
            if other.serial == old && matches!(other.container, Container::Backpack | Container::Equipped) {
                doc.set_scalar(&format!("{}.serial", other.path), new_serial);
            }
        }
    }
}

pub fn set_state_flags(doc: &mut Node, item: &InvItem, flags: i64) {
    doc.set_scalar(&format!("{}.state_flags", item.path), flags.to_string());
    if matches!(item.container, Container::Backpack | Container::Equipped) {
        for other in list_items(doc) {
            if other.serial == item.serial && matches!(other.container, Container::Backpack | Container::Equipped) {
                doc.set_scalar(&format!("{}.state_flags", other.path), flags.to_string());
            }
        }
    }
}

/// Count of backpack items that use capacity (equipped ones are free).
pub fn backpack_used(doc: &Node) -> usize {
    list_items(doc).iter().filter(|i| i.container == Container::Backpack && i.flags != Some(1)).count()
}

pub fn bank_used(doc: &Node) -> usize {
    list_items(doc).iter().filter(|i| i.container == Container::Bank).count()
}

/// Add an item to the backpack in the next free slot (game style: `state_flags: 1`).
pub fn add_backpack_item(doc: &mut Node, serial: &str) -> usize {
    let next = doc
        .get(BACKPACK)
        .and_then(|n| n.as_map())
        .map(|m| m.keys().filter_map(slot_num).max().map(|x| x + 1).unwrap_or(0))
        .unwrap_or(0);
    let mut m = Map::default();
    m.insert("serial", Node::Scalar(Scalar { val: serial.to_string(), style: yaml::Style::Single }));
    m.insert("state_flags", Node::str("1"));
    insert_slot(doc.ensure(BACKPACK), next, Node::Map(m));
    next
}

pub fn add_bank_item(doc: &mut Node, serial: &str) -> usize {
    let next = doc
        .get(BANK)
        .and_then(|n| n.as_map())
        .map(|m| m.keys().filter_map(slot_num).max().map(|x| x + 1).unwrap_or(0))
        .unwrap_or(0);
    let mut m = Map::default();
    m.insert("serial", Node::Scalar(Scalar { val: serial.to_string(), style: yaml::Style::Single }));
    m.insert("state_flags", Node::str("1"));
    insert_slot(doc.ensure(BANK), next, Node::Map(m));
    next
}

fn insert_slot(container: &mut Node, slot: usize, item: Node) {
    if container.is_null() {
        *container = Node::Map(Map::default());
    }
    if let Node::Map(m) = container {
        // keep slots before any non-slot key (unknown_items)
        let pos = m.entries.iter().position(|(k, _)| slot_num(k).is_none()).unwrap_or(m.entries.len());
        m.entries.insert(pos, (format!("slot_{slot}"), item));
    }
}

/// Remove a backpack/bank item and renumber the following slots so they stay
/// contiguous. Removing an equipped backpack item also unequips it.
pub fn remove_item(doc: &mut Node, item: &InvItem) {
    match item.container {
        Container::Backpack | Container::Bank => {
            let base = if item.container == Container::Bank { BANK } else { BACKPACK };
            if let Some(Node::Map(m)) = doc.get_mut(base) {
                m.remove(&format!("slot_{}", item.slot));
                for (k, _) in m.entries.iter_mut() {
                    if let Some(n) = slot_num(k) {
                        if n > item.slot {
                            *k = format!("slot_{}", n - 1);
                        }
                    }
                }
            }
            if item.container == Container::Backpack && item.flags == Some(1) {
                unequip_serial(doc, &item.serial);
            }
        }
        Container::Equipped => {
            unequip_serial(doc, &item.serial);
            // the backpack twin is no longer equipped
            for other in list_items(doc) {
                if other.container == Container::Backpack && other.serial == item.serial {
                    if let Some(Node::Map(m)) = doc.get_mut(&other.path) {
                        m.remove("flags");
                    }
                }
            }
        }
        Container::LostLoot => {
            if let Some(Node::Seq(s)) = doc.get_mut(LOSTLOOT) {
                if item.slot < s.items.len() {
                    s.items.remove(item.slot);
                }
            }
        }
        Container::Unknown => {
            if let Some(Node::Seq(s)) = doc.get_mut(&format!("{BACKPACK}.unknown_items")) {
                if item.slot < s.items.len() {
                    s.items.remove(item.slot);
                }
            }
        }
    }
}

fn unequip_serial(doc: &mut Node, serial: &str) {
    if let Some(Node::Map(m)) = doc.get_mut(EQUIPPED) {
        m.entries.retain(|(_, v)| {
            !matches!(v, Node::Seq(s) if s.items.iter().any(|it| it.get("serial").and_then(|x| x.as_str()) == Some(serial)))
        });
    }
}

// ====================================================================== profile

pub struct Profile<'a>(pub &'a mut Node);

pub const SDU_GRAPH: &str = "sdu_upgrades";

impl Profile<'_> {
    pub fn sdu_tokens(&self) -> i64 {
        get_i64(self.0, "domains.local.progression_shared.point_pools.echotokenprogresspoints").unwrap_or(0)
    }
    pub fn set_sdu_tokens(&mut self, v: i64) {
        self.0.set_scalar("domains.local.progression_shared.point_pools.echotokenprogresspoints", v.max(0).to_string());
    }

    fn sdu_graph_index(&self) -> Option<usize> {
        self.0
            .get("domains.local.progression_shared.graphs")?
            .as_seq()?
            .items
            .iter()
            .position(|g| g.get("name").and_then(|n| n.as_str()) == Some(SDU_GRAPH))
    }

    /// Purchased SDU nodes (name -> points spent)
    pub fn sdu_nodes(&self) -> Vec<(String, i64)> {
        let Some(i) = self.sdu_graph_index() else { return vec![] };
        self.0
            .get(&format!("domains.local.progression_shared.graphs[{i}].nodes"))
            .and_then(|n| n.as_seq())
            .map(|s| {
                s.items
                    .iter()
                    .filter_map(|n| Some((n.get("name")?.as_str()?.to_string(), n.get("points_spent").and_then(|p| p.as_i64()).unwrap_or(0))))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Buy SDU nodes: each purchased node stores `points_spent = cost`.
    /// Tokens are raised if needed so the total stays consistent.
    pub fn set_sdu_nodes(&mut self, nodes: &[(String, u32)]) {
        let i = match self.sdu_graph_index() {
            Some(i) => i,
            None => {
                let g = self.0.ensure("domains.local.progression_shared.graphs");
                if g.is_null() {
                    *g = Node::Seq(Seq::default());
                }
                let s = g.as_seq_mut().unwrap();
                let mut m = Map::default();
                m.insert("name", Node::str(SDU_GRAPH));
                m.insert("group_def_name", Node::str("Oak2_GlobalProgressGraph_Group"));
                m.insert("nodes", Node::null());
                s.items.push(Node::Map(m));
                s.items.len() - 1
            }
        };
        let path = format!("domains.local.progression_shared.graphs[{i}].nodes");
        let mut seq = Seq::default();
        for (name, cost) in nodes {
            let mut m = Map::default();
            m.insert("name", Node::str(name.as_str()));
            m.insert("points_spent", Node::str(cost.to_string()));
            seq.items.push(Node::Map(m));
        }
        *self.0.ensure(&path) = Node::Seq(seq);
        let spent: i64 = nodes.iter().map(|(_, c)| *c as i64).sum();
        if self.sdu_tokens() < spent {
            self.set_sdu_tokens(spent);
        }
    }

    /// Unlocked cosmetic ids per group (e.g. unlockable_darksiren)
    pub fn unlockables(&self) -> Vec<(String, Vec<String>)> {
        self.0
            .get("domains.local.unlockables")
            .and_then(|n| n.as_map())
            .map(|m| {
                m.entries
                    .iter()
                    .map(|(k, v)| {
                        let list = v
                            .get("entries")
                            .and_then(|e| e.as_seq())
                            .map(|s| s.items.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default();
                        (k.clone(), list)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn add_unlockables(&mut self, group: &str, ids: &[String]) -> usize {
        let path = format!("domains.local.unlockables.{group}.entries");
        let n = self.0.ensure(&path);
        if n.is_null() {
            *n = Node::Seq(Seq::default());
        }
        let mut added = 0;
        if let Node::Seq(s) = n {
            for id in ids {
                if !s.items.iter().any(|x| x.as_str() == Some(id)) {
                    s.items.push(Node::str(id.as_str()));
                    added += 1;
                }
            }
        }
        added
    }

    pub fn add_shared_progress(&mut self, flag: &str) {
        push_unique(self.0.ensure("domains.local.unlockables.shared_progress.entries"), flag);
    }

    pub fn vault_card_tokens(&self) -> Vec<(String, i64)> {
        self.0
            .get("domains.local.shared.currencies")
            .and_then(|n| n.as_map())
            .map(|m| m.entries.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect())
            .unwrap_or_default()
    }
    pub fn set_shared_currency(&mut self, k: &str, v: i64) {
        self.0.set_scalar(&format!("domains.local.shared.currencies.{k}"), v.max(0).to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xp_formula_matches_saves() {
        assert_eq!(min_xp(2, CHAR_XP_MULT), 857);
        assert_eq!(min_xp(50, CHAR_XP_MULT), 3430227);
        assert_eq!(min_xp(60, CHAR_XP_MULT), 5714893);
        assert_eq!(min_xp(2, SPEC_XP_MULT), 1143);
    }

    #[test]
    fn gap_roundtrip() {
        let v = parse_gap("gap,Cosmetics_Vehicle[Cosmetics_Vehicle_Mat66_ExNihilo],A[B]");
        assert_eq!(v, vec![("Cosmetics_Vehicle".into(), "Cosmetics_Vehicle_Mat66_ExNihilo".into()), ("A".into(), "B".into())]);
    }

    #[test]
    fn timestamp_shape() {
        assert_eq!(timestamp().len(), 15);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20730), (2026, 10, 4));
    }
}

// ====================================================================== helpers used by the GUI

/// Mission records the game itself wrote for its "skip story" option
/// (extracted by tools/make_story_template.py).
static STORY_TEMPLATE: &str = include_str!("../../../data/story_template.yaml");

/// Complete the main story the way the game's own story skip does: every main
/// mission set the character has not finished is replaced by the record the
/// game writes for a skipped story, and the story globals are set. Sets the
/// template lacks are completed generically.
pub fn complete_story(doc: &mut Node, set_missions: &std::collections::HashMap<String, Vec<String>>) {
    let tpl = yaml::parse(STORY_TEMPLATE).ok();
    for set in MAIN_STORY.iter().copied().chain(std::iter::once("missionset_main_postgame")) {
        let path = format!("missions.local_sets.{set}");
        let done = doc.get(&format!("{path}.status")).and_then(|s| s.as_str()) == Some("completed");
        if done {
            continue;
        }
        match tpl.as_ref().and_then(|t| t.get(&path)).cloned() {
            Some(node) => *doc.ensure(&path) = node,
            None => {
                let ms = set_missions.get(set).cloned().unwrap_or_default();
                complete_set(doc, set, &ms);
            }
        }
    }
    let globals: Vec<String> = tpl
        .as_ref()
        .and_then(|t| t.get("globals"))
        .and_then(|g| g.as_map())
        .map(|m| m.keys().map(|k| k.to_string()).collect())
        .unwrap_or_else(|| STORY_GLOBALS.iter().map(|s| s.to_string()).collect());
    for g in globals {
        doc.set_scalar(&format!("globals.{g}"), "TRUE");
    }
    // tracked missions would point at finished missions
    if let Some(Node::Map(m)) = doc.get_mut("missions") {
        m.remove("tracked_missions");
    }
}

/// Which equip slot a cosmetic part goes into, e.g.
/// `Cosmetics_DarkSiren_Head07_Demon` -> (`character`, `Cosmetics_DarkSiren_Head`).
pub fn cosmetic_slot(part: &str) -> Option<(&'static str, String)> {
    let p = part;
    if p.starts_with("Cosmetics_Vehicle") {
        return Some(("vehicle", "Cosmetics_Vehicle".into()));
    }
    let rest = p.strip_prefix("Cosmetics_")?;
    let (owner, tail) = rest.split_once('_')?;
    let kind: String = tail.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let kind = match kind.as_str() {
        k if k.starts_with("Skin") => "Skin",
        k if k.starts_with("Head") => "Head",
        k if k.starts_with("Body") => "Body",
        k if k.starts_with("Attachment") => "Attachment",
        _ => return None,
    };
    let group = if owner.eq_ignore_ascii_case("echo4") { "echo4" } else { "character" };
    Some((group, format!("Cosmetics_{owner}_{kind}")))
}

/// Structural rules every game-written save follows. Run before writing;
/// a non-empty result means an edit produced something the game never writes.
pub fn check_invariants(doc: &Node, kind: SaveKind) -> Vec<String> {
    let mut out = vec![];
    let items = list_items(doc);
    // contiguous slot numbers
    for (c, base) in [(Container::Backpack, BACKPACK), (Container::Bank, BANK)] {
        let mut slots: Vec<usize> = items.iter().filter(|i| i.container == c).map(|i| i.slot).collect();
        slots.sort();
        if slots.iter().enumerate().any(|(i, s)| i != *s) {
            out.push(format!("{} slots are not contiguous ({base})", c.label()));
        }
    }
    // equipped items have a backpack twin with the same state flags
    for e in items.iter().filter(|i| i.container == Container::Equipped) {
        match items.iter().find(|b| b.container == Container::Backpack && b.serial == e.serial) {
            None => out.push(format!("Equipped item in slot {} has no backpack copy", e.slot)),
            Some(b) => {
                if b.flags != Some(1) {
                    out.push(format!("Backpack copy of equipped item (slot {}) is not marked equipped", e.slot));
                }
                if b.state_flags != e.state_flags {
                    out.push(format!("Equipped item in slot {} and its backpack copy have different flags", e.slot));
                }
            }
        }
    }
    // every serial decodes
    for i in &items {
        if crate::serial::Serial::decode(&i.serial).is_err() {
            out.push(format!("{} item {} has an undecodable serial", i.container.label(), i.slot));
        }
    }
    if kind == SaveKind::Character {
        if let Some(e) = doc.get("state.experience").and_then(|n| n.as_seq()) {
            for x in &e.items {
                let ty = x.get("type").and_then(|t| t.as_str()).unwrap_or("");
                let lvl = x.get("level").and_then(|l| l.as_i64()).unwrap_or(0);
                let max = if ty == "Character" { MAX_CHAR_LEVEL } else { MAX_SPEC_LEVEL } as i64;
                if lvl < 1 || lvl > max {
                    out.push(format!("{ty} level {lvl} is outside 1..{max}"));
                }
            }
        }
    }
    out
}
