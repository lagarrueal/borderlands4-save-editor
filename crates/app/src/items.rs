//! Backpack / bank / lost-loot views and the item editor.

use crate::app::{App, Which};
use bl4core::db::{kind_label, Db};
use bl4core::item::{self, ItemInfo, Rarity, Severity};
use bl4core::save::{self, Container, InvItem};
use bl4core::serial::{PartRef, Serial};
use eframe::egui::{self, Color32, RichText};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Backpack,
    LostLoot,
    Bank,
}

pub struct Analyzed {
    pub serial: Option<Serial>,
    pub info: Option<ItemInfo>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct ItemsState {
    pub sel: Option<String>,
    pub select_index: Option<usize>,
    pub filter: String,
    cache: HashMap<String, Rc<Analyzed>>,
    serial_text: String,
    serial_for: String,
    paste: String,
    add_slot: String,
    add_part: Option<PartRef>,
    new_cat: Option<u32>,
    new_comp: String,
    new_level: u32,
    part_filter: String,
}

impl ItemsState {
    pub fn invalidate(&mut self) {
        self.serial_for.clear();
    }
    pub fn analyze(&mut self, db: &Db, serial: &str) -> Rc<Analyzed> {
        if let Some(a) = self.cache.get(serial) {
            return a.clone();
        }
        let a = match Serial::decode(serial) {
            Ok(s) => {
                let info = item::analyze(db, &s);
                Analyzed { serial: Some(s), info: Some(info), error: None }
            }
            Err(e) => Analyzed { serial: None, info: None, error: Some(e.to_string()) },
        };
        let a = Rc::new(a);
        if self.cache.len() > 5000 {
            self.cache.clear();
        }
        self.cache.insert(serial.to_string(), a.clone());
        a
    }
}

pub fn rarity_color(r: Rarity) -> Color32 {
    let [a, b, c] = r.rgb();
    Color32::from_rgb(a, b, c)
}

pub fn equip_slot_label(n: usize) -> &'static str {
    match n {
        0 => "Weapon 1",
        1 => "Weapon 2",
        2 => "Weapon 3",
        3 => "Weapon 4",
        4 => "Shield",
        5 => "Ordnance",
        6 => "Repkit",
        7 => "Enhancement",
        8 => "Class Mod",
        _ => "Equipped",
    }
}

fn marker(info: Option<&ItemInfo>) -> (&'static str, Color32, String) {
    match info {
        None => ("✖", Color32::from_rgb(255, 80, 80), "Serial cannot be decoded".into()),
        Some(i) => match i.issues.iter().map(|x| x.sev).max() {
            Some(Severity::Error) => ("✖", Color32::from_rgb(255, 80, 80), "The game will reject this item".into()),
            Some(Severity::Warning) => ("⚠", Color32::from_rgb(255, 190, 60), "Not obtainable in normal play".into()),
            _ => ("✔", Color32::from_rgb(90, 200, 90), "Valid".into()),
        },
    }
}

