//! Barra superior e barra de status — o que fica visível nas duas telas.

use egui::{Align, Layout, RichText};
use stepeasy_core::export::Format;

use crate::app::{App, Dialogo, Screen};
use crate::icons::{self, Icon};
use crate::theme;
use crate::toast::Level;

pub fn top_bar(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    egui::Panel::top("top")
        .exact_size(52.0)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(4.0);
                ui.label(RichText::new("StepEasy").size(18.0).strong());
                ui.add_space(12.0);

                let gravando = app.is_recording();
                ui.add_enabled_ui(!gravando, |ui| {
                    if ui
                        .selectable_label(app.screen == Screen::Recorder, "Gravar")
                        .clicked()
                    {
                        app.screen = Screen::Recorder;
                    }
                    let tem_projeto = app.project.is_some();
                    ui.add_enabled_ui(tem_projeto, |ui| {
                        if ui
                            .selectable_label(app.screen == Screen::Editor, "Editar")
                            .clicked()
                        {
                            app.screen = Screen::Editor;
                        }
                    });
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (icone, dica) = if app.dark {
                        (Icon::Sun, "Mudar para o tema claro")
                    } else {
                        (Icon::Moon, "Mudar para o tema escuro")
                    };
                    if icons::button(ui, icone).on_hover_text(dica).clicked() {
                        app.dark = !app.dark;
                        theme::apply(&ctx, app.dark);
                    }

                    ui.add_enabled_ui(!gravando, |ui| {
                        let tem_projeto = app.project.is_some();
                        ui.add_enabled_ui(tem_projeto, |ui| {
                            ui.menu_button("Exportar", |ui| {
                                if ui.button(Format::Markdown.label()).clicked() {
                                    ui.close();
                                    app.export(Format::Markdown);
                                }
                                if ui.button(Format::Html.label()).clicked() {
                                    ui.close();
                                    app.export(Format::Html);
                                }
                            });
                            if ui
                                .button("Salvar")
                                .on_hover_text("Ctrl+S")
                                .clicked()
                            {
                                app.save(false);
                            }
                        });
                        if ui.button("Abrir").on_hover_text("Ctrl+O").clicked() {
                            app.open_dialog();
                        }

                        // Fica ao lado de Abrir porque é aqui que o usuário
                        // está depois de parar de gravar e revisar os passos.
                        if app.can_continue_recording()
                            && ui
                                .button("Continuar gravando")
                                .on_hover_text(
                                    "Acrescenta passos novos ao fim desta gravação",
                                )
                                .clicked()
                        {
                            app.continue_recording(&ctx);
                        }
                    });
                });
            });
        });
}

/// Diálogos modais. Desenhados por último, por cima de tudo.
pub fn dialogos(app: &mut App, ctx: &egui::Context) {
    let Some(dialogo) = app.dialogo else {
        return;
    };
    let palette = theme::palette(ctx);

    egui::Modal::new(egui::Id::new("dialogo")).show(ctx, |ui| {
        ui.set_width(420.0);

        match dialogo {
            Dialogo::ConfirmarSaida => {
                ui.label(RichText::new("Sair sem salvar?").size(17.0).strong());
                ui.add_space(6.0);
                let passos = app.project.as_ref().map_or(0, |p| p.recording.steps.len());
                ui.label(
                    RichText::new(format!(
                        "Esta gravação tem {passos} passo(s) com alterações não salvas. \
                         Fechar agora perde o que não estiver no arquivo."
                    ))
                    .color(palette.muted),
                );

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("Salvar e sair").clicked() {
                        app.save(false);
                        // Se o usuário desistiu no seletor de arquivo, o
                        // projeto continua sujo e a janela não deve fechar.
                        if !app.tem_trabalho_nao_salvo() {
                            app.fechar_mesmo_assim(ctx);
                        } else {
                            app.dialogo = None;
                        }
                    }
                    if ui.button("Sair sem salvar").clicked() {
                        app.fechar_mesmo_assim(ctx);
                    }
                    if ui.button("Cancelar").clicked() {
                        app.dialogo = None;
                    }
                });
            }

            Dialogo::RecuperarRascunho => {
                ui.label(
                    RichText::new("Encontramos uma gravação não salva")
                        .size(17.0)
                        .strong(),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "O StepEasy guarda um rascunho enquanto você grava e edita. \
                         O da sessão anterior não chegou a ser salvo em arquivo.",
                    )
                    .color(palette.muted),
                );

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("Recuperar").clicked() {
                        app.recuperar_rascunho();
                    }
                    if ui.button("Descartar").clicked() {
                        app.descartar_rascunho();
                    }
                    if ui.button("Decidir depois").clicked() {
                        app.dialogo = None;
                    }
                });
            }
        }
    });
}

pub fn status_bar(app: &mut App, ui: &mut egui::Ui) {
    let palette = theme::palette(ui.ctx());

    egui::Panel::bottom("status")
        .exact_size(30.0)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let (texto, cor) = match app.toasts.active() {
                    Some(toast) => (
                        toast.text.clone(),
                        match toast.level {
                            Level::Error => palette.accent,
                            Level::Info => palette.muted,
                        },
                    ),
                    None => (String::new(), palette.muted),
                };

                if !texto.is_empty() {
                    ui.label(RichText::new(texto).color(cor).size(12.0));
                    if icons::small_button(ui, Icon::Close)
                        .on_hover_text("Dispensar")
                        .clicked()
                    {
                        app.toasts.dismiss();
                    }
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(project) = &app.project {
                        let arquivo = project
                            .path()
                            .and_then(|p| p.file_name())
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "não salvo".to_string());
                        let marca = if project.is_dirty() { "● " } else { "" };
                        ui.label(
                            RichText::new(format!(
                                "{marca}{arquivo} · {} passo(s)",
                                project.recording.steps.len()
                            ))
                            .color(palette.muted)
                            .size(12.0),
                        );
                    }
                });
            });
        });
}
