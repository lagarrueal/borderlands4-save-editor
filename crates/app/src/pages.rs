//! Character, currency, progression, missions, appearance and raw pages.

use crate::app::{class_label, App, Which};
use bl4core::db::Db;
use bl4core::item::ItemInfo;
use bl4core::save::{self, Character, Profile};
use bl4core::serial::Serial;
use bl4core::yaml::{self, Node};
use eframe::egui::{self, Color32, RichText};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct PagesState {
    raw_text: String,
    raw_which: Option<bool>,
    raw_dirty: bool,
    raw_search: String,
    mission_filter: String,
    show_all_sets: bool,
    cosmetic_filter: String,
    include_premium: bool,
    uvh_target: u32,
    show_respawn: bool,
    name_buf: Option<String>,
}

fn doc_clone(app: &App, w: Which) -> Option<Node> {
    app.sess.as_ref().and_then(|s| s.doc(w)).cloned()
}

fn big_number(ui: &mut egui::Ui, label: &str, value: i64, max: i64) -> Option<i64> {
    let mut v = value;
    let mut out = None;
    ui.label(label);
    if ui.add(egui::DragValue::new(&mut v).range(0..=max).speed(1000.0)).changed() {
        out = Some(v);
    }
    out
}

// ====================================================================== character

pub fn character(app: &mut App, ui: &mut egui::Ui) {
    let Some(mut doc) = doc_clone(app, Which::Character) else { return };
    let c = Character(&mut doc);
    let name = c.name();
    let class = c.class();
    let (lvl, xp) = c.xp("Character").unwrap_or((1, 0));
    let spec = c.xp("Specialization");
    let diff = c.difficulty();
    let tm = c.true_mode();
    egui::Grid::new("char").num_columns(2).spacing([20.0, 8.0]).show(ui, |ui| {
        ui.label("Name");
        let buf = app.pages.name_buf.get_or_insert_with(|| name.clone());
        let r = ui.add(egui::TextEdit::singleline(buf).desired_width(240.0));
        if r.lost_focus() && *buf != name && !buf.trim().is_empty() {
            let n = buf.trim().to_string();
            app.edit(Which::Character, |d| Character(d).set_name(&n));
        }
        ui.end_row();
        ui.label("Class");
        ui.label(format!("{} ({class})", class_label(&class)));
        ui.end_row();
        ui.label("Level");
        ui.horizontal(|ui| {
            let mut l = lvl;
            if ui.add(egui::DragValue::new(&mut l).range(1..=save::MAX_CHAR_LEVEL)).changed() {
                app.edit(Which::Character, |d| {
                    let _ = Character(d).set_level("Character", l);
                });
            }
            if ui.button(format!("Max ({})", save::MAX_CHAR_LEVEL)).clicked() {
                app.edit(Which::Character, |d| {
                    let _ = Character(d).set_level("Character", save::MAX_CHAR_LEVEL);
                });
            }
            ui.label(RichText::new(format!("XP {xp}")).weak());
        });
        ui.end_row();
        ui.label("Specialization level");
        ui.horizontal(|ui| match spec {
            Some((sl, sxp)) => {
                let mut l = sl;
                if ui.add(egui::DragValue::new(&mut l).range(1..=save::MAX_SPEC_LEVEL)).changed() {
                    app.edit(Which::Character, |d| {
                        let _ = Character(d).set_level("Specialization", l);
                    });
                }
                ui.label(RichText::new(format!("XP {sxp}")).weak());
            }
            None => {
                ui.label(RichText::new("locked (unlocks after the story)").weak());
                if ui.button("Unlock").clicked() {
                    app.edit(Which::Character, |d| {
                        let _ = Character(d).set_level("Specialization", 1);
                    });
                }
            }
        });
        ui.end_row();
        ui.label("Difficulty");
        ui.horizontal(|ui| {
            for d in ["Easy", "Normal", "Hard"] {
                if ui.selectable_label(diff == d, d).clicked() && diff != d {
                    app.edit(Which::Character, |doc| Character(doc).set_difficulty(d));
                }
            }
        });
        ui.end_row();
        ui.label("True Mode");
        let mut t = tm;
        if ui.checkbox(&mut t, "Enemies scale like a 4-player party").changed() {
            app.edit(Which::Character, |d| Character(d).set_true_mode(t));
        }
        ui.end_row();
        let pt = doc.get("state.total_playtime").and_then(|n| n.as_f64()).unwrap_or(0.0);
        ui.label("Play time");
        ui.label(format!("{}h {:02}m", (pt / 3600.0) as u64, ((pt % 3600.0) / 60.0) as u64));
        ui.end_row();
        ui.label("Character GUID");
        ui.label(RichText::new(doc.get("state.char_guid").and_then(|n| n.as_str()).unwrap_or("")).monospace().weak());
        ui.end_row();
        ui.label("Checkpoint (spawn point)");
        let cur = doc.get("state.checkpoint_name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        ui.horizontal(|ui| {
            let mut chosen = None;
            egui::ComboBox::from_id_salt("checkpoint").width(380.0).selected_text(if cur.is_empty() { "-" } else { cur.as_str() }).show_ui(ui, |ui| {
                for st in app.db.stations.iter().filter(|s| app.pages.show_respawn || s.r#type == "fast_travel") {
                    if ui.selectable_label(st.cp == cur, &st.cp).clicked() {
                        chosen = Some(st.cp.clone());
                    }
                }
            });
            ui.checkbox(&mut app.pages.show_respawn, "include respawn points");
            if let Some(c) = chosen {
                if c != cur {
                    app.edit(Which::Character, |d| d.set_scalar("state.checkpoint_name", c.clone()));
                }
            }
        });
        ui.end_row();
    });
    ui.add_space(10.0);
    ui.label(RichText::new("Level edits write the level, the matching minimum XP and skill points together, exactly as the game stores them. Max character level is 60.").weak());
}