pub fn inventory(app: &mut App, ui: &mut egui::Ui, view: View) {
    let which = if view == View::Bank { Which::Profile } else { Which::Character };
    let Some(doc) = app.sess.as_ref().and_then(|s| s.doc(which)) else {
        ui.label("This save does not contain that inventory.");
        return;
    };
    let all = save::list_items(doc);
    let equipped: HashMap<String, usize> = all
        .iter()
        .filter(|i| i.container == Container::Equipped)
        .map(|i| (i.serial.clone(), i.slot))
        .collect();
    let mut list: Vec<InvItem> = match view {
        View::Backpack => all.iter().filter(|i| matches!(i.container, Container::Backpack | Container::Unknown)).cloned().collect(),
        View::LostLoot => all.iter().filter(|i| i.container == Container::LostLoot).cloned().collect(),
        View::Bank => all.iter().filter(|i| i.container == Container::Bank).cloned().collect(),
    };
    // equipped first, ordered by equip slot
    list.sort_by_key(|i| (equipped.get(&i.serial).copied().unwrap_or(99), i.container == Container::Unknown, i.slot));
    if let Some(n) = app.items.select_index.take() {
        if let Some(it) = list.get(n) {
            app.items.sel = Some(it.path.clone());
        }
    }

    // capacity line
    let prof_doc = app.sess.as_ref().and_then(|s| s.doc(Which::Profile));
    let sdu: Vec<(String, i64)> = prof_doc
        .map(|p| {
            let mut n = p.clone();
            save::Profile(&mut n).sdu_nodes()
        })
        .unwrap_or_default();
    let bonus = |prefix: &str, adds: &[u32]| -> u32 {
        (1..=adds.len()).filter(|i| sdu.iter().any(|(n, _)| n == &format!("{prefix}_{i:02}"))).map(|i| adds[i - 1]).sum()
    };
    ui.horizontal(|ui| {
        match view {
            View::Backpack => {
                let cap = 16 + bonus("Backpack", &[4, 4, 6, 6, 6, 8, 8, 12]);
                let used = all.iter().filter(|i| i.container == Container::Backpack && i.flags != Some(1)).count() as u32;
                let col = if used > cap { Color32::from_rgb(255, 80, 80) } else { Color32::from_gray(200) };
                ui.label(RichText::new(format!("Backpack {used}/{cap} (equipped items are free)")).color(col));
                if used > cap {
                    ui.label(RichText::new("Over capacity: the game deletes the overflow on load!").color(col));
                }
            }
            View::Bank => {
                let cap = 25 + bonus("Bank", &[25, 50, 50, 50, 50, 50, 100, 100]);
                let used = list.len() as u32;
                ui.label(format!("Bank {used}/{cap}"));
            }
            View::LostLoot => {
                ui.label(format!("Lost Loot: {} items", list.len()));
            }
        }
        ui.separator();
        ui.label("Filter:");
        ui.add(egui::TextEdit::singleline(&mut app.items.filter).desired_width(160.0));
    });
    ui.separator();

    let avail = ui.available_size();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(avail.x * 0.38, avail.y), egui::Layout::top_down(egui::Align::Min), |ui| {
            item_list(app, ui, &list, &equipped, view);
        });
        ui.separator();
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), avail.y), egui::Layout::top_down(egui::Align::Min), |ui| {
            egui::ScrollArea::vertical().id_salt("item_editor").show(ui, |ui| {
                let sel = app.items.sel.clone().and_then(|p| list.iter().find(|i| i.path == p).cloned());
                match sel {
                    Some(it) => item_editor(app, ui, &it, which, &equipped),
                    None => {
                        ui.label("Select an item to see and edit it.");
                    }
                }
                ui.add_space(12.0);
                ui.separator();
                add_item_panel(app, ui, view);
            });
        });
    });
}

fn item_list(app: &mut App, ui: &mut egui::Ui, list: &[InvItem], equipped: &HashMap<String, usize>, view: View) {
    let filter = app.items.filter.to_lowercase();
    egui::ScrollArea::vertical().id_salt("item_list").show(ui, |ui| {
        let mut last_header = None;
        for it in list {
            let a = app.items.analyze(&app.db, &it.serial);
            let info = a.info.as_ref();
            let name = info.map(|i| i.name.clone()).unwrap_or_else(|| "Undecodable item".into());
            let tname = info.map(|i| i.type_name.clone()).unwrap_or_default();
            if !filter.is_empty() && !format!("{name} {tname}").to_lowercase().contains(&filter) {
                continue;
            }
            let header = match (view, equipped.get(&it.serial), &it.container) {
                (View::Backpack, Some(_), _) => "Equipped",
                (View::Backpack, None, Container::Unknown) => "Unknown items (parked by the game)",
                (View::Backpack, None, _) => "Backpack",
                (View::Bank, ..) => "Bank",
                (View::LostLoot, ..) => "Lost Loot",
            };
            if last_header != Some(header) {
                ui.add_space(4.0);
                ui.label(RichText::new(header).strong().color(Color32::from_rgb(255, 170, 40)));
                last_header = Some(header);
            }
            let selected = app.items.sel.as_deref() == Some(&it.path);
            let (mk, mc, tip) = marker(info);
            let rc = info.map(|i| rarity_color(i.rarity)).unwrap_or(Color32::GRAY);
            let resp = ui
                .horizontal(|ui| {
                    if let Some(i) = info {
                        let mfr = app.db.category(i.category).and_then(|c| c.mfr.clone());
                        let key = app.db.category(i.category).map(|c| c.key.clone()).unwrap_or_default();
                        if let Some((bytes, uri)) = app.icons.type_icon(&i.kind, &key, mfr.as_deref()) {
                            ui.add(egui::Image::from_bytes(uri, bytes).fit_to_exact_size(egui::vec2(34.0, 20.0)).tint(rc));
                        }
                    } else {
                        ui.add_space(38.0);
                    }
                    ui.label(RichText::new(mk).color(mc)).on_hover_text(tip);
                    let lvl = info.and_then(|i| i.level).map(|l| format!("L{l}")).unwrap_or_default();
                    let slot = equipped.get(&it.serial).map(|s| format!("[{}] ", equip_slot_label(*s))).unwrap_or_default();
                    let fav = if it.favorite() { "★ " } else { "" };
                    let r = ui.selectable_label(selected, RichText::new(format!("{slot}{fav}{name}")).color(rc).strong());
                    ui.label(RichText::new(format!("{lvl} {tname}")).weak().small());
                    r
                })
                .inner;
            if resp.clicked() {
                app.items.sel = Some(it.path.clone());
            }
        }
        if list.is_empty() {
            ui.label("Empty.");
        }
    });
}

