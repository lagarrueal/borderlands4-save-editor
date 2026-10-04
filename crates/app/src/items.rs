//! Backpack / bank / lost-loot views and the item editor.

use crate::app::{App, Which};
use bl4core::db::{kind_label, Db};
use bl4core::item::{self, ItemInfo, Rarity, Severity};
use bl4core::save::{self, Container, InvItem};
use bl4core::serial::{PartRef, Serial};
use crate::widgets::{search_combo, Opt};
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
    let (bp_cap, bank_cap) = app.capacities();
    ui.horizontal(|ui| {
        match view {
            View::Backpack => {
                let cap = bp_cap;
                let used = all.iter().filter(|i| i.container == Container::Backpack && i.flags != Some(1)).count() as u32;
                let col = if used > cap { Color32::from_rgb(255, 80, 80) } else { Color32::from_gray(200) };
                ui.label(RichText::new(format!("Backpack {used}/{cap} (equipped items are free)")).color(col));
                if used > cap {
                    ui.label(RichText::new("Over capacity: the game deletes the overflow on load!").color(col));
                }
            }
            View::Bank => {
                let cap = bank_cap;
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
        if let Some(lvl) = app.character_level() {
            if view != View::LostLoot {
                ui.separator();
                let what = if view == View::Bank { "bank" } else { "backpack" };
                if ui.button(format!("Scale all to level {lvl}")).on_hover_text(format!("Set every {what} item (equipped ones too) to your character level")).clicked() {
                    let (w, c) = if view == View::Bank { (Which::Profile, Container::Bank) } else { (Which::Character, Container::Backpack) };
                    let mut res = (0, 0);
                    app.edit(w, |d| res = save::scale_items(d, c, lvl));
                    app.items.invalidate();
                    app.info(format!("{} items set to level {lvl}{}", res.0, if res.1 > 0 { format!(" ({} without a level field skipped)", res.1) } else { String::new() }));
                }
            }
        }
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

/// Render the game's rich text: [secondary]..[/secondary] blue,
/// [rarity_legendary].. gold, [redtext].. red italic (flavour text).
pub fn markup(ui: &mut egui::Ui, text: &str) {
    let base = Color32::from_gray(210);
    let mut job = egui::text::LayoutJob::default();
    let mut stack: Vec<String> = vec![];
    let fmt = |stack: &Vec<String>| {
        let mut f = egui::TextFormat { color: base, ..Default::default() };
        for t in stack {
            if t == "redtext" {
                f.color = Color32::from_rgb(235, 60, 60);
                f.italics = true;
            } else if t.contains("legendary") {
                f.color = Color32::from_rgb(255, 200, 40);
            } else if t.contains("secondary") || t.contains("primary") {
                f.color = Color32::from_rgb(120, 200, 255);
            }
        }
        f
    };
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find('[') {
            Some(start) => {
                if start > 0 {
                    job.append(&rest[..start], 0.0, fmt(&stack));
                }
                match rest[start..].find(']') {
                    Some(end) => {
                        let tag = &rest[start + 1..start + end];
                        if let Some(closing) = tag.strip_prefix('/') {
                            if let Some(i) = stack.iter().rposition(|t| t == closing) {
                                stack.remove(i);
                            }
                        } else if !tag.contains(' ') && tag.len() < 40 {
                            stack.push(tag.to_string());
                        } else {
                            job.append(&rest[start..start + end + 1], 0.0, fmt(&stack));
                        }
                        rest = &rest[start + end + 1..];
                    }
                    None => {
                        job.append(&rest[start..], 0.0, fmt(&stack));
                        break;
                    }
                }
            }
            None => {
                job.append(rest, 0.0, fmt(&stack));
                break;
            }
        }
    }
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
            if let Some(cl) = app.character_level() {
                if ui.add_enabled(cl != l, egui::Button::new(format!("Scale to character level ({cl})"))).clicked() {
                    let mut s = serial.clone();
                    if s.set_level(cl) {
                        apply_serial(app, it, which, &s.encode());
                    }
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
            let opts: Vec<Opt<PartRef>> = candidates(&app.db, p.r.cat, &p.slot)
                .into_iter()
                .map(|cand| part_opt(&app.db, cand).selected(cand == p.r))
                .collect();
            let chosen = search_combo(ui, ("part", n, &it.path), 360.0, cur_label, opts);
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
        let slot_opts: Vec<Opt<String>> = slots.iter().map(|(c, s)| Opt::new(s.clone(), format!("{s} ({c})")).selected(app.items.add_slot == *s)).collect();
        let slot_lbl = if app.items.add_slot.is_empty() { "slot…".to_string() } else { app.items.add_slot.clone() };
        if let Some(sl) = search_combo(ui, "add_slot", 170.0, slot_lbl, slot_opts) {
            app.items.add_slot = sl;
            app.items.add_part = None;
        }
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
        let part_opts: Vec<Opt<PartRef>> = cands.into_iter().map(|c| part_opt(&app.db, c).selected(app.items.add_part == Some(c))).collect();
        if let Some(c) = search_combo(ui, "add_part", 340.0, sel_lbl, part_opts) {
            app.items.add_part = Some(c);
        }
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
        if ui.button("Duplicate").on_hover_text("Copy this item into the backpack (or bank)").clicked() && app.has_room(which) {
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
        if it.container == Container::LostLoot && ui.button("Move to backpack").clicked() && app.has_room(Which::Character) {
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
            let target = if to_bank { Which::Profile } else { Which::Character };
            if ui.button(if to_bank { "Move to bank" } else { "Move to backpack" }).clicked() && app.has_room(target) {
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

/// A dropdown entry for a part: label, key, and its effect text for searching.
fn part_opt(db: &Db, r: PartRef) -> Opt<PartRef> {
    let p = db.part(r).unwrap();
    let lbl = format!("{}  ({}){}", item::part_label(db, p), p.k, if p.r#mod { " [mod]" } else { "" });
    let extra: Vec<String> = p.text.iter().map(|t| item::plain(t)).chain(p.desc.clone()).collect();
    Opt::new(r, lbl).extra(extra.join(" "))
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
        let cat_opts: Vec<Opt<u32>> = cats
            .iter()
            .map(|c| {
                Opt::new(c.id, format!("{} ({})", c.name.clone().unwrap_or_else(|| c.key.clone()), kind_label(&c.kind)))
                    .extra(c.key.clone())
                    .selected(new_cat == Some(c.id))
            })
            .collect();
        if let Some(c) = search_combo(ui, "new_cat", 260.0, lbl, cat_opts) {
            new_cat = Some(c);
        }
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
            let comp_opts: Vec<Opt<String>> = comps
                .iter()
                .map(|(k, l)| Opt::new(k.clone(), if l != k { format!("{l} ({k})") } else { k.clone() }).selected(app.items.new_comp == *k))
                .collect();
            if let Some(k) = search_combo(ui, "new_comp", 300.0, cl, comp_opts) {
                app.items.new_comp = k;
            }
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
    if !app.has_room(which) {
        return;
    }
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
