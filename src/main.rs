#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../icon.png"))
        .expect("failed to load OpenArchi application icon");
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            // eframe derives the macOS application-menu label from this
            // initial title. Keep it language-neutral; the localized window
            // title is applied from the first frame below.
            .with_title("OpenArchi")
            .with_icon(icon)
            .with_inner_size([1512., 960.])
            .with_min_inner_size([1050., 700.]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "OpenArchi",
        options,
        Box::new(|cc| Ok(Box::new(atelier_bim::app::AtelierApp::new(cc)))),
    )
}