fn image(ui: &mut egui::Ui, icon: Option<(&'static [u8], String)>, size: egui::Vec2, tint: Color32) {
    if let Some((bytes, uri)) = icon {
        ui.add(egui::Image::from_bytes(uri, bytes).fit_to_exact_size(size).tint(tint));
    }
}

fn markup(ui: &mut egui::Ui, text: &str) {
    // [secondary]..[/secondary], [rarity_legendary].. -> colored segments
    let mut job = egui::text::LayoutJob::default();
    let mut color = Color32::from_gray(210);
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(start) = rest.find('[') {
            if start > 0 {
                job.append(&rest[..start], 0.0, egui::TextFormat { color, ..Default::default() });
            }
            if let Some(end) = rest[start..].find(']') {
                let tag = &rest[start + 1..start + end];
                color = if tag.starts_with('/') {
                    Color32::from_gray(210)
                } else if tag.contains("legendary") {
                    Color32::from_rgb(255, 120, 60)
                } else if tag.contains("secondary") || tag.contains("primary") {
                    Color32::from_rgb(120, 200, 255)
                } else {
                    color
                };
                rest = &rest[start + end + 1..];
                continue;
            }
            job.append(&rest[start..], 0.0, egui::TextFormat { color, ..Default::default() });
            break;
        } else {
            job.append(rest, 0.0, egui::TextFormat { color, ..Default::default() });
            break;
        }
    }
    let first = text.split(", ").count();
    let _ = first;
    ui.label(job);
}

