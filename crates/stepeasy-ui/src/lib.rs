//! Interface do StepEasy.

mod annotate;
mod app;
pub mod icons;
mod recovery;
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

/// Ícone da janela, gerado a partir de `assets/logo/stepeasy_icon.svg` pelo
/// exemplo `gerar_icones`. Vai embutido para o binário não depender de arquivo
/// nenhum ao lado dele.
fn icone() -> Option<std::sync::Arc<egui::IconData>> {
    const PNG: &[u8] = include_bytes!("../../../assets/icons/stepeasy-256.png");

    match eframe::icon_data::from_png_bytes(PNG) {
        Ok(icone) => Some(std::sync::Arc::new(icone)),
        Err(err) => {
            tracing::warn!("ícone da janela inválido: {err}");
            None
        }
    }
}

/// Abre a janela do aplicativo. Só retorna quando o usuário fecha.
///
/// `abrir` é uma gravação `.stepeasy` para carregar já no editor.
pub fn run(abrir: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("StepEasy")
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([900.0, 560.0])
        .with_app_id("stepeasy");

    if let Some(icone) = icone() {
        viewport = viewport.with_icon(icone);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "StepEasy",
        options,
        Box::new(move |cc| {
            let mut app = App::new(cc);
            if let Some(path) = abrir {
                app.open_path(&path);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("não foi possível abrir a janela: {e}"))
}
