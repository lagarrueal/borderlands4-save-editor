//! Item card statistics computed from the game's own balance data.
//!
//! An item's numbers come from attribute effects: the item type's base
//! aspects (category -> base type -> root), then each part's aspects. Many
//! part aspects are templates (`barrel_attr_base_values`, ...) defined once
//! and pointed at a data-table row by the part. Values are resolved through
//! data tables and attribute formulas, e.g. weapon damage =
//! base_weapon_damage x universal_balance_scalar^level x type scale x
//! barrel Damage_Scale, then scaled by magazine/part modifiers.
//!
//! The result is an estimate: skills, anointments and in-game buffs are not
//! applied, and a few resolvers (expressions) are not evaluated.

use crate::db::Db;
use crate::item::{ItemInfo, Rarity};
use crate::serial::Serial;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

fn norm_col(c: &str) -> String {
    let l = c.to_lowercase();
    // strip `_<n>_<32 hex>` suffixes UE adds to struct columns
    let parts: Vec<&str> = l.rsplitn(3, '_').collect();
    if parts.len() == 3 && parts[0].len() == 32 && parts[0].chars().all(|c| c.is_ascii_hexdigit()) && parts[1].chars().all(|c| c.is_ascii_digit()) {
        return parts[2].to_string();
    }
    l
}

fn unquote(s: &str) -> &str {
    match (s.find('\''), s.rfind('\'')) {
        (Some(a), Some(b)) if b > a => &s[a + 1..b],
        _ => s,
    }
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

pub struct Eval<'a> {
    db: &'a Db,
    pub level: f64,
    rarity_row: &'static str,
    depth: u32,
    cache: HashMap<String, Option<f64>>,
}

impl<'a> Eval<'a> {
    pub fn new(db: &'a Db, level: u32, rarity: Rarity) -> Self {
        let rarity_row = match rarity {
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
            Rarity::Pearlescent => "pearl",
            _ => "common",
        };
        Eval { db, level: level as f64, rarity_row, depth: 0, cache: HashMap::new() }
    }

    pub fn cell(&self, dt: &str, row: &str, col: Option<&str>) -> Option<f64> {
        let t = self.db.tables.get(&dt.to_lowercase())?;
        let r = t.get(&row.to_lowercase())?;
        match col {
            Some(c) => num(r.get(&norm_col(c))?),
            None => {
                let nums: Vec<f64> = r.values().filter_map(num).collect();
                if nums.len() == 1 {
                    Some(nums[0])
                } else {
                    r.get("none").and_then(num).or_else(|| r.get("value").and_then(num))
                }
            }
        }
    }

    pub fn attr(&mut self, name: &str) -> Option<f64> {
        let key = name.to_lowercase();
        if key == "none" {
            return None;
        }
        if let Some(v) = self.cache.get(&key) {
            return *v;
        }
        if self.depth > 24 {
            return None;
        }
        let def = self.db.attributes.get(&key)?.clone();
        self.depth += 1;
        let r = def.get("value").and_then(|v| self.raw(v));
        self.depth -= 1;
        self.cache.insert(key, r);
        r
    }

    /// Resolve a raw value object from the game data.
    pub fn raw(&mut self, v: &Value) -> Option<f64> {
        if let Some(st) = v.get("structtype").and_then(|s| s.as_str()) {
            let st = unquote(st);
            if st.ends_with("BalanceFormulaValueResolver") {
                let m = v.get("multiplier").and_then(|x| self.raw(x)).unwrap_or(1.0);
                let l = v.get("level").and_then(|x| self.raw(x)).unwrap_or(1.0);
                let p = v.get("power").and_then(|x| self.raw(x)).unwrap_or(1.0);
                let s = v.get("scalar").and_then(|x| self.raw(x)).unwrap_or(1.0);
                return Some(m * l.powf(p) * s);
            }
            if st.ends_with("BalanceStateValueResolver") {
                return match v.get("valuetoresolve").and_then(|x| x.as_str()) {
                    Some("ExperienceLevel") | Some("GameStage") => Some(self.level),
                    _ => None,
                };
            }
            if st.ends_with("InventoryRarityDataTableValueResolver") {
                let col = v.get("raritytablecolumn").and_then(|x| x.as_str())?;
                return self.cell("rarity_balance", self.rarity_row, Some(col)).or(Some(1.0));
            }
        }
        let mut base = None;
        if let Some(dv) = v.get("datatablevalue") {
            if let Some(dt) = dv.get("datatable").and_then(|x| x.as_str()) {
                let dt = unquote(dt);
                if !dt.eq_ignore_ascii_case("none") {
                    let row = dv.get("rowname").and_then(|x| x.as_str()).unwrap_or("");
                    let col = dv.get("columnname").and_then(|x| x.as_str()).filter(|c| !c.eq_ignore_ascii_case("none"));
                    base = self.cell(dt, row, col);
                }
            }
        }
        if base.is_none() {
            if let Some(a) = v.get("attribute").and_then(|x| x.as_str()) {
                let a = unquote(a);
                if !a.eq_ignore_ascii_case("none") {
                    base = self.attr(a);
                }
            }
        }
        if base.is_none() {
            base = v.get("constant").and_then(num);
        }
        if base.is_none() {
            // a bare number or string
            base = num(v);
        }
        let ps = v.get("postscale").and_then(num).unwrap_or(1.0);
        base.map(|b| b * ps)
    }

    /// The row struct's default for a column of `dt`: the value of a cell the
    /// NCS table omits (build_db's `__default__` row).
    pub fn struct_default(&self, dt: &str, col: &str) -> Option<f64> {
        num(self.db.tables.get(&dt.to_lowercase())?.get("__default__")?.get(&norm_col(col))?)
    }