fn item_editor(app: &mut App, ui: &mut egui::Ui, it: &InvItem, which: Which, equipped: &HashMap<String, usize>) {
    let a = app.items.analyze(&app.db, &it.serial);
    if app.items.serial_for != it.serial {
        app.items.serial_for = it.serial.clone();
        app.items.serial_text = it.serial.clone();
        app.items.add_part = None;
    }
    let Some(info) = a.info.clone() else {
        ui.colored_label(Color32::from_rgb(255, 80, 80), format!("Undecodable serial: {}", a.error.clone().unwrap_or_default()));
        serial_box(app, ui, it, which);
        delete_buttons(app, ui, it, which);
        return;
    };
    let serial = a.serial.clone().unwrap();
    let rc = rarity_color(info.rarity);
    let cat = app.db.category(info.category).cloned();
    let mfr = cat.as_ref().and_then(|c| c.mfr.clone());

    // ---- card
    egui::Frame::group(ui.style()).fill(Color32::from_rgb(28, 26, 34)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let key = cat.as_ref().map(|c| c.key.clone()).unwrap_or_default();
            image(ui, app.icons.type_icon(&info.kind, &key, mfr.as_deref()), egui::vec2(150.0, 90.0), rc);
            ui.vertical(|ui| {
                ui.label(RichText::new(&info.name).size(22.0).strong().color(rc));
                ui.label(RichText::new(format!("{} · {} · {}", info.rarity.label(), info.type_name, kind_label(&info.kind))).color(rc));
                ui.horizontal(|ui| {
                    if let Some(l) = info.level {
                        ui.label(RichText::new(format!("Level {l}")).strong());
                    }
                    for e in &info.element {
                        image(ui, app.icons.element(e), egui::vec2(18.0, 18.0), Color32::WHITE);
                        ui.label(e);
                    }
                    if let Some(s) = equipped.get(&it.serial) {
                        ui.label(RichText::new(format!("Equipped: {}", equip_slot_label(*s))).color(Color32::from_rgb(120, 200, 255)));
                    }
                });
                for p in &info.parts {
                    if let Some(fw) = app.db.part(p.r).and_then(|x| x.fw.clone()) {
                        ui.horizontal(|ui| {
                            image(ui, app.icons.firmware(&fw), egui::vec2(22.0, 22.0), Color32::WHITE);
                            ui.label(&p.label);
                        });
                    }
                }
            });
            if let Some(m) = &mfr {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    image(ui, app.icons.manufacturer(m), egui::vec2(90.0, 58.0), Color32::from_gray(230));
                });
            }
        });
        if !info.text.is_empty() {
            ui.separator();
            for t in &info.text {
                markup(ui, &bl4core::stats::render_text(&app.db, &serial, &info, t));
            }
        }
        let stats = crate::pages::item_stats(&app.db, &serial, &info);
        if !stats.is_empty() {
            ui.separator();
            ui.label(RichText::new("Stats (computed from game data; before skills and buffs)").weak().small());
            egui::Grid::new("stats").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
                for (k, v) in stats {
                    ui.label(RichText::new(k).weak());
                    ui.label(RichText::new(v).strong());
                    ui.end_row();
                }
            });
        }
    });

    // ---- validity
    ui.add_space(4.0);
    if info.issues.iter().all(|i| i.sev == Severity::Info) {
        ui.label(RichText::new("✔ Valid: every part exists and the item follows the game's rules for its rarity.").color(Color32::from_rgb(90, 200, 90)));
    }
    for is in &info.issues {
        let (c, p) = match is.sev {
            Severity::Error => (Color32::from_rgb(255, 90, 90), "✖ Invalid:"),
            Severity::Warning => (Color32::from_rgb(255, 190, 60), "⚠ Unusual:"),
            Severity::Info => (Color32::from_gray(150), "ℹ"),
        };
        ui.label(RichText::new(format!("{p} {}", is.msg)).color(c));
    }

    // ---- basics
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if let Some(l) = info.level {
            let mut lv = l;
            ui.label("Level");
            if ui.add(egui::DragValue::new(&mut lv).range(1..=item::MAX_LEVEL)).changed() && lv != l {
                let mut s = serial.clone();
                if s.set_level(lv) {
                    apply_serial(app, it, which, &s.encode());
                }
            }
            if ui.button(format!("Max ({})", item::MAX_LEVEL)).clicked() {
                let mut s = serial.clone();
                if s.set_level(item::MAX_LEVEL) {
                    apply_serial(app, it, which, &s.encode());
                }
            }
        }
        let sf = it.state_flags.unwrap_or(1);
        let mut fav = sf & 2 != 0;
        let mut junk = sf & 4 != 0;
        if ui.checkbox(&mut fav, "Favorite").changed() {
            let v = if fav { sf | 2 } else { sf & !2 };
            let it2 = it.clone();
            app.edit(which, |d| save::set_state_flags(d, &it2, v));
        }
        if ui.checkbox(&mut junk, "Junk").changed() {
            let v = if junk { sf | 4 } else { sf & !4 };
            let it2 = it.clone();
            app.edit(which, |d| save::set_state_flags(d, &it2, v));
        }
    });

    // ---- parts
    ui.add_space(6.0);
    ui.label(RichText::new("Parts").strong().size(16.0));
    let refs = serial.part_refs();
    let effects = bl4core::stats::part_effects(&app.db, &serial, &info);
    let mut action: Option<Box<dyn FnOnce(&mut Serial)>> = None;
    egui::Grid::new("parts").num_columns(4).striped(true).spacing([10.0, 3.0]).show(ui, |ui| {
        for (n, p) in info.parts.iter().enumerate() {
            let _ = refs.get(n);
            ui.label(RichText::new(&p.slot).weak());
            let cur_label = format!("{}  ({})", p.label, p.key);
            let id = ui.id().with(("part", n, &it.path));
            let mut chosen: Option<PartRef> = None;
            egui::ComboBox::from_id_salt(id).width(360.0).selected_text(cur_label).show_ui(ui, |ui| {
                for cand in candidates(&app.db, p.r.cat, &p.slot) {
                    let cp = app.db.part(cand).unwrap();
                    let lbl = format!("{}  ({}){}", item::part_label(&app.db, cp), cp.k, if cp.r#mod { " [mod]" } else { "" });
                    if ui.selectable_label(cand == p.r, lbl).clicked() {
                        chosen = Some(cand);
                    }
                }
            });
            if let Some(c) = chosen {
                if c != p.r {
                    action = Some(Box::new(move |s: &mut Serial| {
                        s.replace_part(n, c);
                    }));
                }
            }
            let fx = effects.get(&p.key).cloned().unwrap_or_default();
            let id_lbl = ui.label(RichText::new(format!("{}:{}", p.r.cat, p.r.idx)).weak().small());
            if !fx.is_empty() {
                id_lbl.on_hover_text(fx.join("
"));
            }
            if ui.small_button("✖").on_hover_text("Remove this part").clicked() {
                action = Some(Box::new(move |s: &mut Serial| {
                    s.remove_part(n);
                }));
            }
            ui.end_row();
            if !p.text.is_empty() {
                ui.label("");
                ui.vertical(|ui| {
                    for t in &p.text {
                        markup(ui, &bl4core::stats::render_text(&app.db, &serial, &info, t));
                    }
                });
                ui.end_row();
            }
        }
    });
    if let Some(f) = action {
        let mut s = serial.clone();
        f(&mut s);
        apply_serial(app, it, which, &s.encode());
        return;
    }

    // ---- add part
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Add part:");
        let slots = slot_options(&app.db, info.category, &info.kind);
        egui::ComboBox::from_id_salt("add_slot")
            .selected_text(if app.items.add_slot.is_empty() { "slot…".to_string() } else { app.items.add_slot.clone() })
            .show_ui(ui, |ui| {
                for (c, s) in &slots {
                    if ui.selectable_label(app.items.add_slot == *s, format!("{s} ({c})")).clicked() {
                        app.items.add_slot = s.clone();
                        app.items.add_part = None;
                    }
                }
            });
        let cands: Vec<PartRef> = slots
            .iter()
            .filter(|(_, s)| *s == app.items.add_slot)
            .flat_map(|(c, s)| candidates(&app.db, *c, s))
            .collect();
        let sel_lbl = app
            .items
            .add_part
            .and_then(|r| app.db.part(r))
            .map(|p| format!("{} ({})", item::part_label(&app.db, p), p.k))
            .unwrap_or_else(|| "part…".into());
        egui::ComboBox::from_id_salt("add_part").width(320.0).selected_text(sel_lbl).show_ui(ui, |ui| {
            ui.add(egui::TextEdit::singleline(&mut app.items.part_filter).hint_text("search"));
            let f = app.items.part_filter.to_lowercase();
            for c in cands {
                let p = app.db.part(c).unwrap();
                let lbl = format!("{} ({})", item::part_label(&app.db, p), p.k);
                if !f.is_empty() && !lbl.to_lowercase().contains(&f) {
                    continue;
                }
                if ui.selectable_label(app.items.add_part == Some(c), lbl).clicked() {
                    app.items.add_part = Some(c);
                }
            }
        });
        if ui.add_enabled(app.items.add_part.is_some(), egui::Button::new("Add")).clicked() {
            if let Some(r) = app.items.add_part {
                let mut s = serial.clone();
                s.add_part(r);
                apply_serial(app, it, which, &s.encode());
            }
        }
    });

    ui.add_space(6.0);
    serial_box(app, ui, it, which);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui.button("Duplicate").on_hover_text("Copy this item into the backpack (or bank)").clicked() {
            let s = it.serial.clone();
            if which == Which::Profile {
                app.edit(Which::Profile, |d| {
                    save::add_bank_item(d, &s);
                });
            } else {
                app.edit(Which::Character, |d| {
                    save::add_backpack_item(d, &s);
                });
            }
            app.info("Item duplicated");
        }
        if it.container == Container::LostLoot && ui.button("Move to backpack").clicked() {
            let it2 = it.clone();
            app.edit(Which::Character, |d| {
                save::remove_item(d, &it2);
                save::add_backpack_item(d, &it2.serial);
            });
            app.items.sel = None;
        }
        let has_both = app.sess.as_ref().map(|s| s.character.is_some() && s.profile.is_some()).unwrap_or(false);
        if has_both && matches!(it.container, Container::Backpack | Container::Bank) {
            let to_bank = which == Which::Character;
            if ui.button(if to_bank { "Move to bank" } else { "Move to backpack" }).clicked() {
                let it2 = it.clone();
                app.edit_both(|c, p| {
                    if to_bank {
                        save::remove_item(c, &it2);
                        save::add_bank_item(p, &it2.serial);
                    } else {
                        save::remove_item(p, &it2);
                        save::add_backpack_item(c, &it2.serial);
                    }
                });
                app.items.sel = None;
            }
        }
        delete_buttons(app, ui, it, which);
    });
}