// ====================================================================== currency

pub fn currency(app: &mut App, ui: &mut egui::Ui) {
    if let Some(mut doc) = doc_clone(app, Which::Character) {
        let c = Character(&mut doc);
        ui.label(RichText::new("Character").strong().size(16.0));
        egui::Grid::new("money").num_columns(3).spacing([16.0, 8.0]).show(ui, |ui| {
            for (k, label, max) in [("cash", "Cash", 2_000_000_000i64), ("eridium", "Eridium", 2_000_000_000)] {
                let v = c.currency(k).and_then(|s| s.parse().ok()).unwrap_or(0);
                if let Some(n) = big_number(ui, label, v, max) {
                    app.edit(Which::Character, |d| Character(d).set_currency(k, n));
                }
                if ui.button("Set 99,999,999").clicked() {
                    app.edit(Which::Character, |d| Character(d).set_currency(k, 99_999_999));
                }
                ui.end_row();
            }
            ui.label("Golden Keys");
            ui.label(RichText::new(c.currency("golden_key").unwrap_or_default()).weak())
                .on_hover_text("Golden keys are held by SHiFT (server side) and cannot be edited offline.");
            ui.label(RichText::new("managed by SHiFT - not editable").weak());
            ui.end_row();
        });
        ui.add_space(8.0);
        ui.label(RichText::new("Ammo").strong().size(16.0));
        let maxes = [("pistol", 900), ("smg", 1620), ("assaultrifle", 1260), ("shotgun", 220), ("sniper", 190), ("repairkit", 10)];
        let ammo = c.ammo();
        egui::Grid::new("ammo").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            for (k, v) in &ammo {
                ui.label(k);
                let mut n = *v;
                let max = maxes.iter().find(|m| m.0 == k).map(|m| m.1).unwrap_or(9999);
                if ui.add(egui::DragValue::new(&mut n).range(0..=max)).changed() {
                    let k2 = k.clone();
                    app.edit(Which::Character, |d| Character(d).set_ammo(&k2, n));
                }
                ui.end_row();
            }
        });
        if ui.button("Fill all ammo (max with every SDU)").clicked() {
            app.edit(Which::Character, |d| {
                for (k, m) in maxes {
                    Character(d).set_ammo(k, m);
                }
            });
        }
    }
    if let Some(mut pdoc) = doc_clone(app, Which::Profile) {
        ui.add_space(10.0);
        ui.label(RichText::new("Account (profile.sav)").strong().size(16.0));
        let p = Profile(&mut pdoc);
        egui::Grid::new("acct").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
            if let Some(n) = big_number(ui, "SDU tokens", p.sdu_tokens(), 1_000_000) {
                app.edit(Which::Profile, |d| Profile(d).set_sdu_tokens(n));
            }
            ui.end_row();
            for (k, v) in p.vault_card_tokens() {
                let label = k.replace("vaultcard0", "Vault Card ").replace("_tokens", " tokens");
                if let Some(n) = big_number(ui, &label, v, 1_000_000) {
                    app.edit(Which::Profile, |d| Profile(d).set_shared_currency(&k, n));
                }
                ui.end_row();
            }
        });
    }
}

