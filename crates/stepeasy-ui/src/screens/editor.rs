//! Editor: timeline à esquerda, captura no centro, detalhes à direita.
//!
//! O egui é imediato, então mexer na gravação no meio do desenho brigaria com
//! o empréstimo do projeto. O padrão aqui é: a interface só **enfileira**
//! [`Action`]s, e elas são aplicadas depois, num ponto só — que é também onde
//! o histórico de undo é alimentado.

use egui::{Align, Layout, RichText, Sense, Vec2};
use stepeasy_core::edit;
use stepeasy_core::model::StepKind;
use uuid::Uuid;

use crate::app::App;
use crate::icons::{self, Icon};
use crate::theme::{self, Palette};

/// Largura da miniatura na timeline.
const THUMB_W: f32 = 132.0;

enum Action {
    Select { id: Uuid, ctrl: bool, shift: bool },
    Move { from: usize, to: usize },
    MoveSelectedBy(i32),
    Delete,
    Duplicate(Uuid),
    Merge,
    InsertManual,
    SetCaption { id: Uuid, text: String },
    SetNotes { id: Uuid, text: String },
    ResetCaption(Uuid),
    SetTitle(String),
    SetDescription(String),
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();

    if app.project.is_none() {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("Nenhuma gravação aberta.");
            });
        });
        return;
    }

    let palette = theme::palette(&ctx);
    let mut actions: Vec<Action> = Vec::new();

    egui::Panel::left("timeline")
        .exact_size(THUMB_W + 44.0)
        .resizable(false)
        .show(ui, |ui| timeline(app, ui, &palette, &mut actions));

    egui::Panel::right("detalhes")
        .exact_size(330.0)
        .resizable(false)
        .show(ui, |ui| detalhes(app, ui, &palette, &mut actions));

    egui::CentralPanel::default().show(ui, |ui| captura(app, ui, &palette));

    atalhos(&ctx, &mut actions);
    for action in actions {
        aplicar(app, action);
    }
}

// ---------------------------------------------------------------- timeline

fn timeline(app: &mut App, ui: &mut egui::Ui, palette: &Palette, actions: &mut Vec<Action>) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(app.history.can_undo(), |ui| {
            let dica = app
                .history
                .undo_label()
                .map(|l| format!("Desfazer: {l} (Ctrl+Z)"))
                .unwrap_or_else(|| "Desfazer (Ctrl+Z)".into());
            if icons::button(ui, Icon::Undo).on_hover_text(dica).clicked() {
                app.undo();
            }
        });
        ui.add_enabled_ui(app.history.can_redo(), |ui| {
            let dica = app
                .history
                .redo_label()
                .map(|l| format!("Refazer: {l} (Ctrl+Shift+Z)"))
                .unwrap_or_else(|| "Refazer (Ctrl+Shift+Z)".into());
            if icons::button(ui, Icon::Redo).on_hover_text(dica).clicked() {
                app.redo();
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icons::button(ui, Icon::Plus)
                .on_hover_text("Inserir um passo escrito à mão")
                .clicked()
            {
                actions.push(Action::InsertManual);
            }
        });
    });
    ui.separator();

    // Instantâneo do que a lista precisa desenhar, para soltar o empréstimo
    // do projeto antes de chamar o `dnd`.
    let itens: Vec<(Uuid, u32, String, Option<String>)> = {
        let project = app.project.as_ref().unwrap();
        project
            .recording
            .steps
            .iter()
            .map(|s| {
                (
                    s.id,
                    s.index,
                    primeira_linha(&s.caption),
                    s.image.as_ref().and_then(|i| i.thumb_path.clone()),
                )
            })
            .collect()
    };

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let resposta = egui_dnd::dnd(ui, "timeline-dnd").show(
                itens.iter(),
                |ui, item, handle, _state| {
                    let (id, index, texto, thumb) = item;
                    let selecionado = app.selection.contains(id) || app.focused == Some(*id);

                    handle.ui(ui, |ui| {
                        let frame = egui::Frame::new()
                            .fill(if selecionado {
                                palette.selection
                            } else {
                                palette.card
                            })
                            .stroke(egui::Stroke::new(
                                1.0,
                                if selecionado {
                                    palette.accent
                                } else {
                                    palette.line
                                },
                            ))
                            .corner_radius(8)
                            .inner_margin(6);

                        let resposta = frame
                            .show(ui, |ui| {
                                ui.set_width(THUMB_W);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{index}"))
                                            .strong()
                                            .color(palette.accent),
                                    );
                                    ui.label(
                                        RichText::new(texto)
                                            .size(11.0)
                                            .color(palette.muted),
                                    );
                                });
                                if let Some(thumb) = thumb {
                                    miniatura(app, ui, thumb);
                                }
                            })
                            .response
                            .interact(Sense::click());

                        if resposta.clicked() {
                            let (ctrl, shift) =
                                ui.input(|i| (i.modifiers.command, i.modifiers.shift));
                            actions.push(Action::Select {
                                id: *id,
                                ctrl,
                                shift,
                            });
                        }
                    });
                },
            );

            if let Some(update) = resposta.final_update() {
                // O `egui_dnd` reporta o destino no espaço "antes da remoção";
                // ao mover para baixo isso é uma posição a mais.
                let to = if update.to > update.from {
                    update.to.saturating_sub(1)
                } else {
                    update.to
                };
                if to != update.from {
                    actions.push(Action::Move {
                        from: update.from,
                        to,
                    });
                }
            }
        });
}

