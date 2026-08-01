// Suppresses the console window that would otherwise pop up behind the GUI
// on Windows; kept in debug builds so println!/eprintln! debugging still works.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod i18n;

/// Everything about the native window that's plain data/decoding, no
/// display or GPU needed, split out from `main()` so it's exercisable from a
/// test — unlike `eframe::run_native` itself, which needs a real windowing
/// backend and so can't run headlessly at all.
fn native_options() -> eframe::NativeOptions {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .expect("bundled icon.png is a valid PNG");
    eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([480.0, 640.0])
            .with_min_inner_size([360.0, 420.0])
            .with_icon(icon),
        ..Default::default()
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "vm-check",
        native_options(),
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_options_decodes_the_bundled_icon_without_panicking() {
        let options = native_options();
        let icon = options
            .viewport
            .icon
            .expect("native_options always sets an icon");
        assert!(!icon.rgba.is_empty());
    }
}
