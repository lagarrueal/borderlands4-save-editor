//! Application state, window layout and the open/save workflow.

use crate::icons::Icons;
use crate::items::ItemsState;
use crate::pages::PagesState;
use bl4core::db::Db;
use bl4core::save::{self, SaveFile, SaveKind};
use bl4core::yaml::Node;
use eframe::egui::{self, Color32, RichText};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Default)]
pub struct StartOptions {
    pub open: Option<PathBuf>,
    pub screenshot: Option<PathBuf>,
    pub tab: Option<String>,
    pub select: Option<usize>,
    pub steam_id: Option<String>,
    pub no_backup: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Character,
    Currency,
    Progression,
    Backpack,
    LostLoot,
    Bank,
    Missions,
    Appearance,
    Raw,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Character => "Character",
            Tab::Currency => "Currency & Ammo",
            Tab::Progression => "UVH & SDU",
            Tab::Backpack => "Backpack",
            Tab::LostLoot => "Lost Loot",
            Tab::Bank => "Bank",
            Tab::Missions => "Missions",
            Tab::Appearance => "Appearance",
            Tab::Raw => "Raw",
        }
    }
    fn parse(s: &str) -> Option<Tab> {
        [
            Tab::Character,
            Tab::Currency,
            Tab::Progression,
            Tab::Backpack,
            Tab::LostLoot,
            Tab::Bank,
            Tab::Missions,
            Tab::Appearance,
            Tab::Raw,
        ]
        .into_iter()
        .find(|t| format!("{t:?}").eq_ignore_ascii_case(s))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Character,
    Profile,
}

pub struct SaveEntry {
    pub path: PathBuf,
    pub label: String,
    pub detail: String,
    pub kind: SaveKind,
}

pub struct Session {
    pub dir: PathBuf,
    pub character: Option<SaveFile>,
    pub profile: Option<SaveFile>,
    pub char_dirty: bool,
    pub prof_dirty: bool,
    undo: Vec<(Option<Node>, Option<Node>)>,
    redo: Vec<(Option<Node>, Option<Node>)>,
}