    /// Resolve a compact value from the editor database (k / dt,row,col / attr / ps).
    pub fn compact(&mut self, e: &Value) -> Option<f64> {
        let mut base = None;
        if let (Some(dt), Some(row)) = (e.get("dt").and_then(|x| x.as_str()), e.get("row").and_then(|x| x.as_str())) {
            base = self.cell(dt, row, e.get("col").and_then(|x| x.as_str()));
        }
        if base.is_none() {
            if let Some(a) = e.get("attr").and_then(|x| x.as_str()) {
                base = self.attr(a);
            }
        }
        if base.is_none() {
            base = e.get("k").and_then(num);
        }
        let ps = e.get("ps").and_then(num).unwrap_or(1.0);
        base.map(|b| b * ps)
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub attr: String,
    pub op: String,
    pub val: f64,
    pub src: String,
}

#[derive(Debug, Default, Clone)]
pub struct Computed {
    pub effects: Vec<Effect>,
    /// base values supplied by the fire behaviour (damage, fire rate, ...)
    pub base: BTreeMap<String, f64>,
}

impl Computed {
    /// Aggregate one attribute:
    /// ((base + PreAdd) * (1 + ScaleAdd + ScaleSimple) * prod(ScaleMultiply)) + PostAdd
    pub fn value(&self, attr: &str) -> Option<f64> {
        let mut base = self.base.get(attr).copied();
        let mut pre = 0.0;
        let mut add = 0.0;
        let mut mul = 1.0;
        let mut post = 0.0;
        let mut any = base.is_some();
        for e in self.effects.iter().filter(|e| e.attr.eq_ignore_ascii_case(attr)) {
            any = true;
            match e.op.as_str() {
                "OverrideBaseValue" => base = Some(e.val),
                "PreAdd" | "Add" => pre += e.val,
                "ScaleAdd" | "ScaleSimple" => add += e.val,
                "ScaleMultiply" => mul *= e.val,
                "PostAdd" => post += e.val,
                _ => {}
            }
        }
        if !any {
            return None;
        }
        Some((base.unwrap_or(0.0) + pre) * (1.0 + add) * mul + post)
    }
}

const MODE_PRIMARY: u32 = 1;

fn mode_ok(e: &Value) -> bool {
    match e.get("mode").and_then(num) {
        Some(m) => (m as u32) & MODE_PRIMARY != 0,
        None => true,
    }
}

/// Collect every effect that applies to an item.
pub fn compute(db: &Db, s: &Serial, info: &ItemInfo) -> Computed {
    let level = info.level.unwrap_or(1).max(1);
    let mut ev = Eval::new(db, level, info.rarity);
    let mut out = Computed::default();

    // base-type chain
    let mut chain = vec![];
    if let Some(c) = db.category(info.category) {
        let mut key = c.key.clone();
        for _ in 0..8 {
            let Some(b) = db.bases.get(&key) else { break };
            chain.push((key.clone(), b.clone()));
            match b.get("base").and_then(|x| x.as_str()) {
                Some(n) if n != key => key = n.to_string(),
                _ => break,
            }
        }
    }
    chain.reverse(); // root first
    for (key, b) in &chain {
        add_source(&mut ev, db, &mut out, b, &format!("type {key}"), true);
    }
    for r in s.parts() {
        if let Some(p) = db.part(r) {
            // fire behaviour of the primary mode comes from the barrel only;
            // underbarrels carry their own behaviour for the secondary mode
            let beh = if p.s == "barrel" { p.beh.clone() } else { None };
            let v = serde_json::json!({
                "fx": p.fx, "tpl": p.tpl, "beh": beh, "asp": p.asp,
            });
            add_source(&mut ev, db, &mut out, &v, &p.k, false);
        }
    }
    // Part stat modifiers ("Damage mod", "Reload speed mod", "Fire rate mod",
    // pearl stat parts, ...): each part gives points to a stat tag, and the
    // item type's inv_stat definition turns them into attribute modifiers:
    //   points x StatToAttributeModifierScalar (Weapon_Stats[row].Default)
    //          x BaseMultiplier (Weapon_Stats[row].<Manufacturer>, 1 if empty)
    //          x Rarity_Balance[rarity].Stat_Scale (Rarity.StatMultiplierColumnName)
    // negated for bZeroIsBetter stats (reload time, burst delay, ...), applied
    // with the entry's modifier type (ScaleAdd unless PreAdd/...).
    // This is where rarity scales damage (Stat_Scale on the damage points);
    // Rarity_Balance.Damage_Scale_Level is not applied on top of it.
    let statdefs = chain.iter().rev().find_map(|(_, b)| b.get("statdefs").and_then(|x| x.as_array()).cloned());
    if let Some(defs) = statdefs {
        let stat_scale = ev.cell("rarity_balance", ev.rarity_row, Some("stat_scale")).unwrap_or(1.0);
        for r in s.parts() {
            let Some(p) = db.part(r) else { continue };
            for m in part_stat_mods(db, p).into_iter().filter(mode_ok) {
                let Some(tag) = m.get("stat").and_then(|x| x.as_str()) else { continue };
                let Some(points) = ev.compact(&m) else { continue };
                for d in defs.iter().filter(|d| d.get("stat").and_then(|x| x.as_str()).is_some_and(|t| t.eq_ignore_ascii_case(tag))) {
                    let Some(attr) = d.get("a").and_then(|x| x.as_str()) else { continue };
                    let Some(scalar) = d.get("sc").and_then(|x| ev.compact(x)) else { continue };
                    let mult = d.get("bm").and_then(|x| ev.compact(x)).unwrap_or(1.0);
                    let mut val = points * scalar * mult * stat_scale;
                    if d.get("neg").and_then(|x| x.as_bool()).unwrap_or(false) {
                        val = -val;
                    }
                    let op = d.get("op").and_then(|x| x.as_str()).unwrap_or("ScaleAdd");
                    out.effects.push(Effect { attr: attr.to_string(), op: op.to_string(), val, src: format!("{} (stat {tag} {points:+})", p.k) });
                }
            }
        }
    }
    out
}

/// A part's stat modifiers: its own, or those of a stat template aspect
/// (the "Mag size mod" parts point at `stat_mod_mag_size`).
fn part_stat_mods(db: &Db, p: &crate::db::Part) -> Vec<Value> {
    let mut out: Vec<Value> = p.mods.as_ref().and_then(|m| m.as_array()).cloned().unwrap_or_default();
    if out.is_empty() {
        for a in p.asp.as_ref().and_then(|a| a.as_array()).into_iter().flatten().filter_map(|x| x.as_str()) {
            if let Some(m) = db.aspects.get(&a.to_lowercase()).and_then(|d| d.get("mods")).and_then(|m| m.as_array()) {
                out.extend(m.iter().cloned());
            }
        }
    }
    out
}

impl Computed {
    /// `value`, but only when something gives the attribute a base (a fire
    /// behaviour value or an OverrideBaseValue): stat modifiers alone do not
    /// make a stat (e.g. "Reload speed mod" on a Borg recharging magazine,
    /// which has no reload time).
    pub fn based_value(&self, attr: &str) -> Option<f64> {
        let has = self.base.contains_key(attr) || self.effects.iter().any(|e| e.op == "OverrideBaseValue" && e.attr.eq_ignore_ascii_case(attr));
        if has {
            self.value(attr)
        } else {
            None
        }
    }

