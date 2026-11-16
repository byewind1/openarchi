#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    if std::env::args().any(|arg| arg == "--version") {
        println!("OpenArchi {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if let Some(code) = atelier_bim::plugins::worker::main_if_requested() {
        std::process::exit(code);
    }
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
        Box::new(|cc| {
            let app = atelier_bim::app::AtelierApp::new(cc);
            if let Some(path) = std::env::var_os("OPENARCHI_NATIVE_SMOKE_REPORT") {
                let adapter = cc.wgpu_render_state.as_ref().map(|state| {
                    let info = state.adapter.get_info();
                    serde_json::json!({"name":info.name,"backend":format!("{:?}",info.backend)})
                });
                Ok(Box::new(NativeSmoke {
                    app,
                    report: path.into(),
                    frames: 0,
                    adapter,
                }))
            } else {
                Ok(Box::new(app))
            }
        }),
    )
}

/// Explicit QA mode: exercise the packaged executable's real window/GPU loop.
/// The report is intentionally narrower than interactive feature acceptance.
struct NativeSmoke {
    app: atelier_bim::app::AtelierApp,
    report: std::path::PathBuf,
    frames: u32,
    adapter: Option<serde_json::Value>,
}
impl eframe::App for NativeSmoke {
    fn logic(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
    }
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        self.frames += 1;
        if self.frames == 30 {
            let value = serde_json::json!({"version":env!("CARGO_PKG_VERSION"),"os":std::env::consts::OS,
                "arch":std::env::consts::ARCH,"frames":self.frames,"adapter":self.adapter,
                "scope":"Packaged native window and GPU startup only; interaction acceptance is separate"});
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.report)
                .and_then(|mut file| file.write_all(value.to_string().as_bytes()))
                .expect("write native smoke report");
            ui.ctx()
                .send_viewport_cmd(eframe::egui::ViewportCommand::Close);
        }
        ui.ctx().request_repaint();
    }
}
