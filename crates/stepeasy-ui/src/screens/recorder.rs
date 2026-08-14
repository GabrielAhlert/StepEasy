//! Tela de gravação: escolher o escopo e começar.

use egui::{RichText, Vec2};
use stepeasy_core::scope::CaptureScope;

use crate::app::{App, PAUSE_HOTKEY, STOP_HOTKEY};
use crate::theme;

/// Largura da coluna central da tela de gravação.
const COLUNA: f32 = 380.0;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let palette = theme::palette(&ctx);

    egui::CentralPanel::default().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(48.0);

            if app.is_recording() {
                gravando(app, ui, &palette);
                return;
            }

            ui.label(RichText::new("Gravar um passo a passo").size(24.0).strong());
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "Cada clique e cada trecho digitado vira um passo com captura de tela. \
                     Você revisa e reorganiza tudo antes de exportar.",
                )
                .color(palette.muted),
            );
            ui.add_space(28.0);

            // O `ComboBox` não obedece à centralização do painel: ele se ancora
            // à esquerda da área disponível, que aqui é a janela inteira. Dar a
            // ele uma coluna de largura fixa faz o painel centralizar a coluna,
            // e aí tudo dentro dela sai alinhado.
            ui.allocate_ui_with_layout(
                Vec2::new(COLUNA, 0.0),
                egui::Layout::top_down(egui::Align::Center),
                |ui| escopo(app, ui, &palette),
            );

            ui.add_space(24.0);
            ui.checkbox(
                &mut app.minimize_while_recording,
                "Minimizar o StepEasy durante a gravação",
            );

            ui.add_space(20.0);
            let botao = egui::Button::new(
                RichText::new("Iniciar gravação")
                    .size(17.0)
                    .strong()
                    .color(palette.accent_fg),
            )
            .fill(palette.accent)
            .min_size(Vec2::new(240.0, 46.0));
            if ui.add(botao).clicked() {
                app.start_recording(&ctx);
            }

            if app.can_continue_recording() {
                let passos = app.project.as_ref().map_or(0, |p| p.recording.steps.len());
                ui.add_space(8.0);
                let continuar = egui::Button::new(
                    RichText::new(format!("Continuar a gravação aberta ({passos} passos)"))
                        .size(14.0),
                )
                .min_size(Vec2::new(240.0, 38.0));
                if ui
                    .add(continuar)
                    .on_hover_text(
                        "Os passos novos entram no fim da gravação, sem apagar os que já existem",
                    )
                    .clicked()
                {
                    app.continue_recording(&ctx);
                }
            }

            ui.add_space(10.0);
            ui.label(
                RichText::new(format!(
                    "Durante a gravação: {STOP_HOTKEY} encerra, {PAUSE_HOTKEY} pausa e retoma"
                ))
                .color(palette.muted)
                .size(12.0),
            );

            if !stepeasy_capture::is_supported() {
                ui.add_space(16.0);
                ui.label(
                    RichText::new(
                        "A captura de entrada ainda não está disponível neste sistema \
                         operacional. Você pode abrir e editar gravações existentes.",
                    )
                    .color(palette.accent),
                );
            }
        });
    });
}