fn miniatura(app: &mut App, ui: &mut egui::Ui, path: &str) {
    let textura = {
        let ctx = ui.ctx().clone();
        if app.textures.contains(path) {
            app.textures.get_or_load(&ctx, path, &[])
        } else {
            let bytes = app
                .project
                .as_mut()
                .and_then(|p| p.blob_opt(path))
                .map(<[u8]>::to_vec);
            bytes.and_then(|b| app.textures.get_or_load(&ctx, path, &b))
        }
    };

    if let Some(textura) = textura {
        let tamanho = textura.size_vec2();
        let escala = THUMB_W / tamanho.x.max(1.0);
        ui.add(egui::Image::new(&textura).fit_to_exact_size(Vec2::new(
            THUMB_W,
            (tamanho.y * escala).min(110.0),
        )));
    }
}

// ----------------------------------------------------------------- captura

fn captura(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    let Some(id) = app.focused else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("Selecione um passo na timeline.").color(palette.muted));
        });
        return;
    };

    let (image_path, cursor, legenda) = {
        let project = app.project.as_ref().unwrap();
        match project.recording.step_by_id(id) {
            Some(step) => (
                step.image.as_ref().map(|i| i.path.clone()),
                step.cursor_in_image(),
                step.caption.clone(),
            ),
            None => (None, None, String::new()),
        }
    };

    ui.add_space(6.0);
    ui.label(RichText::new(primeira_linha(&legenda)).size(15.0).strong());
    ui.add_space(6.0);

    let Some(path) = image_path else {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("Este passo não tem imagem.").color(palette.muted));
        });
        return;
    };

    let textura = {
        let ctx = ui.ctx().clone();
        if app.textures.contains(&path) {
            app.textures.get_or_load(&ctx, &path, &[])
        } else {
            let bytes = app
                .project
                .as_mut()
                .and_then(|p| p.blob_opt(&path))
                .map(<[u8]>::to_vec);
            bytes.and_then(|b| app.textures.get_or_load(&ctx, &path, &b))
        }
    };

    let Some(textura) = textura else {
        ui.label(RichText::new("A imagem deste passo não pôde ser lida.").color(palette.accent));
        return;
    };

    let tamanho = textura.size_vec2();
    let disponivel = ui.available_size();
    let escala = (disponivel.x / tamanho.x)
        .min(disponivel.y / tamanho.y)
        .clamp(0.05, 1.0);
    let mostrado = tamanho * escala;

    let resposta = ui.add(egui::Image::new(&textura).fit_to_exact_size(mostrado));

    // Marcador de clique desenhado por cima, sem tocar no PNG — o export é que
    // grava o anel nos pixels.
    if let Some((cx, cy)) = cursor {
        let centro = resposta.rect.min + Vec2::new(cx as f32 * escala, cy as f32 * escala);
        let raio = 24.0 * escala.max(0.35);
        let painter = ui.painter_at(resposta.rect);
        painter.circle_stroke(centro, raio, egui::Stroke::new(3.0, palette.accent));
        painter.circle_stroke(
            centro,
            raio + 2.0,
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
    }
}

// ---------------------------------------------------------------- detalhes