    /// Fire rate as the item card shows it: a burst weapon fires
    /// AutomaticBurstCount shots at FireRate, then waits BurstFireDelay.
    pub fn card_fire_rate(&self) -> Option<f64> {
        let fr = self.based_value("weapon_fire_rate")?;
        let n = self.value("weapon_burst_count").unwrap_or(1.0).round();
        let d = self.value("weapon_burst_fire_delay").unwrap_or(0.0);
        if n > 1.0 && d > 0.0 && fr > 0.0 {
            Some(n / (n / fr + d))
        } else {
            Some(fr)
        }
    }

    /// Heat/charge magazines (Reload part value 4, e.g. CoV): no ammo, the
    /// gun overheats instead (weapon_reload_four).
    pub fn reload_four(&self) -> bool {
        self.value("weapon_part_reload_value").is_some_and(|v| (v - 4.0).abs() < 1e-6)
    }

    /// Card "Magazine": weapon_compare_shots_until_reload. Shots to overheat
    /// for heat magazines, else MaxLoadedAmmo (an int; the stat rounds up).
    pub fn card_magazine(&self) -> Option<f64> {
        if self.reload_four() {
            return self.value("weapon_heat_impulse").filter(|h| *h > 0.0).map(|h| (1.0 / h).floor());
        }
        self.based_value("weapon_max_loaded_ammo").map(|v| (v - 1e-6).ceil())
    }

    /// Card "Reload": weapon_compare_reload_time. Single-load magazines
    /// (shell by shell) take ReloadTime x (1 + LoopPercent x (MaxAmmo - 2)) /
    /// FeedIncrement; never below the weapon's minimum reload time.
    pub fn card_reload(&self) -> Option<f64> {
        if self.reload_four() {
            return None;
        }
        let mut r = self.based_value("weapon_reload_time")?;
        if self.value("weapon_is_single_load").unwrap_or(0.0) >= 0.5 {
            let lp = self.value("weapon_single_load_reload_loop_percent").unwrap_or(0.0);
            let m = self.based_value("weapon_max_loaded_ammo").map(|v| (v - 1e-6).ceil()).unwrap_or(1.0);
            let two = |a: &str| self.value(a).is_some_and(|v| (v - 2.0).abs() < 1e-6);
            let feed = if two("weapon_part_reload_value") && two("weapon_part_barrel_value") { 2.0 } else { 1.0 };
            r = r * (1.0 + lp * (m - 2.0).max(0.0)) / feed;
        }
        if let Some(min) = self.value("weapon_min_reload_time") {
            r = r.max(min);
        }
        // a reload this short means a mechanic the engine does not model
        // (e.g. single-shot shotguns); show nothing rather than a wrong number
        (r >= 0.1).then_some(r)
    }

