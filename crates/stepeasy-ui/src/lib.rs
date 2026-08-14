//! Interface do StepEasy.

mod app;
pub mod icons;
mod screens;
mod textures;
mod theme;
mod toast;

pub use app::App;

/// Todos os ícones, na ordem em que a vitrine (`--example icones`) os mostra.
pub const ICONES: &[icons::Icon] = &[
    icons::Icon::Undo,
    icons::Icon::Redo,
    icons::Icon::ChevronUp,
    icons::Icon::ChevronDown,
    icons::Icon::Plus,
    icons::Icon::Close,
    icons::Icon::Sun,
    icons::Icon::Moon,
];

pub use icons::Icon;
pub use theme::apply as aplicar_tema;

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