fn detalhes(app: &mut App, ui: &mut egui::Ui, palette: &Palette, actions: &mut Vec<Action>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(6.0);
            ui.label(RichText::new("Gravação").strong());

            let (mut titulo, mut descricao) = {
                let rec = &app.project.as_ref().unwrap().recording;
                (rec.title.clone(), rec.description.clone())
            };
            if ui
                .add(egui::TextEdit::singleline(&mut titulo).desired_width(f32::INFINITY))
                .changed()
            {
                actions.push(Action::SetTitle(titulo));
            }
            if ui
                .add(
                    egui::TextEdit::multiline(&mut descricao)
                        .desired_rows(2)
                        .hint_text("Descrição (opcional)")
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                actions.push(Action::SetDescription(descricao));
            }

            ui.add_space(10.0);
            ui.separator();

            let Some(id) = app.focused else {
                ui.label(RichText::new("Nenhum passo selecionado.").color(palette.muted));
                return;
            };
            let Some(step) = app
                .project
                .as_ref()
                .and_then(|p| p.recording.step_by_id(id))
                .cloned()
            else {
                return;
            };

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Passo {}", step.index)).strong());
                ui.label(
                    RichText::new(descricao_do_tipo(&step.kind))
                        .size(11.0)
                        .color(palette.muted),
                );
            });

            if step.scope_fallback {
                ui.label(
                    RichText::new(
                        "Este passo saiu do escopo escolhido — foi capturada a tela inteira.",
                    )
                    .size(11.0)
                    .color(palette.accent),
                );
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Texto do passo").size(12.0).color(palette.muted));
            let mut caption = step.caption.clone();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut caption)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                actions.push(Action::SetCaption { id, text: caption });
            }
            if step.caption_edited
                && !matches!(step.kind, StepKind::Manual)
                && ui.small_button("Voltar ao texto automático").clicked()
            {
                actions.push(Action::ResetCaption(id));
            }

            ui.add_space(8.0);
            ui.label(RichText::new("Observações").size(12.0).color(palette.muted));
            let mut notes = step.notes.clone();
            if ui
                .add(
                    egui::TextEdit::multiline(&mut notes)
                        .desired_rows(2)
                        .hint_text("Aparece como citação no export")
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                actions.push(Action::SetNotes { id, text: notes });
            }

            if let Some(target) = &step.target {
                ui.add_space(10.0);
                ui.label(RichText::new("Detectado").size(12.0).color(palette.muted));
                for (rotulo, valor) in [
                    ("Controle", target.name.clone()),
                    ("Tipo", target.control_type.clone()),
                    ("Janela", target.window_title.clone()),
                    ("Programa", target.process_name.clone()),
                ] {
                    if let Some(valor) = valor {
                        ui.label(
                            RichText::new(format!("{rotulo}: {valor}"))
                                .size(11.0)
                                .color(palette.muted),
                        );
                    }
                }
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);

            let selecionados = app.selection.len().max(1);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Excluir").clicked() {
                    actions.push(Action::Delete);
                }
                if ui.button("Duplicar").clicked() {
                    actions.push(Action::Duplicate(id));
                }
                ui.add_enabled_ui(selecionados > 1, |ui| {
                    if ui
                        .button(format!("Mesclar {selecionados}"))
                        .on_hover_text("Junta os passos selecionados em um só")
                        .clicked()
                    {
                        actions.push(Action::Merge);
                    }
                });
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if icons::button(ui, Icon::ChevronUp)
                    .on_hover_text("Mover o passo para cima")
                    .clicked()
                {
                    actions.push(Action::MoveSelectedBy(-1));
                }
                if icons::button(ui, Icon::ChevronDown)
                    .on_hover_text("Mover o passo para baixo")
                    .clicked()
                {
                    actions.push(Action::MoveSelectedBy(1));
                }
                ui.label(
                    RichText::new("ou arraste na timeline")
                        .size(11.0)
                        .color(palette.muted),
                );
            });
        });
}

// ----------------------------------------------------------------- ações

fn atalhos(ctx: &egui::Context, actions: &mut Vec<Action>) {
    ctx.input(|i| {
        // Só age quando nenhum campo de texto está com o foco, senão Delete
        // apagaria o passo em vez do caractere.
        if i.focused {
            return;
        }
        if i.key_pressed(egui::Key::Delete) {
            actions.push(Action::Delete);
        }
    });
}