    /// Card DPS, the game's weapon_dps_estimate:
    /// damage x pellets x shots / (shots / burst-aware fire rate + reload),
    /// shots = magazine / ammo cost per shot (or shots to overheat).
    pub fn card_dps(&self) -> Option<f64> {
        let dmg = self.value("weapon_damage")?;
        let pellets = self.value("weapon_projectile_per_shot").unwrap_or(1.0).max(1.0).round();
        let fr = self.card_fire_rate()?;
        let mag = self.card_magazine()?;
        let shots = if self.reload_four() { mag } else { mag / self.value("weapon_shot_cost").unwrap_or(1.0).max(1.0).round() };
        let reload = if self.reload_four() { 0.0 } else { self.card_reload()? };
        if fr <= 0.0 || shots <= 0.0 {
            return None;
        }
        Some(dmg * pellets * shots / (shots / fr + reload))
    }
}

/// `item_type`: `src` is an item type of the base chain. A type that lists a
/// template aspect without overriding its row uses the template's own row
/// (weapon_sr -> weapon_ui_acc_weights' "Sniper" row, dad_repair_kit ->
/// repair_kit_manufacturer_attr_init's "Daedalus" row); parts keep skipping
/// such placeholder cells.
fn add_source(ev: &mut Eval, db: &Db, out: &mut Computed, src: &Value, label: &str, item_type: bool) {
    let mut templated = vec![];
    if let Some(tpl) = src.get("tpl").and_then(|x| x.as_array()) {
        for t in tpl {
            let Some(asp) = t.get("asp").and_then(|x| x.as_str()) else { continue };
            templated.push(asp.to_lowercase());
            let mut name = asp.to_lowercase();
            for _ in 0..4 {
                let Some(def) = db.aspects.get(&name) else { break };
                if let Some(fx) = def.get("fx").and_then(|x| x.as_array()) {
                    for e in fx.iter().filter(|e| mode_ok(e)) {
                        let mut e2 = e.clone();
                        // the part's template points the aspect at its own row
                        if let Some(o) = e2.as_object_mut() {
                            if let Some(dt) = t.get("dt") {
                                o.insert("dt".into(), dt.clone());
                            }
                            if let Some(row) = t.get("row") {
                                o.insert("row".into(), row.clone());
                            }
                            if t.get("dt").is_none() && t.get("row").is_none() {
                                if let Some(k) = t.get("k") {
                                    o.insert("k".into(), k.clone());
                                    o.remove("dt");
                                    o.remove("row");
                                }
                            }
                        }
                        if let (Some(a), Some(v)) = (e2.get("a").and_then(|x| x.as_str()), ev.compact(&e2)) {
                            out.effects.push(Effect {
                                attr: a.to_lowercase(),
                                op: e2.get("op").and_then(|x| x.as_str()).unwrap_or("ScaleMultiply").to_string(),
                                val: v,
                                src: label.to_string(),
                            });
                        }
                    }
                }
                match def.get("parent").and_then(|x| x.as_str()) {
                    Some(p) => name = p.to_string(),
                    None => break,
                }
            }
        }
    }
    if let Some(fx) = src.get("fx").and_then(|x| x.as_array()) {
        for e in fx.iter().filter(|e| mode_ok(e)) {
            if let (Some(a), Some(v)) = (e.get("a").and_then(|x| x.as_str()), ev.compact(e)) {
                out.effects.push(Effect {
                    attr: a.to_lowercase(),
                    op: e.get("op").and_then(|x| x.as_str()).unwrap_or("ScaleMultiply").to_string(),
                    val: v,
                    src: label.to_string(),
                });
            }
        }
    }
    // non-templated parent aspects that carry fixed effects
    if let Some(asps) = src.get("asp").and_then(|x| x.as_array()) {
        for a in asps.iter().filter_map(|x| x.as_str()) {
            if templated.contains(&a.to_lowercase()) {
                continue;
            }
            let Some(def) = db.aspects.get(&a.to_lowercase()) else { continue };
            // a template aspect's data-table cells are placeholders for the
            // row a part supplies; other aspects' cells are real values
            // (element damage scalar 0.8, manufacturer crit bonus, ...)
            let template = def.get("template").and_then(|x| x.as_bool()).unwrap_or(false);
            if let Some(fx) = def.get("fx").and_then(|x| x.as_array()) {
                for e in fx.iter().filter(|e| mode_ok(e) && (!template || item_type || e.get("dt").is_none())) {
                    if let (Some(at), Some(v)) = (e.get("a").and_then(|x| x.as_str()), ev.compact(e)) {
                        out.effects.push(Effect {
                            attr: at.to_lowercase(),
                            op: e.get("op").and_then(|x| x.as_str()).unwrap_or("ScaleMultiply").to_string(),
                            val: v,
                            src: format!("{label} ({a})"),
                        });
                    }
                }
            }
        }
    }
    if let Some(beh) = src.get("beh").and_then(|x| x.as_object()) {
        for (k, attr) in [
            ("damage", "weapon_damage"),
            ("firerate", "weapon_fire_rate"),
            ("spread", "weapon_spread"),
            ("projectilespershot", "weapon_projectile_per_shot"),
            ("automaticburstcount", "weapon_burst_count"),
            ("burstfiredelay", "weapon_burst_fire_delay"),
            ("shotammocost", "weapon_shot_cost"),
            ("accuracyimpulse", "weapon_accuracy_impulse"),
        ] {
            // a cell the table omits holds the row struct's default
            // (Gomie: no AccImpulse_Value -> Struct_Weapon_Barrel_Init 0.2)
            let omitted = |ev: &Eval, v: &Value| {
                let (dt, col) = (v.get("dt")?.as_str()?, v.get("col")?.as_str()?);
                Some(ev.struct_default(dt, col)? * v.get("ps").and_then(num).unwrap_or(1.0))
            };
            if let Some(v) = beh.get(k).and_then(|v| ev.compact(v).or_else(|| omitted(ev, v))) {
                out.base.insert(attr.to_string(), v);
            }
        }
    }
}

/// Whole number with thousands separators, as the item card prints it.
pub fn thousands(v: f64) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

fn fmt_num(v: f64) -> String {
    if v.abs() >= 100.0 {
        format!("{:.0}", v)
    } else if v.abs() >= 10.0 {
        format!("{:.1}", v)
    } else {
        format!("{:.2}", v)
    }
}

/// Lines for the item card: (label, value)
pub fn card(db: &Db, s: &Serial, info: &ItemInfo) -> Vec<(String, String)> {
    let c = compute(db, s, info);
    let mut out = vec![];
    let push = |out: &mut Vec<(String, String)>, l: &str, v: String| out.push((l.to_string(), v));
    match info.kind.as_str() {
        "weapon" | "heavy" => {
            if let Some(d) = c.value("weapon_damage") {
                let pellets = c.value("weapon_projectile_per_shot").unwrap_or(1.0).max(1.0).round();
                if pellets > 1.0 {
                    push(&mut out, "Damage", format!("{} x {}", thousands(d), pellets));
                } else {
                    push(&mut out, "Damage", thousands(d));
                }
            }
            // uistat_accuracy: weapon_accuracy_ui_compare as a whole percent
            if c.based_value("weapon_spread").is_some() {
                let v = UiEval::new(db, info, &c).get("weapon_accuracy_ui_compare");
                push(&mut out, "Accuracy", format!("{:.0}%", v * 100.0));
            }
            if let Some(v) = c.card_fire_rate() {
                push(&mut out, "Fire rate", format!("{:.1}/s", v));
            }
            if let Some(v) = c.card_magazine() {
                push(&mut out, "Magazine", format!("{:.0}", v));
            }
            if let Some(v) = c.card_reload() {
                push(&mut out, "Reload time", format!("{:.1}s", v));
            }
            if let Some(v) = c.card_dps() {
                push(&mut out, "DPS", thousands(v));
            }
            // element line: weapon_ui_elemental_dps = status damage x damage x
            // Status_Application_Defaults[element].DPS / DoT interval
            if let (Some(el), Some(d)) = (info.element.first(), c.value("weapon_damage")) {
                let row = match el.as_str() {
                    "Incendiary" => "fire",
                    other => &other.to_lowercase(),
                };
                let mut ev = Eval::new(db, info.level.unwrap_or(1).max(1), info.rarity);
                let factor = ev.cell("status_application_defaults", row, Some("dps")).unwrap_or(0.0);
                let interval = ev.attr("att_playershared_dotinterval").filter(|v| *v > 0.0);
                let sdmg = c.value("weapon_damage_modifier_base_status_effect_damage").unwrap_or(1.0);
                if let (true, Some(iv)) = (factor > 0.0, interval) {
                    let chance = c.value("weapon_damage_modifier_base_status_effect_chance").unwrap_or(0.0);
                    push(&mut out, el, format!("{} DMG/s | {:.0}% Chance", thousands(sdmg * d * factor / iv), chance * 100.0));
                }
            }
            if let Some(v) = c.value("weapon_damage_modifier_add_critical_hit") {
                if v.abs() > 1e-6 {
                    push(&mut out, "Crit damage", format!("{:+.0}%", v * 100.0));
                }
            }
            // uistat_damage_radius: shown when > 0 as "{ArgA}cm" (default number
            // format: whole, grouped)
            if let Some(v) = c.value("weapon_damage_radius").filter(|v| *v > 0.0) {
                push(&mut out, "Splash radius", format!("{}cm", thousands(v)));
            }
        }
        "shield" | "grenade" | "repkit" => {
            // Gear: base values from the type/manufacturer formulas. Augment
            // tables hold deltas whose exact stacking is not verified, so they
            // are listed per part instead of folded in.
            let base_only = Computed {
                effects: c.effects.iter().filter(|e| e.src.starts_with("type ")).cloned().collect(),
                base: c.base.clone(),
            };
            let lines: &[(&str, &str, &str)] = match info.kind.as_str() {
                "shield" => &[
                    ("shield_capacity", "Capacity", ""),
                    ("shield_regen_rate", "Recharge rate", "/s"),
                    ("shield_regen_delay", "Recharge delay", "s"),
                    ("shield_reactive_armor_damage_capacity", "Armor", ""),
                    ("shield_segments", "Armor segments", ""),
                ],
                "grenade" => &[
                    ("grenade_gadget_damage", "Damage", ""),
                    ("weapon_damage_radius", "Radius", ""),
                    ("gadget_cooldown", "Cooldown", "s"),
                    ("grenade_gadget_max_number_of_charges", "Charges", ""),
                ],
                _ => &[
                    ("repair_kit_heal_amount", "Healing", ""),
                    ("repair_kit_cooldown", "Cooldown", "s"),
                    ("repair_kit_max_charges", "Charges", ""),
                ],
            };
            for (a, l, unit) in lines {
                if let Some(v) = base_only.value(a) {
                    if v > 0.0 && !out.iter().any(|(x, _): &(String, String)| x == l) {
                        push(&mut out, l, format!("{}{unit}", fmt_num(v)));
                    }
                }
            }
        }
        _ => {}
    }
    out
}

/// Every attribute the item touches, for the "all computed attributes" view.
pub fn all_attributes(db: &Db, s: &Serial, info: &ItemInfo) -> Vec<(String, f64, Vec<String>)> {
    let c = compute(db, s, info);
    let mut names: Vec<String> = c.effects.iter().map(|e| e.attr.clone()).chain(c.base.keys().cloned()).collect();
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter_map(|n| {
            let v = c.value(&n)?;
            let srcs = c.effects.iter().filter(|e| e.attr == n).map(|e| format!("{} {} {} ({})", e.op, fmt_num(e.val), "", e.src)).collect();
            Some((n, v, srcs))
        })
        .collect()
}

/// Effects each part contributes, as readable lines keyed by part key.
pub fn part_effects(db: &Db, s: &Serial, info: &ItemInfo) -> HashMap<String, Vec<String>> {
    let c = compute(db, s, info);
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for e in &c.effects {
        let key = e.src.split(" (").next().unwrap_or(&e.src).to_string();
        if key.starts_with("type ") {
            continue;
        }
        let what = e.attr.trim_start_matches("weapon_").replace('_', " ");
        let line = match e.op.as_str() {
            "ScaleMultiply" => format!("{what} x{}", fmt_num(e.val)),
            "ScaleAdd" | "ScaleSimple" => format!("{what} {:+.0}%", e.val * 100.0),
            "PreAdd" | "PostAdd" | "Add" => format!("{what} {:+}", fmt_num(e.val)),
            "OverrideBaseValue" => format!("{what} = {}", fmt_num(e.val)),
            other => format!("{what} {other} {}", fmt_num(e.val)),
        };
        // part-value bookkeeping attributes are not interesting
        if e.attr.starts_with("weapon_part_") {
            continue;
        }
        let v = out.entry(key).or_default();
        if !v.contains(&line) {
            v.push(line);
        }
    }
    out
}

/// Fill `{placeholders}` in an item text line with values computed for this item.
pub fn render_text(db: &Db, s: &Serial, info: &ItemInfo, text: &str) -> String {
    let Some(args) = db.uiargs.get(text) else { return text.to_string() };
    let c = compute(db, s, info);
    let mut ev = Eval::new(db, info.level.unwrap_or(1).max(1), info.rarity);
    let mut out = text.to_string();
    for a in args {
        let Some(k) = &a.k else { continue };
        let v = a
            .a
            .as_deref()
            .and_then(|attr| c.value(attr).or_else(|| ev.attr(attr)))
            .or_else(|| a.c.as_ref().and_then(num))
            // a negative duration/amount means only a modifier resolved, not the value
            .filter(|v| *v >= 0.0 || a.plus);
        let shown = match v {
            Some(v) => {
                let n = if a.pct { format!("{:.0}%", v * 100.0) } else { fmt_num(v) };
                let n = if a.plus && v >= 0.0 { format!("+{n}") } else { n };
                match &a.fmt {
                    Some(f) => f.replace("$VALUE$", &n),
                    None => n,
                }
            }
            None => "?".to_string(),
        };
        out = out.replace(&format!("{{{k}}}"), &shown);
    }
    out
}

// ---------------------------------------------------------------------------
// Item-card attributes evaluated from their game definitions.
//
// The card lines (ui_stat `uistat_*`) display attributes defined in
// attribute_c* as GbxExpressionValueResolver formulas ("(A*(1-(B/C)))+...",
// operators + - * / ^^ (power) < > <= >= == && || !, `attr(name)`, `bb(key)`,
// named variables bound to an Attribute / DataTable cell / constant) and
// GbxConditionalAttributeValueResolver (first true condition's value, else
// `defaultvalue`). Their leaves are stored weapon properties
// (PropertyValueResolver, InventoryStatsContainerValueResolver), read here
// from the item's aggregated effects.
//
// Behaviour context: each attribute's WeaponAttributeContextResolver names
// the behaviour it reads (WeaponBehavior_Fire, _Reload, _Sway...). Inside a
// formula evaluated for one behaviour, a property of an unrelated behaviour
// reads 0: weapon_accuracy_ui_compare (Fire) reads weapon_sway_ui_value =
// weapon_sway_x_scale (Sway.WidthScale) * weapon_sway_y_scale (HeightScale)
// as 0, so the sway term always counts its full weight. Measured on four
// in-game cards: with the real sway (Maliwan SMG 1.6 x 0.9, Jakobs pistol
// 0.4 x 0.3) Plasma Coil and Looming Maggie would show 77-85% and 49-50%
// instead of 86% and 51%.
// ---------------------------------------------------------------------------

/// Class defaults of stored properties the card formulas read when no part
/// sets them (WeaponBehavior_Fire.ShotAmmoCost = 1); anything else reads 0.
const PROP_DEFAULTS: &[(&str, f64)] = &[("weapon_shot_cost", 1.0)];

/// Gbx expression syntax tree.
enum Ex {
    Num(f64),
    Var(String),
    Attr(String),
    Neg(Box<Ex>),
    Not(Box<Ex>),
    Bin(&'static str, Box<Ex>, Box<Ex>),
}

/// Operator tokens, longest first.
const OPS: [&str; 16] = ["^^", "&&", "||", "==", "!=", "<=", ">=", "+", "-", "*", "/", "<", ">", "!", "(", ")"];

fn tokenize(s: &str) -> Option<Vec<String>> {
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' {
            let st = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
            out.push(s[st..i].to_string());
        } else if let Some(op) = OPS.iter().find(|o| s[i..].starts_with(**o)) {
            out.push(op.to_string());
            i += op.len();
        } else {
            return None;
        }
    }
    Some(out)
}

