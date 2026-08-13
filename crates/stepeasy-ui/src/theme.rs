//! Paleta e estilo do aplicativo.
//!
//! O egui vem com um tema escuro e um claro razoáveis, mas genéricos demais.
//! Aqui os dois são reescritos a partir de uma paleta única, para que a
//! aparência não dependa de qual tema está ativo — só as cores mudam.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Cor de destaque do produto, a mesma do marcador de clique.
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0xE0, 0x2B, 0x20);
pub const ACCENT_DARK: Color32 = Color32::from_rgb(0xFF, 0x6B, 0x5E);

pub struct Palette {
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub fg: Color32,
    pub muted: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub accent_fg: Color32,
    pub selection: Color32,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            bg: Color32::from_rgb(0xF7, 0xF7, 0xF8),
            panel: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            fg: Color32::from_rgb(0x1B, 0x1B, 0x1F),
            muted: Color32::from_rgb(0x6B, 0x6B, 0x76),
            line: Color32::from_rgb(0xE3, 0xE3, 0xE8),
            accent: ACCENT_LIGHT,
            accent_fg: Color32::WHITE,
            selection: Color32::from_rgb(0xFB, 0xDE, 0xDC),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: Color32::from_rgb(0x13, 0x13, 0x16),
            panel: Color32::from_rgb(0x1C, 0x1C, 0x21),
            card: Color32::from_rgb(0x23, 0x23, 0x29),
            fg: Color32::from_rgb(0xEC, 0xEC, 0xF1),
            muted: Color32::from_rgb(0x9A, 0x9A, 0xA6),
            line: Color32::from_rgb(0x2C, 0x2C, 0x34),
            accent: ACCENT_DARK,
            accent_fg: Color32::from_rgb(0x2A, 0x0B, 0x08),
            selection: Color32::from_rgb(0x3A, 0x22, 0x20),
        }
    }

    pub fn for_dark_mode(dark: bool) -> Self {
        if dark {
            Self::dark()
        } else {
            Self::light()
        }
    }
}

/// Aplica a paleta aos dois temas e fixa qual deles está ativo.
///
/// Os dois são configurados sempre, e não só o atual: assim alternar o tema
/// não depende de reconfigurar nada, e a janela já nasce certa se o sistema
/// mudar de claro para escuro.
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::ThemePreference::Dark
    } else {
        egui::ThemePreference::Light
    });

    for tema in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.set_visuals_of(tema, visuals_for(tema == egui::Theme::Dark));
        ctx.style_mut_of(tema, shape);
    }
}

fn visuals_for(dark: bool) -> Visuals {
    let p = Palette::for_dark_mode(dark);

    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.override_text_color = Some(p.fg);
    visuals.panel_fill = p.panel;
    visuals.window_fill = p.panel;
    visuals.extreme_bg_color = p.bg;
    visuals.faint_bg_color = p.bg;
    visuals.window_stroke = Stroke::new(1.0, p.line);
    visuals.selection.bg_fill = p.selection;
    visuals.selection.stroke = Stroke::new(1.0, p.accent);
    visuals.hyperlink_color = p.accent;

    let radius = CornerRadius::same(8);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
    }
    visuals.widgets.noninteractive.bg_fill = p.card;
    visuals.widgets.noninteractive.weak_bg_fill = p.card;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);

    visuals.widgets.inactive.bg_fill = p.card;
    visuals.widgets.inactive.weak_bg_fill = p.card;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, p.line);

    visuals.widgets.hovered.bg_fill = p.selection;
    visuals.widgets.hovered.weak_bg_fill = p.selection;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);

    visuals.widgets.active.bg_fill = p.accent;
    visuals.widgets.active.weak_bg_fill = p.accent;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, p.accent);

    visuals.window_corner_radius = CornerRadius::same(10);
    visuals
}

fn shape(style: &mut egui::Style) {
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(12);
    style.spacing.interact_size.y = 28.0;
}

/// Cores da paleta ativa, para desenhar coisas que não são widgets padrão
/// (a timeline, o marcador de clique, os rótulos secundários).
pub fn palette(ctx: &egui::Context) -> Palette {
    Palette::for_dark_mode(ctx.theme() == egui::Theme::Dark)
}