fn gravando(app: &mut App, ui: &mut egui::Ui, palette: &theme::Palette) {
    let pausado = app.is_paused();

    let (titulo, cor) = if pausado {
        ("❙❙ Pausado", palette.muted)
    } else {
        ("● Gravando", palette.accent)
    };
    ui.label(RichText::new(titulo).size(24.0).strong().color(cor));

    ui.add_space(8.0);
    let contagem = if app.continuing {
        format!("{} passo(s) acrescentado(s)", app.recorded_steps)
    } else {
        format!("{} passo(s) capturado(s)", app.recorded_steps)
    };
    ui.label(RichText::new(contagem).size(16.0).color(palette.muted));

    ui.add_space(24.0);

    ui.horizontal(|ui| {
        // Centraliza a dupla de botões dentro do painel centralizado.
        let largura = 220.0 + 160.0 + ui.spacing().item_spacing.x;
        ui.add_space((ui.available_width() - largura).max(0.0) / 2.0);

        let encerrar = egui::Button::new(RichText::new("Encerrar gravação").size(16.0).strong())
            .min_size(Vec2::new(220.0, 42.0));
        if ui.add(encerrar).clicked() {
            let ctx = ui.ctx().clone();
            app.stop_recording(&ctx);
        }

        let rotulo = if pausado { "Retomar" } else { "Pausar" };
        let pausar =
            egui::Button::new(RichText::new(rotulo).size(16.0)).min_size(Vec2::new(160.0, 42.0));
        if ui.add(pausar).clicked() {
            app.toggle_pause();
        }
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new(format!(
            "de qualquer lugar: {STOP_HOTKEY} encerra, {PAUSE_HOTKEY} pausa e retoma"
        ))
        .color(palette.muted)
        .size(12.0),
    );

    if pausado {
        ui.add_space(6.0);
        ui.label(
            RichText::new("Nada está sendo capturado enquanto a pausa durar.")
                .color(palette.muted)
                .size(12.0),
        );
    }
}

fn escopo(app: &mut App, ui: &mut egui::Ui, palette: &theme::Palette) {
    ui.label(RichText::new("O que capturar").strong());
    ui.add_space(6.0);

    let atual = app.scope.label().to_string();
    egui::ComboBox::from_id_salt("escopo")
        .width(COLUNA - 24.0)
        .selected_text(atual)
        .show_ui(ui, |ui| {
            let sob_cursor = CaptureScope::MonitorUnderCursor;
            if ui
                .selectable_label(
                    matches!(app.scope, CaptureScope::MonitorUnderCursor),
                    sob_cursor.label(),
                )
                .clicked()
            {
                app.scope = sob_cursor;
            }

            let ativa = CaptureScope::ActiveWindow;
            if ui
                .selectable_label(
                    matches!(app.scope, CaptureScope::ActiveWindow),
                    ativa.label(),
                )
                .clicked()
            {
                app.scope = ativa;
            }

            let todas = CaptureScope::AllMonitors;
            if ui
                .selectable_label(
                    matches!(app.scope, CaptureScope::AllMonitors),
                    todas.label(),
                )
                .clicked()
            {
                app.scope = todas;
            }

            let selecionada = matches!(app.scope, CaptureScope::Monitor { .. });
            if ui
                .selectable_label(selecionada, "Tela específica")
                .clicked()
                && !selecionada
            {
                let id = app
                    .monitors
                    .iter()
                    .find(|m| m.is_primary)
                    .or_else(|| app.monitors.first())
                    .map(|m| m.id.clone())
                    .unwrap_or_default();
                app.scope = CaptureScope::Monitor { id };
            }
        });

    if let CaptureScope::Monitor { id } = app.scope.clone() {
        ui.add_space(8.0);
        let atual = app
            .monitors
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.display_label())
            .unwrap_or_else(|| "Selecione uma tela".to_string());

        egui::ComboBox::from_id_salt("monitor")
            .width(COLUNA - 24.0)
            .selected_text(atual)
            .show_ui(ui, |ui| {
                for monitor in &app.monitors {
                    if ui
                        .selectable_label(monitor.id == id, monitor.display_label())
                        .clicked()
                    {
                        app.scope = CaptureScope::Monitor {
                            id: monitor.id.clone(),
                        };
                    }
                }
            });

        if app.monitors.is_empty() {
            ui.label(
                RichText::new("Nenhuma tela foi detectada.")
                    .color(palette.accent)
                    .size(12.0),
            );
        }
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(explicacao(&app.scope))
            .color(palette.muted)
            .size(12.0),
    );
}

fn explicacao(scope: &CaptureScope) -> &'static str {
    match scope {
        CaptureScope::MonitorUnderCursor => {
            "Cada captura pega a tela onde o clique aconteceu. É o modo mais seguro \
             quando você usa mais de um monitor."
        }
        CaptureScope::ActiveWindow => {
            "Só a janela em foco entra na imagem, sem área de trabalho nem barra de tarefas. \
             Cliques em menus suspensos caem para a tela inteira."
        }
        CaptureScope::AllMonitors => {
            "Todas as telas lado a lado numa imagem só. Gera arquivos grandes."
        }
        CaptureScope::Monitor { .. } => {
            "Sempre a mesma tela, mesmo que o clique aconteça em outra."
        }
        CaptureScope::Region { .. } => "Sempre a mesma região da tela.",
    }
}