struct ExParser {
    t: Vec<String>,
    i: usize,
}

impl ExParser {
    /// Binary operators by increasing precedence; unary ! - and ^^ bind tighter.
    const LEVELS: [&'static [&'static str]; 5] = [&["||"], &["&&"], &["==", "!=", "<", ">", "<=", ">="], &["+", "-"], &["*", "/"]];

    fn peek(&self) -> Option<&str> {
        self.t.get(self.i).map(|s| s.as_str())
    }

    fn bump(&mut self) -> Option<String> {
        self.i += 1;
        self.t.get(self.i - 1).cloned()
    }

    fn bin(&mut self, level: usize) -> Option<Ex> {
        if level == Self::LEVELS.len() {
            return self.unary();
        }
        let mut l = self.bin(level + 1)?;
        while let Some(op) = self.peek().and_then(|p| Self::LEVELS[level].iter().find(|o| **o == p).copied()) {
            self.i += 1;
            l = Ex::Bin(op, Box::new(l), Box::new(self.bin(level + 1)?));
        }
        Some(l)
    }

    fn unary(&mut self) -> Option<Ex> {
        match self.peek() {
            Some("-") => {
                self.i += 1;
                Some(Ex::Neg(Box::new(self.unary()?)))
            }
            Some("!") => {
                self.i += 1;
                Some(Ex::Not(Box::new(self.unary()?)))
            }
            _ => {
                let base = self.primary()?;
                if self.peek() == Some("^^") {
                    self.i += 1;
                    return Some(Ex::Bin("^^", Box::new(base), Box::new(self.unary()?)));
                }
                Some(base)
            }
        }
    }

    fn primary(&mut self) -> Option<Ex> {
        let t = self.bump()?;
        if t == "(" {
            let e = self.bin(0)?;
            return (self.bump()? == ")").then_some(e);
        }
        if let Ok(n) = t.parse::<f64>() {
            return Some(Ex::Num(n));
        }
        if !t.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_') {
            return None;
        }
        if self.peek() == Some("(") {
            self.i += 1;
            let name = self.bump()?;
            if self.bump()? != ")" {
                return None;
            }
            return match t.to_lowercase().as_str() {
                "attr" | "att" => Some(Ex::Attr(name)),
                "bb" => Some(Ex::Num(0.0)), // AI blackboard: never set on items
                _ => None,
            };
        }
        match t.to_lowercase().as_str() {
            "true" => Some(Ex::Num(1.0)),
            "false" => Some(Ex::Num(0.0)),
            _ => Some(Ex::Var(t)),
        }
    }
}