// ====================================================================== UVH and SDU

pub fn progression(app: &mut App, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(mut doc) = doc_clone(app, Which::Character) {
            let c = Character(&mut doc);
            let (max, active) = c.uvh();
            let story = c.story_complete();
            ui.label(RichText::new("Ultimate Vault Hunter").strong().size(16.0));
            ui.label(format!("Unlocked: UVH {max} · Active: {}", if active == 0 { "off".to_string() } else { format!("UVH {active}") }));
            if !story {
                ui.label(RichText::new("The story is not finished on this character: UVH normally needs it. Use Missions > Complete main story first (recommended).").color(Color32::from_rgb(255, 190, 60)));
            }
            if app.pages.uvh_target == 0 {
                app.pages.uvh_target = max.max(1);
            }
            ui.horizontal(|ui| {
                ui.label("Unlock up to UVH");
                ui.add(egui::DragValue::new(&mut app.pages.uvh_target).range(1..=7));
                if ui.button("Unlock").clicked() {
                    let t = app.pages.uvh_target;
                    app.edit(Which::Character, |d| Character(d).unlock_uvh(t, active.min(t)));
                    if app.sess.as_ref().map(|s| s.profile.is_some()).unwrap_or(false) {
                        app.edit(Which::Profile, |d| Profile(d).add_shared_progress("shared_progress.vault_hunter_level"));
                    }
                    app.info(format!("UVH 1-{t} unlocked (rank-up challenges marked complete)"));
                }
            });
            ui.horizontal(|ui| {
                ui.label("Active UVH level:");
                for l in 0..=max {
                    let label = if l == 0 { "Off".to_string() } else { l.to_string() };
                    if ui.selectable_label(active == l, label).clicked() && active != l {
                        app.edit(Which::Character, |d| d.set_scalar("globals.vault_hunter_level", l.to_string()));
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(RichText::new("Equipment slots").strong().size(16.0));
            let unlocked: Vec<i64> = doc
                .get("state.inventory.equip_slots_unlocked")
                .and_then(|n| n.as_seq())
                .map(|s| s.items.iter().filter_map(|n| n.as_i64()).collect())
                .unwrap_or_default();
            ui.label(format!(
                "Unlocked optional slots: {}",
                unlocked.iter().map(|s| crate::items::equip_slot_label(*s as usize)).collect::<Vec<_>>().join(", ")
            ));
            if ui.button("Unlock weapon slots 3-4, repkit, enhancement and class mod slots").clicked() {
                app.edit(Which::Character, |d| Character(d).unlock_equip_slots());
            }
        }
        if let Some(mut pdoc) = doc_clone(app, Which::Profile) {
            ui.add_space(12.0);
            ui.label(RichText::new("SDU upgrades (account-wide)").strong().size(16.0));
            let p = Profile(&mut pdoc);
            let bought: Vec<String> = p.sdu_nodes().into_iter().map(|x| x.0).collect();
            let tokens = p.sdu_tokens();
            let all: Vec<(String, u32, String)> = app.db.sdu.iter().map(|n| (n.name.clone(), n.cost, n.effect.clone())).collect();
            let spent: u32 = all.iter().filter(|n| bought.contains(&n.0)).map(|n| n.1).sum();
            ui.label(format!("SDU tokens: {tokens} earned, {spent} spent on {} of {} upgrades", bought.len(), all.len()));
            if ui.button("Buy every SDU upgrade").clicked() {
                let nodes: Vec<(String, u32)> = all.iter().map(|n| (n.0.clone(), n.1)).collect();
                app.edit(Which::Profile, |d| Profile(d).set_sdu_nodes(&nodes));
                app.info("All SDU upgrades purchased");
            }
            let mut groups: BTreeMap<String, Vec<&(String, u32, String)>> = BTreeMap::new();
            for n in &all {
                let g = n.0.rsplit_once('_').map(|x| x.0.to_string()).unwrap_or_default();
                groups.entry(g).or_default().push(n);
            }
            egui::Grid::new("sdu").num_columns(3).spacing([16.0, 4.0]).show(ui, |ui| {
                for (g, nodes) in &groups {
                    let have = nodes.iter().filter(|n| bought.contains(&n.0)).count();
                    ui.label(g.replace('_', " "));
                    let mut target = have as u32;
                    if ui.add(egui::Slider::new(&mut target, 0..=nodes.len() as u32).text("levels")).changed() {
                        let mut keep: Vec<(String, u32)> = all
                            .iter()
                            .filter(|n| bought.contains(&n.0) && !nodes.iter().any(|x| x.0 == n.0))
                            .map(|n| (n.0.clone(), n.1))
                            .collect();
                        for n in nodes.iter().take(target as usize) {
                            keep.push((n.0.clone(), n.1));
                        }
                        app.edit(Which::Profile, |d| Profile(d).set_sdu_nodes(&keep));
                    }
                    ui.label(RichText::new(nodes.first().map(|n| n.2.clone()).unwrap_or_default()).weak().small());
                    ui.end_row();
                }
            });
        }
    });
}

// ====================================================================== missions

pub fn missions(app: &mut App, ui: &mut egui::Ui) {
    let Some(doc) = doc_clone(app, Which::Character) else { return };
    let sets = save::mission_sets(&doc);
    ui.horizontal(|ui| {
        if ui.button("Complete main story").on_hover_text("Marks every main mission set completed like the game's own story skip, and sets the story progress flags").clicked() {
            let sm = app.set_missions.clone();
            app.edit(Which::Character, |d| save::complete_story(d, &sm));
            if app.sess.as_ref().map(|s| s.profile.is_some()).unwrap_or(false) {
                app.edit(Which::Profile, |d| {
                    let mut p = Profile(d);
                    p.add_shared_progress("shared_progress.prologue_completed");
                    p.add_shared_progress("shared_progress.story_completed");
                });
            }
            app.info("Main story completed");
        }
        ui.separator();
        ui.label("Filter:");
        ui.add(egui::TextEdit::singleline(&mut app.pages.mission_filter).desired_width(160.0));
        ui.checkbox(&mut app.pages.show_all_sets, "Show sets not started yet");
    });
    ui.separator();
    let filter = app.pages.mission_filter.to_lowercase();
    let mut names: Vec<String> = sets.iter().map(|s| s.0.clone()).collect();
    if app.pages.show_all_sets {
        let mut extra: Vec<String> = app.set_missions.keys().filter(|k| !names.contains(k)).cloned().collect();
        extra.sort();
        names.extend(extra);
    }
    // main story order first
    names.sort_by_key(|n| (save::MAIN_STORY.iter().position(|m| m == n).unwrap_or(999), n.clone()));
    egui::ScrollArea::vertical().show(ui, |ui| {
        for set in names {
            let st = sets.iter().find(|s| s.0 == set);
            let status = st.map(|s| s.1.clone()).unwrap_or_else(|| "not started".into());
            let pretty = set_title(&app.db, &app.set_missions, &set);
            if !filter.is_empty() && !format!("{set} {pretty}").to_lowercase().contains(&filter) {
                continue;
            }
            let color = match status.as_str() {
                "completed" => Color32::from_rgb(90, 200, 90),
                "not started" => Color32::from_gray(140),
                _ => Color32::from_rgb(255, 190, 60),
            };
            let header = RichText::new(format!("{pretty}  [{status}]")).color(color);
            egui::CollapsingHeader::new(header).id_salt(&set).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&set).monospace().weak());
                    if status != "completed" && ui.button("Complete set").clicked() {
                        let ms = app.set_missions.get(&set).cloned().unwrap_or_default();
                        let s2 = set.clone();
                        app.edit(Which::Character, |d| save::complete_set(d, &s2, &ms));
                    }
                    if st.is_some() && ui.button("Reset set").on_hover_text("Removes this set's progress (risky for story sets: world changes stay)").clicked() {
                        let s2 = set.clone();
                        app.edit(Which::Character, |d| save::reset_set(d, &s2));
                    }
                });
                let in_save: Vec<save::MissionState> = st.map(|s| s.2.clone()).unwrap_or_default();
                let mut all_m: Vec<String> = app.set_missions.get(&set).cloned().unwrap_or_default();
                for m in &in_save {
                    if !all_m.contains(&m.mission) {
                        all_m.push(m.mission.clone());
                    }
                }
                for m in all_m {
                    let ms = in_save.iter().find(|x| x.mission == m);
                    let mstatus = ms.map(|x| x.status.clone()).unwrap_or_else(|| "-".into());
                    let mname = app.db.missions.get(&m).map(|x| x.name.clone()).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{mname}")).strong());
                        ui.label(RichText::new(&m).weak().small());
                        ui.label(&mstatus);
                        if mstatus != "completed" && ui.small_button("Complete").clicked() {
                            let (s2, m2) = (set.clone(), m.clone());
                            app.edit(Which::Character, |d| save::complete_mission(d, &s2, &m2));
                        }
                    });
                    if let Some(ms) = ms {
                        for (obj, ost) in &ms.objectives {
                            ui.horizontal(|ui| {
                                ui.add_space(24.0);
                                ui.label(RichText::new(obj).small());
                                let id = ui.id().with((&set, &m, obj));
                                let mut chosen = None;
                                egui::ComboBox::from_id_salt(id).selected_text(ost).show_ui(ui, |ui| {
                                    for s in ["WaitingOnDependencies", "WaitingOnMission", "Active", "Completed_Finishing", "Completed_PostFinished"] {
                                        if ui.selectable_label(ost == s, s).clicked() {
                                            chosen = Some(s);
                                        }
                                    }
                                });
                                if let Some(s) = chosen {
                                    let (s2, m2, o2) = (set.clone(), m.clone(), obj.clone());
                                    app.edit(Which::Character, |d| save::set_objective(d, &s2, &m2, &o2, s));
                                }
                            });
                        }
                    }
                }
            });
        }
    });
}