fn delete_buttons(app: &mut App, ui: &mut egui::Ui, it: &InvItem, which: Which) {
    if ui.button(RichText::new("Delete").color(Color32::from_rgb(255, 110, 110))).clicked() {
        let it2 = it.clone();
        app.edit(which, |d| save::remove_item(d, &it2));
        app.items.sel = None;
        app.info("Item deleted");
    }
}

fn serial_box(app: &mut App, ui: &mut egui::Ui, it: &InvItem, which: Which) {
    ui.horizontal(|ui| {
        ui.label("Serial:");
        ui.add(egui::TextEdit::singleline(&mut app.items.serial_text).desired_width(ui.available_width() - 150.0).font(egui::TextStyle::Monospace));
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(it.serial.clone());
        }
        let changed = app.items.serial_text.trim() != it.serial;
        if ui.add_enabled(changed, egui::Button::new("Apply")).clicked() {
            let t = app.items.serial_text.trim().to_string();
            match Serial::decode(&t) {
                Ok(_) => apply_serial(app, it, which, &t),
                Err(e) => app.error(format!("Not a valid serial: {e}")),
            }
        }
    });
}

fn apply_serial(app: &mut App, it: &InvItem, which: Which, new_serial: &str) {
    let it2 = it.clone();
    let ns = new_serial.to_string();
    app.edit(which, |d| save::set_item_serial(d, &it2, &ns));
    app.items.serial_for.clear();
}