fn parse_expr(s: &str) -> Option<Ex> {
    let mut p = ExParser { t: tokenize(s)?, i: 0 };
    let e = p.bin(0)?;
    (p.i == p.t.len()).then_some(e)
}

/// Last segment of a class reference
/// ("Asset'/Script/GbxWeapon.WeaponBehavior_Fire'" -> "WeaponBehavior_Fire").
fn class_name(s: &str) -> &str {
    let s = unquote(s);
    s.rsplit(['.', '/']).next().unwrap_or(s)
}

/// Evaluates card attributes of one item from their definitions.
pub struct UiEval<'a> {
    db: &'a Db,
    c: &'a Computed,
    ev: Eval<'a>,
    cache: HashMap<String, Option<f64>>,
    /// behaviour each enclosing attribute resolves to (innermost last)
    ctx: Vec<String>,
}

impl<'a> UiEval<'a> {
    pub fn new(db: &'a Db, info: &ItemInfo, c: &'a Computed) -> Self {
        UiEval { db, c, ev: Eval::new(db, info.level.unwrap_or(1).max(1), info.rarity), cache: HashMap::new(), ctx: vec![] }
    }

    /// An attribute's card value (unresolvable reads 0).
    pub fn get(&mut self, name: &str) -> f64 {
        self.resolve(name).unwrap_or(0.0)
    }