impl Session {
    pub fn doc(&self, w: Which) -> Option<&Node> {
        match w {
            Which::Character => self.character.as_ref().map(|s| &s.doc),
            Which::Profile => self.profile.as_ref().map(|s| &s.doc),
        }
    }
    fn snapshot(&self) -> (Option<Node>, Option<Node>) {
        (self.character.as_ref().map(|s| s.doc.clone()), self.profile.as_ref().map(|s| s.doc.clone()))
    }
    fn restore(&mut self, snap: (Option<Node>, Option<Node>)) {
        if let (Some(c), Some(d)) = (self.character.as_mut(), snap.0) {
            c.doc = d;
            self.char_dirty = true;
        }
        if let (Some(p), Some(d)) = (self.profile.as_mut(), snap.1) {
            p.doc = d;
            self.prof_dirty = true;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

pub struct App {
    pub db: Db,
    pub icons: Icons,
    pub folder: Option<PathBuf>,
    pub saves: Vec<SaveEntry>,
    pub sess: Option<Session>,
    pub tab: Tab,
    pub steam_id: String,
    pub game_running: bool,
    last_check: Instant,
    pub log: Vec<(Level, String)>,
    pub items: ItemsState,
    pub pages: PagesState,
    /// mission set -> missions (from the game data)
    pub set_missions: HashMap<String, Vec<String>>,
    screenshot: Option<PathBuf>,
    frame_no: u32,
    no_backup: bool,
    confirm_discard: Option<PathBuf>,
}

impl App {
    pub fn new(ctx: &egui::Context, opts: StartOptions) -> App {
        setup_style(ctx);
        let db = Db::load();
        let mut set_missions: HashMap<String, Vec<String>> = HashMap::new();
        for (m, info) in &db.missions {
            if !info.set.is_empty() {
                set_missions.entry(info.set.to_lowercase()).or_default().push(m.clone());
            }
        }
        for v in set_missions.values_mut() {
            v.sort();
        }
        let mut app = App {
            db,
            icons: Icons::load(),
            folder: None,
            saves: vec![],
            sess: None,
            tab: Tab::Character,
            steam_id: opts.steam_id.clone().unwrap_or_default(),
            game_running: save::game_running(),
            last_check: Instant::now(),
            log: vec![],
            items: ItemsState::default(),
            pages: PagesState::default(),
            set_missions,
            screenshot: opts.screenshot.clone(),
            frame_no: 0,
            no_backup: opts.no_backup,
            confirm_discard: None,
        };
        let db_note = format!(
            "Game data: {} item types, {} parts ({}){}",
            app.db.categories.len(),
            app.db.parts.len(),
            app.db.origin,
            if app.db.meta.mod_paks.is_empty() { String::new() } else { format!("; includes mods: {}", app.db.meta.mod_paks.join(", ")) }
        );
        app.info(db_note);
        if let Some(p) = opts.open {
            if p.is_dir() {
                app.set_folder(&p);
            } else {
                if let Some(d) = p.parent() {
                    app.set_folder(d);
                }
                app.open(&p);
            }
        } else if let Some(d) = save::default_save_dirs().into_iter().next() {
            app.set_folder(&d);
        }
        if let Some(t) = opts.tab.as_deref().and_then(Tab::parse) {
            app.tab = t;
        }
        if let Some(n) = opts.select {
            app.items.select_index = Some(n);
        }
        app
    }

    pub fn info(&mut self, s: impl Into<String>) {
        self.log.push((Level::Info, s.into()));
    }
    pub fn warn(&mut self, s: impl Into<String>) {
        self.log.push((Level::Warn, s.into()));
    }
    pub fn error(&mut self, s: impl Into<String>) {
        self.log.push((Level::Error, s.into()));
    }

    fn steam_id(&self) -> Option<u64> {
        self.steam_id.trim().parse().ok()
    }

    pub fn set_folder(&mut self, dir: &Path) {
        self.folder = Some(dir.to_path_buf());
        self.saves.clear();
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|x| x.eq_ignore_ascii_case("sav")).unwrap_or(false))
            .collect();
        files.sort_by_key(|p| {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            (stem != "profile", stem.parse::<u32>().unwrap_or(u32::MAX), stem)
        });
        for p in files {
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let (label, detail, kind) = match SaveFile::open(&p, self.steam_id()) {
                Ok(mut sf) => {
                    if sf.kind == SaveKind::Profile {
                        ("Profile".to_string(), "bank, SDUs, cosmetics".to_string(), SaveKind::Profile)
                    } else {
                        let c = save::Character(&mut sf.doc);
                        let lvl = c.xp("Character").map(|x| x.0).unwrap_or(0);
                        let (uvh_max, _) = c.uvh();
                        let class = class_label(&c.class());
                        (
                            format!("{} - {}", stem, c.name()),
                            format!("{class} L{lvl}{}", if uvh_max > 0 { format!(" UVH{uvh_max}") } else { String::new() }),
                            SaveKind::Character,
                        )
                    }
                }
                Err(e) => (stem.clone(), format!("unreadable: {e}"), SaveKind::Character),
            };
            self.saves.push(SaveEntry { path: p, label, detail, kind });
        }
    }

    pub fn any_dirty(&self) -> bool {
        self.sess.as_ref().map(|s| s.char_dirty || s.prof_dirty).unwrap_or(false)
    }

    /// Open a character (with its profile) or the profile alone. Both files
    /// are backed up as they are on disk before anything else happens.
    pub fn open(&mut self, path: &Path) {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let sid = self.steam_id();
        let mut main = match SaveFile::open(path, sid) {
            Ok(s) => s,
            Err(e) => {
                self.error(format!("Cannot open {}: {e}", path.display()));
                return;
            }
        };
        if self.steam_id.is_empty() {
            self.steam_id = main.steam_id.to_string();
        }
        let mut profile = None;
        let mut character = None;
        if main.kind == SaveKind::Profile {
            profile = Some(main);
        } else {
            let pp = dir.join("profile.sav");
            if pp.exists() {
                match SaveFile::open(&pp, Some(main.steam_id)) {
                    Ok(p) => profile = Some(p),
                    Err(e) => self.warn(format!("profile.sav not loaded ({e}); bank, SDUs and cosmetics unavailable")),
                }
            }
            if !self.no_backup {
                match main.make_backup() {
                    Ok(b) => self.info(format!("Backup: {}", b.display())),
                    Err(e) => self.error(format!("Backup of {} failed: {e}", path.display())),
                }
            }
            character = Some(main);
        }
        if let Some(p) = profile.as_mut() {
            if !self.no_backup {
                match p.make_backup() {
                    Ok(b) => self.info(format!("Backup: {}", b.display())),
                    Err(e) => self.error(format!("Backup of profile.sav failed: {e}")),
                }
            }
        }
        let name = character
            .as_mut()
            .map(|c| save::Character(&mut c.doc).name())
            .unwrap_or_else(|| "profile".into());
        self.info(format!("Opened {} ({name})", path.display()));
        if self.game_running {
            self.warn("Borderlands 4 is running: you can look and edit, but saving is blocked until the game is closed (it would overwrite your changes).");
        }
        self.tab = if character.is_some() { Tab::Character } else { Tab::Bank };
        self.items = ItemsState::default();
        self.pages = PagesState::default();
        self.sess = Some(Session { dir, character, profile, char_dirty: false, prof_dirty: false, undo: vec![], redo: vec![] });
    }

    /// Apply an edit to one document with undo support.
    pub fn edit(&mut self, w: Which, f: impl FnOnce(&mut Node)) {
        let Some(s) = self.sess.as_mut() else { return };
        let snap = s.snapshot();
        let doc = match w {
            Which::Character => s.character.as_mut().map(|x| &mut x.doc),
            Which::Profile => s.profile.as_mut().map(|x| &mut x.doc),
        };
        if let Some(d) = doc {
            f(d);
            s.undo.push(snap);
            if s.undo.len() > 100 {
                s.undo.remove(0);
            }
            s.redo.clear();
            match w {
                Which::Character => s.char_dirty = true,
                Which::Profile => s.prof_dirty = true,
            }
        }
    }

    /// Edit both documents at once (e.g. moving an item backpack <-> bank).
    pub fn edit_both(&mut self, f: impl FnOnce(&mut Node, &mut Node)) {
        let Some(s) = self.sess.as_mut() else { return };
        let snap = s.snapshot();
        if let (Some(c), Some(p)) = (s.character.as_mut(), s.profile.as_mut()) {
            f(&mut c.doc, &mut p.doc);
            s.undo.push(snap);
            s.redo.clear();
            s.char_dirty = true;
            s.prof_dirty = true;
        }
    }

    fn undo(&mut self) {
        if let Some(s) = self.sess.as_mut() {
            if let Some(snap) = s.undo.pop() {
                let cur = s.snapshot();
                s.restore(snap);
                s.redo.push(cur);
            }
        }
    }

    fn redo(&mut self) {
        if let Some(s) = self.sess.as_mut() {
            if let Some(snap) = s.redo.pop() {
                let cur = s.snapshot();
                s.restore(snap);
                s.undo.push(cur);
            }
        }
    }

    pub fn save_all(&mut self) {
        self.game_running = save::game_running();
        if self.game_running {
            self.error("Not saved: Borderlands 4 is running. Quit the game completely, then save again. (The game keeps the character in memory and would overwrite the file.)");
            return;
        }
        // structural self-check: never write something the game would not write
        if let Some(s) = self.sess.as_ref() {
            let mut problems = vec![];
            if let (true, Some(c)) = (s.char_dirty, s.character.as_ref()) {
                problems.extend(save::check_invariants(&c.doc, SaveKind::Character));
            }
            if let (true, Some(p)) = (s.prof_dirty, s.profile.as_ref()) {
                problems.extend(save::check_invariants(&p.doc, SaveKind::Profile));
            }
            if !problems.is_empty() {
                self.error(format!("Not saved - the edit broke the save structure: {}. Use Undo.", problems.join("; ")));
                return;
            }
        }
        let Some(s) = self.sess.as_mut() else { return };
        let mut msgs = vec![];
        if s.char_dirty {
            if let Some(c) = s.character.as_mut() {
                match c.write() {
                    Ok(()) => {
                        s.char_dirty = false;
                        msgs.push((Level::Info, format!("Saved {}", c.path.display())));
                    }
                    Err(e) => msgs.push((Level::Error, format!("Saving {} failed: {e}", c.path.display()))),
                }
            }
        }
        if s.prof_dirty {
            if let Some(p) = s.profile.as_mut() {
                match p.write() {
                    Ok(()) => {
                        s.prof_dirty = false;
                        msgs.push((Level::Info, format!("Saved {}", p.path.display())));
                    }
                    Err(e) => msgs.push((Level::Error, format!("Saving {} failed: {e}", p.path.display()))),
                }
            }
        }
        if msgs.is_empty() {
            msgs.push((Level::Info, "Nothing to save".into()));
        }
        self.log.extend(msgs);
        if let Some(d) = self.folder.clone() {
            self.set_folder(&d);
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("BL4 Save Editor").color(Color32::from_rgb(255, 170, 40)));
            ui.separator();
            if ui.button("Open save…").clicked() {
                let mut dlg = rfd::FileDialog::new().add_filter("Borderlands 4 save", &["sav"]);
                if let Some(d) = &self.folder {
                    dlg = dlg.set_directory(d);
                }
                if let Some(p) = dlg.pick_file() {
                    self.request_open(&p);
                }
            }
            if ui.button("Save folder…").clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    self.set_folder(&d);
                }
            }
            let can_save = self.sess.is_some() && !self.game_running;
            let save_btn = ui.add_enabled(can_save, egui::Button::new(if self.any_dirty() { "Save *" } else { "Save" }));
            if save_btn.clicked() || (can_save && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S))) {
                self.save_all();
            }
            let has_undo = self.sess.as_ref().map(|s| !s.undo.is_empty()).unwrap_or(false);
            let has_redo = self.sess.as_ref().map(|s| !s.redo.is_empty()).unwrap_or(false);
            if ui.add_enabled(has_undo, egui::Button::new("Undo")).clicked()
                || (has_undo && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)))
            {
                self.undo();
                self.items.invalidate();
            }
            if ui.add_enabled(has_redo, egui::Button::new("Redo")).clicked()
                || (has_redo && ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Y)))
            {
                self.redo();
                self.items.invalidate();
            }
            ui.separator();
            ui.label("Steam ID:");
            ui.add(egui::TextEdit::singleline(&mut self.steam_id).desired_width(150.0).hint_text("from the save folder"));
            if self.game_running {
                ui.separator();
                ui.label(
                    RichText::new(" Borderlands 4 is running - saving disabled ")
                        .color(Color32::WHITE)
                        .background_color(Color32::from_rgb(170, 30, 30)),
                );
            }
        });
    }

    fn request_open(&mut self, p: &Path) {
        if self.any_dirty() {
            self.confirm_discard = Some(p.to_path_buf());
        } else {
            if let Some(d) = p.parent() {
                if self.folder.as_deref() != Some(d) {
                    self.set_folder(d);
                }
            }
            self.open(p);
        }
    }

    fn save_list(&mut self, ui: &mut egui::Ui) {
        ui.strong("Saves");
        if let Some(f) = &self.folder {
            ui.label(RichText::new(f.display().to_string()).small().weak());
        } else {
            ui.label("No save folder found. Use Open save…");
        }
        ui.separator();
        let current = self
            .sess
            .as_ref()
            .and_then(|s| s.character.as_ref().or(s.profile.as_ref()))
            .map(|f| f.path.clone());
        let mut clicked = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for e in &self.saves {
                let sel = current.as_ref() == Some(&e.path);
                let r = ui.selectable_label(sel, RichText::new(&e.label).strong());
                ui.label(RichText::new(&e.detail).small().weak());
                ui.add_space(2.0);
                if r.clicked() {
                    clicked = Some(e.path.clone());
                }
            }
        });
        if let Some(p) = clicked {
            self.request_open(&p);
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some((lvl, msg)) = self.log.last() {
                let c = match lvl {
                    Level::Info => Color32::from_gray(200),
                    Level::Warn => Color32::from_rgb(255, 190, 60),
                    Level::Error => Color32::from_rgb(255, 90, 90),
                };
                ui.label(RichText::new(msg).color(c));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button(format!("Log ({})", self.log.len()), |ui| {
                    egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                        for (lvl, m) in self.log.iter().rev() {
                            let c = match lvl {
                                Level::Info => Color32::from_gray(200),
                                Level::Warn => Color32::from_rgb(255, 190, 60),
                                Level::Error => Color32::from_rgb(255, 90, 90),
                            };
                            ui.label(RichText::new(m).color(c));
                        }
                    });
                });
            });
        });
    }

    fn central(&mut self, ui: &mut egui::Ui) {
        let Some(sess) = &self.sess else {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("Open a save to start");
                ui.label("Pick a character on the left, or use Open save… to load a .sav file.");
                ui.label("Every save is backed up (bl4editor_backups folder next to it) the moment it is opened.");
            });
            return;
        };
        let has_char = sess.character.is_some();
        let has_prof = sess.profile.is_some();
        let tabs: Vec<Tab> = [
            (Tab::Character, has_char),
            (Tab::Currency, has_char || has_prof),
            (Tab::Progression, has_char || has_prof),
            (Tab::Backpack, has_char),
            (Tab::LostLoot, has_char),
            (Tab::Bank, has_prof),
            (Tab::Missions, has_char),
            (Tab::Appearance, has_char || has_prof),
            (Tab::Raw, true),
        ]
        .into_iter()
        .filter(|x| x.1)
        .map(|x| x.0)
        .collect();
        if !tabs.contains(&self.tab) {
            self.tab = tabs[0];
        }
        ui.horizontal(|ui| {
            for t in &tabs {
                if ui.selectable_label(self.tab == *t, RichText::new(t.label()).size(15.0)).clicked() {
                    self.tab = *t;
                }
            }
        });
        ui.separator();
        match self.tab {
            Tab::Character => crate::pages::character(self, ui),
            Tab::Currency => crate::pages::currency(self, ui),
            Tab::Progression => crate::pages::progression(self, ui),
            Tab::Backpack => crate::items::inventory(self, ui, crate::items::View::Backpack),
            Tab::LostLoot => crate::items::inventory(self, ui, crate::items::View::LostLoot),
            Tab::Bank => crate::items::inventory(self, ui, crate::items::View::Bank),
            Tab::Missions => crate::pages::missions(self, ui),
            Tab::Appearance => crate::pages::appearance(self, ui),
            Tab::Raw => crate::pages::raw(self, ui),
        }
    }

    fn handle_screenshot(&mut self, ui: &mut egui::Ui) {
        let Some(out) = self.screenshot.clone() else { return };
        self.frame_no += 1;
        let shot = ui.ctx().input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let mut buf = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                buf.extend_from_slice(&p.to_array());
            }
            let _ = image::save_buffer(&out, &buf, w as u32, h as u32, image::ColorType::Rgba8);
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.frame_no == 12 {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ui.ctx().request_repaint();
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.last_check.elapsed().as_secs() >= 3 {
            self.game_running = save::game_running();
            self.last_check = Instant::now();
        }
        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("saves").resizable(true).default_size(230.0).show(ui, |ui| self.save_list(ui));
        egui::CentralPanel::default().show(ui, |ui| self.central(ui));

        if let Some(p) = self.confirm_discard.clone() {
            egui::Window::new("Unsaved changes").collapsible(false).resizable(false).show(ui.ctx(), |ui| {
                ui.label("The open save has unsaved changes. Discard them?");
                ui.horizontal(|ui| {
                    if ui.button("Discard and open").clicked() {
                        self.confirm_discard = None;
                        if let Some(d) = p.parent() {
                            self.set_folder(d);
                        }
                        self.open(&p);
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_discard = None;
                    }
                });
            });
        }
        self.handle_screenshot(ui);
        // keep polling the game state even when idle
        ui.ctx().request_repaint_after(std::time::Duration::from_secs(3));
    }
}

pub fn class_label(class: &str) -> &'static str {
    match class {
        "Char_DarkSiren" => "Vex (Siren)",
        "Char_Paladin" => "Amon (Forgeknight)",
        "Char_ExoSoldier" => "Rafa (Exo-Soldier)",
        "Char_Gravitar" => "Harlowe (Gravitar)",
        "Char_RoboDealer" => "C4SH (Rogue)",
        "Char_CorpoHacker" => "Corpo Hacker",
        _ => "Unknown class",
    }
}

fn setup_style(ctx: &egui::Context) {
    ctx.set_visuals(egui::Visuals::dark());
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 5.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
    });
}