fn set_title(db: &Db, set_missions: &std::collections::HashMap<String, Vec<String>>, set: &str) -> String {
    let names: Vec<String> = set_missions
        .get(set)
        .map(|ms| ms.iter().filter_map(|m| db.missions.get(m)).map(|x| x.name.clone()).filter(|n| !n.is_empty()).collect())
        .unwrap_or_default();
    let base = set.trim_start_matches("missionset_").replace('_', " ");
    match names.first() {
        Some(n) if names.len() == 1 => format!("{n} ({base})"),
        Some(n) => format!("{n} +{} ({base})", names.len() - 1),
        None => base,
    }
}

// ====================================================================== appearance

const PREMIUM_MARKERS: &[&str] = &["preorder", "premium", "goldenpower", "shift", "headhunter", "legacy", "twitch", "prime", "deluxe", "bonus"];

pub fn appearance(app: &mut App, ui: &mut egui::Ui) {
    let prof = doc_clone(app, Which::Profile);
    let unlocked: Vec<(String, Vec<String>)> = prof.clone().map(|mut p| Profile(&mut p).unlockables()).unwrap_or_default();
    let is_unlocked = |id: &str| unlocked.iter().any(|(_, l)| l.iter().any(|x| x == id));
    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(mut doc) = doc_clone(app, Which::Character) {
            let c = Character(&mut doc);
            let class = c.class().trim_start_matches("Char_").to_lowercase();
            let eq = c.equipped_cosmetics();
            ui.label(RichText::new("Equipped on this character").strong().size(16.0));
            // slots: from the catalogue entries for this class, echo4 and vehicles
            let mut slots: BTreeMap<(String, String), Vec<(String, String, String)>> = BTreeMap::new();
            for (id, cm) in &app.db.cosmetics {
                let Some(part) = &cm.part else { continue };
                let g = cm.group.to_lowercase();
                let relevant = g == format!("unlockable_{class}") || g == "unlockable_echo4" || g == "unlockable_vehicles";
                if !relevant {
                    continue;
                }
                if let Some((group, slot)) = save::cosmetic_slot(part) {
                    slots.entry((group.to_string(), slot)).or_default().push((part.clone(), cm.name.clone().unwrap_or_default(), id.clone()));
                }
            }
            egui::Grid::new("equipped_cos").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
                for ((group, slot), mut opts) in slots {
                    opts.sort_by(|a, b| a.1.cmp(&b.1));
                    let cur = eq.iter().find(|(g, s, _)| *g == group && s.eq_ignore_ascii_case(&slot)).map(|x| x.2.clone());
                    // keep the casing the save already uses for this slot
                    let slot_key = eq.iter().find(|(g, s, _)| *g == group && s.eq_ignore_ascii_case(&slot)).map(|x| x.1.clone()).unwrap_or(slot.clone());
                    ui.label(slot.trim_start_matches("Cosmetics_").replace('_', " "));
                    let cur_lbl = cur
                        .as_ref()
                        .map(|p| opts.iter().find(|o| &o.0 == p).map(|o| o.1.clone()).unwrap_or(p.clone()))
                        .unwrap_or_else(|| "default".into());
                    let mut chosen: Option<Option<String>> = None;
                    egui::ComboBox::from_id_salt(("cos", &group, &slot)).width(300.0).selected_text(cur_lbl).show_ui(ui, |ui| {
                        if ui.selectable_label(cur.is_none(), "default").clicked() {
                            chosen = Some(None);
                        }
                        for (part, name, id) in &opts {
                            let ok = is_unlocked(id);
                            let label = format!("{}{}", if name.is_empty() { part } else { name }, if ok { "" } else { "  (locked)" });
                            if ui.add_enabled(ok, egui::Button::selectable(cur.as_ref() == Some(part), label)).clicked() {
                                chosen = Some(Some(part.clone()));
                            }
                        }
                    });
                    if let Some(ch) = chosen {
                        let (g2, s2) = (group.clone(), slot_key.clone());
                        app.edit(Which::Character, |d| Character(d).set_cosmetic(&g2, &s2, ch.as_deref()));
                    }
                    ui.end_row();
                }
            });
            ui.add_space(12.0);
        }
        if prof.is_none() {
            return;
        }
        ui.label(RichText::new("Unlocked cosmetics (account-wide)").strong().size(16.0));
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.pages.include_premium, "Include pre-order / premium / SHiFT items")
                .on_hover_text("These normally come from purchases or promotions");
            ui.label("Filter:");
            ui.add(egui::TextEdit::singleline(&mut app.pages.cosmetic_filter).desired_width(150.0));
        });
        let mut by_group: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
        for (id, cm) in &app.db.cosmetics {
            let g = cm.group.to_lowercase();
            if g.ends_with("_character") {
                continue; // per-character unlocks live in the character save
            }
            by_group.entry(g).or_default().push((id.clone(), cm.name.clone().unwrap_or_default()));
        }
        let premium = |id: &str| {
            let l = id.to_lowercase();
            PREMIUM_MARKERS.iter().any(|m| l.contains(m))
        };
        if ui.button("Unlock everything shown").clicked() {
            let inc = app.pages.include_premium;
            let groups = by_group.clone();
            let n = {
                let mut total = 0;
                app.edit(Which::Profile, |d| {
                    let mut p = Profile(d);
                    for (g, items) in &groups {
                        let ids: Vec<String> = items.iter().map(|x| x.0.clone()).filter(|id| inc || !premium(id)).collect();
                        total += p.add_unlockables(g, &ids);
                    }
                });
                total
            };
            app.info(format!("{n} cosmetics unlocked"));
        }
        let filter = app.pages.cosmetic_filter.to_lowercase();
        for (g, mut items) in by_group {
            items.sort_by(|a, b| a.1.cmp(&b.1));
            let have = items.iter().filter(|x| is_unlocked(&x.0)).count();
            egui::CollapsingHeader::new(format!("{}  {have}/{}", g.trim_start_matches("unlockable_"), items.len())).id_salt(&g).show(ui, |ui| {
                let inc = app.pages.include_premium;
                if ui.button("Unlock all in this group").clicked() {
                    let ids: Vec<String> = items.iter().map(|x| x.0.clone()).filter(|id| inc || !premium(id)).collect();
                    let g2 = g.clone();
                    app.edit(Which::Profile, |d| {
                        Profile(d).add_unlockables(&g2, &ids);
                    });
                }
                egui::Grid::new(("cosg", &g)).num_columns(2).show(ui, |ui| {
                    for (id, name) in &items {
                        if !filter.is_empty() && !format!("{id} {name}").to_lowercase().contains(&filter) {
                            continue;
                        }
                        let mut on = is_unlocked(id);
                        if ui.checkbox(&mut on, if name.is_empty() { id.as_str() } else { name.as_str() }).changed() && on {
                            let (g2, id2) = (g.clone(), id.clone());
                            app.edit(Which::Profile, |d| {
                                Profile(d).add_unlockables(&g2, &[id2]);
                            });
                        }
                        ui.label(RichText::new(id).weak().small());
                        ui.end_row();
                    }
                });
            });
        }
    });
}