    /// An attribute's card value; None when it does not resolve.
    pub fn resolve(&mut self, name: &str) -> Option<f64> {
        let k = name.to_lowercase();
        let def = self.db.attributes.get(&k);
        let beh = def
            .and_then(|d| d.get("context"))
            .and_then(|c| c.get("behaviortypetofurtherresolveto"))
            .and_then(|x| x.as_str())
            .map(class_name)
            .unwrap_or("")
            .to_string();
        // a specific behaviour is not re-resolved to an unrelated one
        // (a subclass, WeaponBehavior_FireProjectile of _Fire, is fine)
        let outer = self.ctx.iter().rev().find(|b| !b.is_empty() && b.as_str() != "WeaponBehavior");
        let reachable = beh.is_empty() || beh == "WeaponBehavior" || outer.map_or(true, |o| beh.starts_with(o.as_str()));
        let key = if reachable { k.clone() } else { format!("{k}@unreachable") };
        if let Some(v) = self.cache.get(&key) {
            return *v;
        }
        self.cache.insert(key.clone(), None); // cycle guard
        self.ctx.push(beh);
        let v = self.compute(&k, def, reachable);
        self.ctx.pop();
        self.cache.insert(key, v);
        v
    }

    fn compute(&mut self, k: &str, def: Option<&Value>, reachable: bool) -> Option<f64> {
        let v = def.and_then(|d| d.get("value"));
        let st = v.and_then(|v| v.get("structtype")).and_then(|x| x.as_str()).map(class_name).unwrap_or("");
        let stored = st == "PropertyValueResolver" || st == "InventoryStatsContainerValueResolver";
        if stored && !reachable {
            return Some(0.0);
        }
        // Int properties (MaxLoadedAmmo, ProjectilesPerShot) after float modifiers
        let int = v.and_then(|v| v.get("resolvedtype")).and_then(|x| x.as_str()) == Some("Int");
        let int = |x: f64| if int { (x - 1e-6).ceil() } else { x };
        if self.set(k) {
            return self.c.value(k).map(int);
        }
        if stored {
            // the same property under another attribute name
            // (weapon_auto_burst_count = weapon_burst_count = Fire.AutomaticBurstCount)
            if let Some(alias) = self.alias(k, def) {
                return self.c.value(&alias).map(int);
            }
            return Some(PROP_DEFAULTS.iter().find(|(n, _)| *n == k).map_or(0.0, |(_, d)| *d));
        }
        let v = v?;
        match st {
            // true when the item's part of that slot type has the given part value
            // (weapon_reload_four: Reload part value 4 = heat magazine)
            "GbxCondition_WeaponPartValue" => {
                let slot = v.get("type").and_then(|x| x.as_str())?.to_lowercase();
                let want = v.get("value").and_then(num)?;
                let have = self.c.value(&format!("weapon_part_{slot}_value"));
                Some(have.is_some_and(|h| (h - want).abs() < 1e-6) as u8 as f64)
            }
            "GbxExpressionValueResolver" => self.expr(v.get("expression")?),
            "GbxConditionalAttributeValueResolver" => {
                for cv in v.get("conditionalvalues").and_then(|x| x.as_array()).into_iter().flatten() {
                    if self.condition(cv.get("condition").unwrap_or(&Value::Null)) {
                        return Some(self.slot(cv.get("value").unwrap_or(&Value::Null)));
                    }
                }
                Some(v.get("defaultvalue").map_or(0.0, |d| self.slot(d)))
            }
            _ => self.ev.raw(v),
        }
    }

