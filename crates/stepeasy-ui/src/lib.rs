//! Interface do StepEasy.

mod app;
mod screens;
mod textures;
mod theme;
mod toast;

pub use app::App;

/// Abre a janela do aplicativo. Só retorna quando o usuário fecha.
pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("StepEasy")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0])
            .with_app_id("stepeasy"),
        ..Default::default()
    };

    eframe::run_native(
        "StepEasy",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("não foi possível abrir a janela: {e}"))
}
