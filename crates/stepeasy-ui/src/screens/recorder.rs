//! Tela de gravação: escolher o escopo e começar.

use egui::{RichText, Vec2};
use rust_i18n::t;
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

            ui.label(RichText::new(t!("gravador.titulo")).size(24.0).strong());
            ui.add_space(6.0);
            ui.label(RichText::new(t!("gravador.subtitulo")).color(palette.muted));
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
            ui.checkbox(&mut app.minimize_while_recording, t!("gravador.minimizar"));

            ui.add_space(20.0);
            let botao = egui::Button::new(
                RichText::new(t!("gravador.iniciar"))
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
                    RichText::new(t!("gravador.continuar", passos = passos)).size(14.0),
                )
                .min_size(Vec2::new(240.0, 38.0));
                if ui
                    .add(continuar)
                    .on_hover_text(t!("gravador.continuar_dica"))
                    .clicked()
                {
                    app.continue_recording(&ctx);
                }
            }

            ui.add_space(10.0);
            ui.label(
                RichText::new(t!(
                    "gravador.atalhos",
                    parar = STOP_HOTKEY,
                    pausar = PAUSE_HOTKEY
                ))
                .color(palette.muted)
                .size(12.0),
            );

            if !stepeasy_capture::is_supported() {
                ui.add_space(16.0);
                ui.label(RichText::new(t!("gravador.sem_suporte")).color(palette.accent));
            }
        });
    });
}

fn gravando(app: &mut App, ui: &mut egui::Ui, palette: &theme::Palette) {
    let pausado = app.is_paused();

    let (titulo, cor) = if pausado {
        (t!("gravador.pausado"), palette.muted)
    } else {
        (t!("gravador.gravando"), palette.accent)
    };
    ui.label(RichText::new(titulo).size(24.0).strong().color(cor));

    ui.add_space(8.0);
    let contagem = if app.continuing {
        t!("gravador.acrescentados", n = app.recorded_steps)
    } else {
        t!("gravador.capturados", n = app.recorded_steps)
    };
    ui.label(RichText::new(contagem).size(16.0).color(palette.muted));

    ui.add_space(24.0);

    ui.horizontal(|ui| {
        // Centraliza a dupla de botões dentro do painel centralizado.
        let largura = 220.0 + 160.0 + ui.spacing().item_spacing.x;
        ui.add_space((ui.available_width() - largura).max(0.0) / 2.0);

        let encerrar =
            egui::Button::new(RichText::new(t!("gravador.encerrar")).size(16.0).strong())
                .min_size(Vec2::new(220.0, 42.0));
        if ui.add(encerrar).clicked() {
            let ctx = ui.ctx().clone();
            app.stop_recording(&ctx);
        }

        let rotulo = if pausado {
            t!("gravador.retomar")
        } else {
            t!("gravador.pausar")
        };
        let pausar =
            egui::Button::new(RichText::new(rotulo).size(16.0)).min_size(Vec2::new(160.0, 42.0));
        if ui.add(pausar).clicked() {
            app.toggle_pause();
        }
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new(t!(
            "gravador.atalhos_gravando",
            parar = STOP_HOTKEY,
            pausar = PAUSE_HOTKEY
        ))
        .color(palette.muted)
        .size(12.0),
    );

    if pausado {
        ui.add_space(6.0);
        ui.label(
            RichText::new(t!("gravador.pausa_aviso"))
                .color(palette.muted)
                .size(12.0),
        );
    }
}

fn escopo(app: &mut App, ui: &mut egui::Ui, palette: &theme::Palette) {
    ui.label(RichText::new(t!("gravador.escopo_titulo")).strong());
    ui.add_space(6.0);

    egui::ComboBox::from_id_salt("escopo")
        .width(COLUNA - 24.0)
        .selected_text(app.scope.label())
        .show_ui(ui, |ui| {
            // A opção "tela específica" não é um valor pronto: ela precisa
            // escolher qual monitor, então fica fora desta lista.
            for opcao in [
                CaptureScope::MonitorUnderCursor,
                CaptureScope::ActiveWindow,
                CaptureScope::AllMonitors,
            ] {
                let ativa = std::mem::discriminant(&app.scope) == std::mem::discriminant(&opcao);
                if ui.selectable_label(ativa, opcao.label()).clicked() {
                    app.scope = opcao;
                }
            }

            let selecionada = matches!(app.scope, CaptureScope::Monitor { .. });
            let rotulo = t!("escopo.tela_especifica");
            if ui.selectable_label(selecionada, rotulo).clicked() && !selecionada {
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
            .unwrap_or_else(|| t!("escopo.tela_especifica").to_string());

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
                RichText::new(t!("gravador.sem_telas"))
                    .color(palette.accent)
                    .size(12.0),
            );
        }
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(app.scope.help())
            .color(palette.muted)
            .size(12.0),
    );
}