    /// Whether the item's effects or fire behaviour set the attribute.
    fn set(&self, k: &str) -> bool {
        self.c.base.contains_key(k) || self.c.effects.iter().any(|e| e.attr == k)
    }

    /// An attribute the item sets that names the same property of the same behaviour.
    fn alias(&self, k: &str, def: Option<&Value>) -> Option<String> {
        fn prop(d: &Value) -> Option<(String, &str)> {
            let p = d.get("value")?.get("property")?.get("propertypath")?.as_str()?.to_lowercase();
            let ctx = d.get("context").and_then(|c| c.get("behaviortypetofurtherresolveto")).and_then(|x| x.as_str()).unwrap_or("");
            Some((p, class_name(ctx)))
        }
        let want = prop(def?)?;
        let names = self.c.base.keys().map(|s| s.as_str()).chain(self.c.effects.iter().map(|e| e.attr.as_str()));
        names.filter(|n| *n != k).find(|n| self.db.attributes.get(*n).and_then(prop).as_ref() == Some(&want)).map(|s| s.to_string())
    }

    /// A value slot: {datatablevalue | attribute | constant, postscale}.
    fn slot(&mut self, v: &Value) -> f64 {
        let mut b = None;
        if let Some(dv) = v.get("datatablevalue") {
            if let Some(dt) = dv.get("datatable").and_then(|x| x.as_str()).map(unquote).filter(|d| !d.eq_ignore_ascii_case("none")) {
                let row = dv.get("rowname").and_then(|x| x.as_str()).unwrap_or("");
                let col = dv.get("columnname").and_then(|x| x.as_str()).filter(|c| !c.eq_ignore_ascii_case("none"));
                b = self.ev.cell(dt, row, col);
            }
        }
        if b.is_none() {
            if let Some(a) = v.get("attribute").and_then(|x| x.as_str()).map(unquote).filter(|a| !a.eq_ignore_ascii_case("none")) {
                b = Some(self.get(a));
            }
        }
        b.or_else(|| v.get("constant").and_then(num)).unwrap_or(0.0) * v.get("postscale").and_then(num).unwrap_or(1.0)
    }

    fn condition(&mut self, c: &Value) -> bool {
        if let Some(ex) = c.get("inlinestruct").and_then(|s| s.get("expression")) {
            return self.expr(ex).is_some_and(|x| x != 0.0);
        }
        if let Some(a) = c.get("externalattribute").and_then(|x| x.as_str()) {
            return self.get(unquote(a)) != 0.0;
        }
        false
    }

    /// GbxExpressionValueResolver: a formula string, or {formula, variables}.
    fn expr(&mut self, ex: &Value) -> Option<f64> {
        let text = ex.as_str().or_else(|| ex.get("formula").and_then(|f| f.as_str()))?;
        let mut vars = HashMap::new();
        let pairs = ex.get("variables").and_then(|v| v.get("variablevalues")).and_then(|v| v.get("pairs")).and_then(|p| p.as_object());
        for p in pairs.into_iter().flat_map(|m| m.values()) {
            if let (Some(k), Some(val)) = (p.get("key").and_then(|x| x.as_str()), p.get("value").and_then(|v| v.get("value"))) {
                vars.insert(k.to_lowercase(), val.clone());
            }
        }
        let tree = parse_expr(text)?;
        Some(self.eval(&tree, &vars))
    }

    /// A formula variable: {type: Attribute | DataTable | Float | Int | Bool, value}.
    fn var(&mut self, name: &str, vars: &HashMap<String, Value>) -> f64 {
        let Some(val) = vars.get(&name.to_lowercase()) else { return self.get(name) };
        let inner = val.get("value");
        match val.get("type").and_then(|x| x.as_str()) {
            Some("Attribute") => inner.and_then(|x| x.as_str()).map_or(0.0, |a| self.get(unquote(a))),
            Some("DataTable") => inner
                .and_then(|d| self.ev.cell(unquote(d.get("datatable")?.as_str()?), d.get("rowname")?.as_str()?, d.get("columnname").and_then(|x| x.as_str())))
                .unwrap_or(0.0),
            Some("Bool") => (inner.and_then(|x| x.as_str()) == Some("true")) as u8 as f64,
            _ => inner.and_then(num).unwrap_or(0.0),
        }
    }

    fn eval(&mut self, e: &Ex, vars: &HashMap<String, Value>) -> f64 {
        let t = |c: bool| c as u8 as f64;
        match e {
            Ex::Num(n) => *n,
            Ex::Var(v) => self.var(v, vars),
            Ex::Attr(a) => self.get(a),
            Ex::Neg(x) => -self.eval(x, vars),
            Ex::Not(x) => t(self.eval(x, vars) == 0.0),
            Ex::Bin(op, l, r) => {
                let (a, b) = (self.eval(l, vars), self.eval(r, vars));
                match *op {
                    "+" => a + b,
                    "-" => a - b,
                    "*" => a * b,
                    "/" if b != 0.0 => a / b,
                    "/" => 0.0,
                    "^^" => a.powf(b),
                    "<" => t(a < b),
                    ">" => t(a > b),
                    "<=" => t(a <= b),
                    ">=" => t(a >= b),
                    "==" => t((a - b).abs() < 1e-9),
                    "!=" => t((a - b).abs() >= 1e-9),
                    "&&" => t(a != 0.0 && b != 0.0),
                    "||" => t(a != 0.0 || b != 0.0),
                    _ => 0.0,
                }
            }
        }
    }
}
