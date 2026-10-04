#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! BL4 Save Editor: an offline, Gibbed-style save editor for Borderlands 4.

mod app;
mod icons;
mod items;
mod pages;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let mut opts = app::StartOptions::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" if i + 1 < args.len() => {
                opts.screenshot = Some(args[i + 1].clone().into());
                i += 1;
            }
            "--tab" if i + 1 < args.len() => {
                opts.tab = Some(args[i + 1].clone());
                i += 1;
            }
            "--select" if i + 1 < args.len() => {
                opts.select = args[i + 1].parse().ok();
                i += 1;
            }
            "--steam-id" if i + 1 < args.len() => {
                opts.steam_id = Some(args[i + 1].clone());
                i += 1;
            }
            "--no-backup" => opts.no_backup = true,
            other => opts.open = Some(other.into()),
        }
        i += 1;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 860.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("BL4 Save Editor"),
        ..Default::default()
    };
    eframe::run_native(
        "BL4 Save Editor",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::App::new(&cc.egui_ctx, opts)))
        }),
    )
}