fn aplicar(app: &mut App, action: Action) {
    match action {
        Action::Select { id, ctrl, shift } => selecionar(app, id, ctrl, shift),

        Action::Move { from, to } => {
            app.edit("Reordenar", |rec| edit::move_step(rec, from, to));
        }

        Action::MoveSelectedBy(delta) => {
            let Some(id) = app.focused else { return };
            let Some(pos) = app
                .project
                .as_ref()
                .and_then(|p| p.recording.position_of(id))
            else {
                return;
            };
            let total = app.project.as_ref().unwrap().recording.steps.len();
            let destino = pos as i32 + delta;
            if destino < 0 || destino >= total as i32 {
                return;
            }
            app.edit("Reordenar", |rec| {
                edit::move_step(rec, pos, destino as usize)
            });
        }

        Action::Delete => {
            let ids = alvos(app);
            if ids.is_empty() {
                return;
            }
            let rotulo = if ids.len() > 1 {
                "Excluir passos"
            } else {
                "Excluir passo"
            };
            app.edit(rotulo, |rec| edit::delete_steps(rec, &ids));
            app.selection.clear();
            app.focused = None;
            app.sanitize_selection();
        }

        Action::Duplicate(id) => {
            let novo = app.edit("Duplicar passo", |rec| edit::duplicate_step(rec, id));
            if let Some(Some(novo)) = novo {
                app.focused = Some(novo);
                app.selection = vec![novo];
            }
        }

        Action::Merge => {
            let ids = alvos(app);
            if ids.len() < 2 {
                return;
            }
            let resultado = app.edit("Mesclar passos", |rec| edit::merge_steps(rec, &ids));
            if let Some(Some(id)) = resultado {
                app.focused = Some(id);
                app.selection = vec![id];
            }
        }

        Action::InsertManual => {
            let pos = app
                .focused
                .and_then(|id| {
                    app.project
                        .as_ref()
                        .and_then(|p| p.recording.position_of(id))
                })
                .map(|p| p + 1)
                .unwrap_or(0);
            let novo = app.edit("Inserir passo", |rec| {
                edit::insert_manual(rec, pos, "Novo passo")
            });
            if let Some(id) = novo {
                app.focused = Some(id);
                app.selection = vec![id];
            }
        }

        Action::SetCaption { id, text } => {
            app.edit("Editar texto", |rec| edit::set_caption(rec, id, text));
        }

        Action::SetNotes { id, text } => {
            app.edit("Editar observações", |rec| {
                if let Some(step) = rec.step_by_id_mut(id) {
                    step.notes = text;
                }
            });
        }

        Action::ResetCaption(id) => {
            app.edit("Restaurar texto automático", |rec| {
                edit::reset_caption(rec, id)
            });
        }

        Action::SetTitle(text) => {
            app.edit("Editar título", |rec| rec.title = text);
        }

        Action::SetDescription(text) => {
            app.edit("Editar descrição", |rec| rec.description = text);
        }
    }
}

/// Passos que as ações em lote atingem: a seleção, ou o passo em foco.
fn alvos(app: &App) -> Vec<Uuid> {
    if app.selection.is_empty() {
        app.focused.into_iter().collect()
    } else {
        app.selection.clone()
    }
}

fn selecionar(app: &mut App, id: Uuid, ctrl: bool, shift: bool) {
    let Some(project) = &app.project else { return };

    if shift {
        if let (Some(ancora), Some(alvo)) = (
            app.focused.and_then(|f| project.recording.position_of(f)),
            project.recording.position_of(id),
        ) {
            let (a, b) = if ancora <= alvo {
                (ancora, alvo)
            } else {
                (alvo, ancora)
            };
            app.selection = project.recording.steps[a..=b].iter().map(|s| s.id).collect();
            app.focused = Some(id);
            return;
        }
    }

    if ctrl {
        if let Some(pos) = app.selection.iter().position(|s| *s == id) {
            app.selection.remove(pos);
        } else {
            app.selection.push(id);
        }
    } else {
        app.selection = vec![id];
    }
    app.focused = Some(id);
}

fn primeira_linha(texto: &str) -> String {
    let linha = texto.lines().next().unwrap_or("").trim();
    let limpo = linha.replace("**", "");
    if limpo.chars().count() > 60 {
        let corte: String = limpo.chars().take(57).collect();
        format!("{corte}…")
    } else if limpo.is_empty() {
        "(sem texto)".to_string()
    } else {
        limpo
    }
}

fn descricao_do_tipo(kind: &StepKind) -> &'static str {
    match kind {
        StepKind::Click { .. } => "clique",
        StepKind::DoubleClick { .. } => "duplo clique",
        StepKind::Drag { .. } => "arrasto",
        StepKind::Type { .. } => "digitação",
        StepKind::Key { .. } => "atalho",
        StepKind::Scroll { .. } => "rolagem",
        StepKind::Manual => "escrito à mão",
        StepKind::Merged { .. } => "agrupado",
    }
}
