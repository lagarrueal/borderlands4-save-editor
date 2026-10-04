//! Shared widgets.

use eframe::egui;

pub struct Opt<T> {
    pub value: T,
    pub label: String,
    /// extra text the search also matches (part effect descriptions, keys)
    pub extra: String,
    pub selected: bool,
    pub enabled: bool,
}

impl<T> Opt<T> {
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Opt { value, label: label.into(), extra: String::new(), selected: false, enabled: true }
    }
    pub fn extra(mut self, e: impl Into<String>) -> Self {
        self.extra = e.into();
        self
    }
    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        self
    }
    pub fn enabled(mut self, e: bool) -> Self {
        self.enabled = e;
        self
    }
}

/// A combo box with a search field at the top of its list. Every word typed
/// must appear in the label or the extra text (case-insensitive).
pub fn search_combo<T: Clone>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    width: f32,
    selected_text: impl Into<egui::WidgetText>,
    options: Vec<Opt<T>>,
) -> Option<T> {
    let id = egui::Id::new(&id_salt);
    let fid = id.with("filter");
    let mut chosen = None;
    let resp = egui::ComboBox::from_id_salt(id).width(width).height(420.0).selected_text(selected_text).show_ui(ui, |ui| {
        let mut filter: String = ui.data_mut(|d| d.get_temp(fid).unwrap_or_default());
        let r = ui.add(egui::TextEdit::singleline(&mut filter).hint_text("type to search…").desired_width(f32::INFINITY));
        if !r.has_focus() && filter.is_empty() {
            r.request_focus();
        }
        ui.data_mut(|d| d.insert_temp(fid, filter.clone()));
        let words: Vec<String> = filter.to_lowercase().split_whitespace().map(|s| s.to_string()).collect();
        let mut shown = 0;
        for o in &options {
            if !words.is_empty() {
                let hay = format!("{} {}", o.label, o.extra).to_lowercase();
                if !words.iter().all(|w| hay.contains(w)) {
                    continue;
                }
            }
            shown += 1;
            if ui.add_enabled(o.enabled, egui::Button::selectable(o.selected, o.label.as_str())).clicked() {
                chosen = Some(o.value.clone());
            }
        }
        if shown == 0 {
            ui.label(egui::RichText::new("no match").weak());
        }
    });
    if chosen.is_some() || (!resp.response.has_focus() && resp.inner.is_none()) {
        // reset the search when the list closes
        if chosen.is_some() {
            ui.data_mut(|d| d.remove::<String>(fid));
        }
    }
    chosen
}