// ====================================================================== raw

pub fn raw(app: &mut App, ui: &mut egui::Ui) {
    let has_char = app.sess.as_ref().map(|s| s.character.is_some()).unwrap_or(false);
    let mut use_char = app.pages.raw_which.unwrap_or(has_char);
    ui.horizontal(|ui| {
        if has_char && ui.selectable_label(use_char, "Character").clicked() {
            use_char = true;
        }
        if app.sess.as_ref().map(|s| s.profile.is_some()).unwrap_or(false) && ui.selectable_label(!use_char, "Profile").clicked() {
            use_char = false;
        }
        ui.separator();
        ui.label("Find:");
        ui.add(egui::TextEdit::singleline(&mut app.pages.raw_search).desired_width(200.0));
    });
    let w = if use_char { Which::Character } else { Which::Profile };
    if app.pages.raw_which != Some(use_char) || (!app.pages.raw_dirty && app.pages.raw_text.is_empty()) {
        app.pages.raw_which = Some(use_char);
        app.pages.raw_text = app.sess.as_ref().and_then(|s| s.doc(w)).map(|d| d.emit()).unwrap_or_default();
        app.pages.raw_dirty = false;
    }
    ui.label(RichText::new("Advanced: the decrypted save as YAML. Edit with care; Apply checks that it still parses.").weak());
    ui.horizontal(|ui| {
        if ui.add_enabled(app.pages.raw_dirty, egui::Button::new("Apply YAML")).clicked() {
            match yaml::parse(&app.pages.raw_text) {
                Ok(n) => {
                    app.edit(w, |d| *d = n);
                    app.pages.raw_dirty = false;
                    app.items.invalidate();
                    app.info("Raw YAML applied");
                }
                Err(e) => app.error(format!("YAML error: {e}")),
            }
        }
        if ui.button("Reload").clicked() {
            app.pages.raw_text.clear();
            app.pages.raw_dirty = false;
        }
        if !app.pages.raw_search.is_empty() {
            let n = app.pages.raw_text.matches(&app.pages.raw_search).count();
            ui.label(format!("{n} matches"));
        }
    });
    egui::ScrollArea::vertical().show(ui, |ui| {
        let r = ui.add(egui::TextEdit::multiline(&mut app.pages.raw_text).code_editor().desired_width(f32::INFINITY));
        if r.changed() {
            app.pages.raw_dirty = true;
        }
    });
}

// ====================================================================== stats

/// Item card numbers. Filled in by the stats engine.
pub fn item_stats(db: &Db, serial: &Serial, info: &ItemInfo) -> Vec<(String, String)> {
    bl4core::stats::card(db, serial, info)
}
