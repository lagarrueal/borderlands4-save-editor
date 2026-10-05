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
        add_source(&mut ev, db, &mut out, b, &format!("type {key}"));
    }
    for r in s.parts() {
        if let Some(p) = db.part(r) {
            // fire behaviour of the primary mode comes from the barrel only;
            // underbarrels carry their own behaviour for the secondary mode
            let beh = if p.s == "barrel" { p.beh.clone() } else { None };
            let v = serde_json::json!({
                "fx": p.fx, "tpl": p.tpl, "beh": beh, "asp": p.asp,
            });
            add_source(&mut ev, db, &mut out, &v, &p.k);
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

    /// Magazine size (MaxLoadedAmmo is an int, the stat rounds up).
    pub fn card_magazine(&self) -> Option<f64> {
        self.based_value("weapon_max_loaded_ammo").map(|v| (v - 1e-6).ceil())
    }

    /// Sustained DPS as the card shows it: one magazine at the card fire rate,
    /// then a reload.
    pub fn card_dps(&self) -> Option<f64> {
        let dmg = self.value("weapon_damage")?;
        let pellets = self.value("weapon_projectile_per_shot").unwrap_or(1.0).max(1.0).round();
        let fr = self.card_fire_rate()?;
        let mag = self.card_magazine()?;
        let reload = self.based_value("weapon_reload_time").unwrap_or(0.0);
        if fr <= 0.0 || mag <= 0.0 {
            return None;
        }
        Some(dmg * pellets * mag / (mag / fr + reload))
    }
}

fn add_source(ev: &mut Eval, db: &Db, out: &mut Computed, src: &Value, label: &str) {
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
                for e in fx.iter().filter(|e| mode_ok(e) && (!template || e.get("dt").is_none())) {
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
        ] {
            if let Some(v) = beh.get(k).and_then(|v| ev.compact(v)) {
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
            if let Some(v) = c.card_fire_rate() {
                push(&mut out, "Fire rate", format!("{:.1}/s", v));
            }
            if let Some(v) = c.card_magazine() {
                push(&mut out, "Magazine", format!("{:.0}", v));
            }
            if let Some(v) = c.based_value("weapon_reload_time") {
                push(&mut out, "Reload time", format!("{:.1}s", v));
            }
            if let Some(v) = c.card_dps() {
                push(&mut out, "DPS", thousands(v));
            }
            if let Some(v) = c.value("weapon_spread") {
                push(&mut out, "Spread", fmt_num(v));
            }
            if let Some(v) = c.value("weapon_damage_modifier_add_critical_hit") {
                if v.abs() > 1e-6 {
                    push(&mut out, "Crit damage", format!("{:+.0}%", v * 100.0));
                }
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