/// Parts that can go into `slot` of category `cat`.
pub fn candidates(db: &Db, cat: u32, slot: &str) -> Vec<PartRef> {
    let mut v: Vec<PartRef> = db
        .cat_parts
        .get(&cat)
        .map(|l| l.iter().copied().filter(|r| db.part(*r).map(|p| p.s == slot).unwrap_or(false)).collect())
        .unwrap_or_default();
    v.sort_by_key(|r| db.part(*r).map(|p| p.k.clone()).unwrap_or_default());
    v
}

/// (category, slot) pairs an item of this category can take parts from.
fn slot_options(db: &Db, cat: u32, kind: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = vec![];
    let mut cats = vec![cat];
    cats.extend_from_slice(bl4core::db::allowed_pools(kind));
    for c in cats {
        let mut slots: Vec<String> = db
            .cat_parts
            .get(&c)
            .map(|l| l.iter().filter_map(|r| db.part(*r).map(|p| p.s.clone())).collect())
            .unwrap_or_default();
        slots.sort();
        slots.dedup();
        for s in slots {
            if !out.iter().any(|(_, x)| *x == s) {
                out.push((c, s));
            }
        }
    }
    out
}

fn add_item_panel(app: &mut App, ui: &mut egui::Ui, view: View) {
    let which = if view == View::Bank { Which::Profile } else { Which::Character };
    if view == View::LostLoot {
        return;
    }
    ui.label(RichText::new(if which == Which::Profile { "Add to bank" } else { "Add to backpack" }).strong().size(16.0));
    ui.horizontal(|ui| {
        ui.label("Serial code:");
        ui.add(egui::TextEdit::singleline(&mut app.items.paste).desired_width(420.0).hint_text("@Ug...").font(egui::TextStyle::Monospace));
    });
    let paste = app.items.paste.trim().to_string();
    if !paste.is_empty() {
        let a = app.items.analyze(&app.db, &paste);
        match &a.info {
            Some(i) => {
                let (mk, mc, tip) = marker(Some(i));
                ui.horizontal(|ui| {
                    ui.label(RichText::new(mk).color(mc));
                    ui.label(RichText::new(format!("{} - {} L{}", i.name, i.type_name, i.level.unwrap_or(0))).color(rarity_color(i.rarity)));
                    ui.label(RichText::new(tip).weak());
                });
                for is in i.issues.iter().filter(|x| x.sev >= Severity::Warning) {
                    ui.label(RichText::new(&is.msg).small().color(Color32::from_rgb(255, 190, 60)));
                }
                let blocked = i.issues.iter().any(|x| x.sev == Severity::Error);
                if ui.add_enabled(!blocked, egui::Button::new("Add item")).on_disabled_hover_text("The game would reject this item").clicked() {
                    add_serial(app, which, &paste);
                    app.items.paste.clear();
                }
            }
            None => {
                ui.colored_label(Color32::from_rgb(255, 90, 90), format!("Not a valid serial: {}", a.error.clone().unwrap_or_default()));
            }
        }
    }
    ui.add_space(6.0);
    ui.label(RichText::new("New item").strong());
    ui.horizontal(|ui| {
        let cats = app.db.item_categories();
        let lbl = app
            .items
            .new_cat
            .and_then(|c| app.db.category(c))
            .map(|c| format!("{} ({})", c.name.clone().unwrap_or_default(), kind_label(&c.kind)))
            .unwrap_or_else(|| "item type…".into());
        let mut new_cat = app.items.new_cat;
        egui::ComboBox::from_id_salt("new_cat").width(260.0).selected_text(lbl).show_ui(ui, |ui| {
            for c in cats {
                let l = format!("{} ({})", c.name.clone().unwrap_or_else(|| c.key.clone()), kind_label(&c.kind));
                if ui.selectable_label(new_cat == Some(c.id), l).clicked() {
                    new_cat = Some(c.id);
                }
            }
        });
        if new_cat != app.items.new_cat {
            app.items.new_cat = new_cat;
            app.items.new_comp.clear();
        }
        if let Some(cid) = app.items.new_cat {
            let mut comps: Vec<(String, String)> = app
                .db
                .cat_parts
                .get(&cid)
                .map(|l| {
                    l.iter()
                        .filter_map(|r| app.db.part(*r))
                        .filter(|p| p.s == "inv_comp")
                        .map(|p| (p.k.clone(), item::part_label(&app.db, p)))
                        .collect()
                })
                .unwrap_or_default();
            comps.sort();
            let cl = if app.items.new_comp.is_empty() { "rarity…".to_string() } else { app.items.new_comp.clone() };
            egui::ComboBox::from_id_salt("new_comp").width(280.0).selected_text(cl).show_ui(ui, |ui| {
                for (k, l) in &comps {
                    let txt = if l != k { format!("{k} - {l}") } else { k.clone() };
                    if ui.selectable_label(app.items.new_comp == *k, txt).clicked() {
                        app.items.new_comp = k.clone();
                    }
                }
            });
        }
        if app.items.new_level == 0 {
            app.items.new_level = item::MAX_LEVEL;
        }
        ui.label("Level");
        ui.add(egui::DragValue::new(&mut app.items.new_level).range(1..=item::MAX_LEVEL));
        let ready = app.items.new_cat.is_some() && !app.items.new_comp.is_empty();
        if ui.add_enabled(ready, egui::Button::new("Create")).clicked() {
            let seed = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(1) % 4000) as u64 + 1;
            match item::build_item(&app.db, app.items.new_cat.unwrap(), &app.items.new_comp, app.items.new_level, seed) {
                Some(s) => {
                    let enc = s.encode();
                    app.items.paste = enc;
                    app.info("New item built: review it above, then press Add item");
                }
                None => app.error("Could not build that item"),
            }
        }
    });
}

fn add_serial(app: &mut App, which: Which, serial: &str) {
    let s = serial.to_string();
    if which == Which::Profile {
        app.edit(Which::Profile, |d| {
            save::add_bank_item(d, &s);
        });
    } else {
        app.edit(Which::Character, |d| {
            save::add_backpack_item(d, &s);
        });
    }
    app.info("Item added");
}
