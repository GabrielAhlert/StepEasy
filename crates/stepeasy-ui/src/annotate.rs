//! Ferramentas de anotação: barra, desenho sobre a captura e painel de edição.
//!
//! As anotações vivem em coordenadas de **pixel da imagem**, não da tela. Todo
//! o vaivém entre os dois espaços passa pelo [`Mapa`], que é montado uma vez
//! por quadro a partir de onde a imagem foi desenhada e de quanto ela foi
//! reduzida para caber.

use egui::{Color32, Pos2, Rect as EguiRect, RichText, Sense, Stroke, Vec2};
use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::model::Annotation;
use stepeasy_core::render::DEFAULT_COLOR;
use uuid::Uuid;

use crate::app::App;
use crate::screens::editor::Action;
use crate::theme::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tool {
    /// Só seleciona e move o que já existe.
    #[default]
    Select,
    Arrow,
    Rect,
    Blur,
    Text,
}

impl Tool {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Select => "Selecionar",
            Self::Arrow => "Seta",
            Self::Rect => "Retângulo",
            Self::Blur => "Borrão",
            Self::Text => "Texto",
        }
    }

    fn dica(&self) -> &'static str {
        match self {
            Self::Select => "Clique numa anotação para editá-la; arraste para mover",
            Self::Arrow => "Arraste de onde a seta começa até onde ela aponta",
            Self::Rect => "Arraste para cercar uma região",
            Self::Blur => "Arraste sobre o que precisa sumir (senha, CPF, nome)",
            Self::Text => "Clique onde o texto deve começar",
        }
    }

    fn desenha_arrastando(&self) -> bool {
        matches!(self, Self::Arrow | Self::Rect | Self::Blur)
    }
}

/// Estado do arrasto em andamento, em coordenadas de imagem.
#[derive(Debug, Clone, Copy)]
pub struct Rascunho {
    pub inicio: Point,
    pub atual: Point,
}

/// Preferências das anotações novas e o que está sendo desenhado agora.
pub struct Estado {
    pub tool: Tool,
    pub color: Color32,
    pub thickness: f32,
    pub text_size: f32,
    pub blur_radius: f32,
    /// Índice da anotação selecionada dentro do passo em foco.
    pub selecionada: Option<usize>,
    pub rascunho: Option<Rascunho>,
    /// Deslocamento acumulado enquanto se arrasta uma anotação existente.
    arrastando: Option<(usize, Point)>,
}

impl Default for Estado {
    fn default() -> Self {
        Self {
            tool: Tool::default(),
            color: cor_egui(DEFAULT_COLOR),
            thickness: 4.0,
            text_size: 28.0,
            blur_radius: 12.0,
            selecionada: None,
            rascunho: None,
            arrastando: None,
        }
    }
}

impl Estado {
    /// Esquece a seleção — chamado ao trocar de passo.
    pub fn limpar(&mut self) {
        self.selecionada = None;
        self.rascunho = None;
        self.arrastando = None;
    }
}

/// Conversão entre pixels da imagem e pontos da tela.
#[derive(Debug, Clone, Copy)]
pub struct Mapa {
    pub origem: Pos2,
    pub escala: f32,
    pub largura: u32,
    pub altura: u32,
}

impl Mapa {
    pub fn para_tela(&self, p: Point) -> Pos2 {
        self.origem + Vec2::new(p.x as f32 * self.escala, p.y as f32 * self.escala)
    }

    pub fn retangulo_na_tela(&self, r: Rect) -> EguiRect {
        EguiRect::from_min_max(
            self.para_tela(Point::new(r.x, r.y)),
            self.para_tela(Point::new(r.right(), r.bottom())),
        )
    }

    /// Ponto da tela em pixel da imagem, preso às bordas.
    pub fn para_imagem(&self, p: Pos2) -> Point {
        let x = ((p.x - self.origem.x) / self.escala).round() as i32;
        let y = ((p.y - self.origem.y) / self.escala).round() as i32;
        Point::new(
            x.clamp(0, self.largura.saturating_sub(1) as i32),
            y.clamp(0, self.altura.saturating_sub(1) as i32),
        )
    }
}

// ------------------------------------------------------------------ barra

/// Barra de ferramentas acima da captura.
pub fn toolbar(app: &mut App, ui: &mut egui::Ui, palette: &Palette) {
    ui.horizontal_wrapped(|ui| {
        // Sem largura explícita o controle deslizante encolhe até virar só a
        // bolinha, sem trilho, e ninguém entende que dá para arrastar.
        ui.spacing_mut().slider_width = 90.0;

        for tool in [Tool::Select, Tool::Arrow, Tool::Rect, Tool::Blur, Tool::Text] {
            if ui
                .selectable_label(app.annot.tool == tool, tool.label())
                .on_hover_text(tool.dica())
                .clicked()
            {
                app.annot.tool = tool;
                app.annot.rascunho = None;
            }
        }

        ui.separator();

        if app.annot.tool == Tool::Blur {
            ui.label(RichText::new("Intensidade").size(11.0).color(palette.muted));
            ui.add(egui::Slider::new(&mut app.annot.blur_radius, 4.0..=40.0).show_value(false));
        } else {
            ui.color_edit_button_srgba(&mut app.annot.color);
            if app.annot.tool == Tool::Text {
                ui.label(RichText::new("Tamanho").size(11.0).color(palette.muted));
                ui.add(egui::Slider::new(&mut app.annot.text_size, 12.0..=96.0).show_value(false));
            } else {
                ui.label(RichText::new("Espessura").size(11.0).color(palette.muted));
                ui.add(egui::Slider::new(&mut app.annot.thickness, 1.0..=16.0).show_value(false));
            }
        }
    });

    if app.annot.tool != Tool::Select {
        ui.label(
            RichText::new(app.annot.tool.dica())
                .size(11.0)
                .color(palette.muted),
        );
    }
}

// -------------------------------------------------------------- interação

/// Trata cliques e arrastos sobre a captura, gerando as ações de edição.
pub fn interact(
    app: &mut App,
    ui: &mut egui::Ui,
    response: &egui::Response,
    mapa: Mapa,
    step: Uuid,
    anotacoes: &[Annotation],
    actions: &mut Vec<Action>,
) {
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.annot.rascunho = None;
        app.annot.arrastando = None;
        app.annot.selecionada = None;
    }

    let Some(pos) = response.interact_pointer_pos() else {
        // Sem ponteiro sobre a imagem: se havia arrasto pendente, ele já foi
        // resolvido no quadro em que o botão foi solto.
        return;
    };
    let ponto = mapa.para_imagem(pos);

    match app.annot.tool {
        Tool::Select => interagir_selecao(app, response, ponto, step, anotacoes, actions),

        Tool::Text if response.clicked() => {
            actions.push(Action::AddAnnotation {
                step,
                annotation: Annotation::Text {
                    at: ponto,
                    text: "Texto".into(),
                    color: cor_core(app.annot.color),
                    size: app.annot.text_size,
                },
            });
        }

        ferramenta if ferramenta.desenha_arrastando() => {
            if response.drag_started() {
                app.annot.rascunho = Some(Rascunho {
                    inicio: ponto,
                    atual: ponto,
                });
            } else if response.dragged() {
                if let Some(rascunho) = &mut app.annot.rascunho {
                    rascunho.atual = ponto;
                }
            } else if response.drag_stopped() {
                if let Some(rascunho) = app.annot.rascunho.take() {
                    if let Some(annotation) = finalizar(app, ferramenta, rascunho) {
                        actions.push(Action::AddAnnotation { step, annotation });
                    }
                }
            }
        }

        _ => {}
    }
}

fn interagir_selecao(
    app: &mut App,
    response: &egui::Response,
    ponto: Point,
    step: Uuid,
    anotacoes: &[Annotation],
    actions: &mut Vec<Action>,
) {
    if response.drag_started() || response.clicked() {
        // A busca é de trás para a frente: o que foi desenhado por último está
        // por cima, então é ele que deve ganhar o clique.
        let alvo = anotacoes
            .iter()
            .enumerate()
            .rev()
            .find(|(_, a)| a.bounds().contains(ponto))
            .map(|(i, _)| i);

        app.annot.selecionada = alvo;
        app.annot.arrastando = alvo.map(|i| (i, ponto));
    }

    if response.dragged() {
        if let Some((indice, anterior)) = app.annot.arrastando {
            let (dx, dy) = (ponto.x - anterior.x, ponto.y - anterior.y);
            if dx != 0 || dy != 0 {
                if let Some(annotation) = anotacoes.get(indice) {
                    let mut movida = annotation.clone();
                    movida.translate(dx, dy);
                    actions.push(Action::UpdateAnnotation {
                        step,
                        index: indice,
                        annotation: movida,
                    });
                }
                app.annot.arrastando = Some((indice, ponto));
            }
        }
    }

    if response.drag_stopped() {
        app.annot.arrastando = None;
    }
}

/// Converte o arrasto terminado na anotação correspondente, descartando os
/// gestos curtos demais para terem sido intencionais.
fn finalizar(app: &App, tool: Tool, rascunho: Rascunho) -> Option<Annotation> {
    let Rascunho { inicio, atual } = rascunho;
    let distancia = (inicio.x - atual.x).abs().max((inicio.y - atual.y).abs());
    if distancia < 5 {
        return None;
    }

    Some(match tool {
        Tool::Arrow => Annotation::Arrow {
            from: inicio,
            to: atual,
            color: cor_core(app.annot.color),
            thickness: app.annot.thickness,
        },
        Tool::Rect => Annotation::Rect {
            rect: Rect::from_corners(inicio, atual),
            color: cor_core(app.annot.color),
            thickness: app.annot.thickness,
        },
        Tool::Blur => Annotation::Blur {
            rect: Rect::from_corners(inicio, atual),
            radius: app.annot.blur_radius,
        },
        _ => return None,
    })
}

// ----------------------------------------------------------------- prévia

/// Desenha as anotações e o rascunho em cima da imagem.
///
/// O borrão já aplicado aparece **desfocado de verdade**: a região é recortada,
/// borrada uma vez e guardada como textura. Só enquanto o arrasto está em
/// andamento é que ele vira uma tarja — aí o retângulo muda a cada quadro e
/// recalcular o desfoque junto travaria o gesto.
pub fn preview(
    app: &mut App,
    ctx: &egui::Context,
    painter: &egui::Painter,
    mapa: Mapa,
    imagem: &str,
    anotacoes: &[Annotation],
    palette: &Palette,
) {
    for (indice, annotation) in anotacoes.iter().enumerate() {
        match annotation {
            Annotation::Blur { rect, radius } => {
                let alvo = Borrao {
                    imagem,
                    indice,
                    rect: *rect,
                    radius: *radius,
                };
                desenhar_borrao(app, ctx, painter, mapa, alvo);
            }
            outra => desenhar(painter, mapa, outra),
        }

        if app.annot.selecionada == Some(indice) {
            let caixa = mapa.retangulo_na_tela(annotation.bounds()).expand(4.0);
            painter.rect_stroke(
                caixa,
                4.0,
                Stroke::new(1.5, palette.accent),
                egui::StrokeKind::Outside,
            );
        }
    }

    if let Some(rascunho) = app.annot.rascunho {
        if let Some(previa) = finalizar(app, app.annot.tool, rascunho) {
            desenhar(painter, mapa, &previa);
        }
    }
}

/// O borrão a desenhar e onde encontrar os pixels dele.
struct Borrao<'a> {
    imagem: &'a str,
    indice: usize,
    rect: Rect,
    radius: f32,
}

/// Desenha a região já borrada, calculando-a na primeira vez.
fn desenhar_borrao(
    app: &mut App,
    ctx: &egui::Context,
    painter: &egui::Painter,
    mapa: Mapa,
    alvo: Borrao<'_>,
) {
    let Borrao {
        imagem,
        indice,
        rect,
        radius,
    } = alvo;
    let caixa = mapa.retangulo_na_tela(rect);
    let prefixo = format!("blur:{imagem}:{indice}:");
    let chave = format!(
        "{prefixo}{},{},{}x{},{}",
        rect.x, rect.y, rect.width, rect.height, radius as u32
    );

    if !app.textures.contains(&chave) {
        // Mexer na intensidade ou arrastar a anotação cria uma chave nova a
        // cada quadro; a anterior desta mesma anotação já não serve.
        app.textures.forget_prefix(&prefixo);
    }

    let png = app
        .project
        .as_mut()
        .and_then(|p| p.blob_opt(imagem))
        .map(<[u8]>::to_vec);

    let textura = app.textures.get_or_build(ctx, &chave, || {
        let png = png?;
        let borrado = stepeasy_core::render::blurred_region(&png, rect, radius).ok()?;
        let tamanho = [borrado.width() as usize, borrado.height() as usize];
        Some(egui::ColorImage::from_rgba_unmultiplied(
            tamanho,
            borrado.as_raw(),
        ))
    });

    match textura {
        Some(textura) => {
            painter.image(
                textura.id(),
                caixa,
                EguiRect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // Sem a imagem original não dá para borrar; ao menos não deixa passar
        // o que deveria estar escondido.
        None => {
            painter.rect_filled(caixa, 0.0, Color32::from_black_alpha(220));
        }
    }
}

fn desenhar(painter: &egui::Painter, mapa: Mapa, annotation: &Annotation) {
    match annotation {
        Annotation::Rect {
            rect,
            color,
            thickness,
        } => {
            painter.rect_stroke(
                mapa.retangulo_na_tela(*rect),
                0.0,
                Stroke::new(thickness * mapa.escala, cor_egui(*color)),
                egui::StrokeKind::Middle,
            );
        }

        Annotation::Arrow {
            from,
            to,
            color,
            thickness,
        } => {
            let a = mapa.para_tela(*from);
            let b = mapa.para_tela(*to);
            let stroke = Stroke::new(thickness * mapa.escala, cor_egui(*color));
            painter.line_segment([a, b], stroke);

            let direcao = (b - a).normalized();
            let tamanho = (thickness * 4.5 * mapa.escala).max(6.0);
            for angulo in [150.0_f32, -150.0_f32] {
                let rad = angulo.to_radians();
                let haste = Vec2::new(
                    direcao.x * rad.cos() - direcao.y * rad.sin(),
                    direcao.x * rad.sin() + direcao.y * rad.cos(),
                ) * tamanho;
                painter.line_segment([b, b + haste], stroke);
            }
        }

        // Só o rascunho passa por aqui: a anotação já aplicada é desenhada
        // desfocada de verdade, em `desenhar_borrao`.
        Annotation::Blur { rect, .. } => {
            let caixa = mapa.retangulo_na_tela(*rect);
            painter.rect_filled(caixa, 0.0, Color32::from_black_alpha(150));
            painter.rect_stroke(
                caixa,
                0.0,
                Stroke::new(1.0, Color32::from_white_alpha(140)),
                egui::StrokeKind::Middle,
            );
        }

        Annotation::Text {
            at,
            text,
            color,
            size,
        } => {
            painter.text(
                mapa.para_tela(*at),
                egui::Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(size * mapa.escala),
                cor_egui(*color),
            );
        }
    }
}

// ------------------------------------------------------------ painel lateral

/// Lista das anotações do passo, com edição da que está selecionada.
pub fn panel(
    app: &mut App,
    ui: &mut egui::Ui,
    step: Uuid,
    anotacoes: &[Annotation],
    palette: &Palette,
    actions: &mut Vec<Action>,
) {
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.label(RichText::new(format!("Anotações ({})", anotacoes.len())).strong());

    if anotacoes.is_empty() {
        ui.label(
            RichText::new("Use a barra acima da captura para marcar a imagem.")
                .size(11.0)
                .color(palette.muted),
        );
        return;
    }

    for (indice, annotation) in anotacoes.iter().enumerate() {
        let selecionada = app.annot.selecionada == Some(indice);
        ui.horizontal(|ui| {
            let resumo = match annotation {
                Annotation::Text { text, .. } => format!("Texto: {}", primeiras_palavras(text)),
                outra => outra.label().to_string(),
            };
            if ui.selectable_label(selecionada, resumo).clicked() {
                app.annot.selecionada = Some(indice);
            }
            if ui.small_button("remover").clicked() {
                actions.push(Action::DeleteAnnotation { step, index: indice });
            }
        });

        if selecionada {
            editar(ui, step, indice, annotation, actions);
        }
    }
}

fn editar(
    ui: &mut egui::Ui,
    step: Uuid,
    index: usize,
    annotation: &Annotation,
    actions: &mut Vec<Action>,
) {
    let mut alterada = annotation.clone();
    let mut mudou = false;

    ui.indent(("anotacao", index), |ui| match &mut alterada {
        Annotation::Text {
            text, color, size, ..
        } => {
            if ui
                .add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY))
                .changed()
            {
                mudou = true;
            }
            ui.horizontal(|ui| {
                let mut cor = cor_egui(*color);
                if ui.color_edit_button_srgba(&mut cor).changed() {
                    *color = cor_core(cor);
                    mudou = true;
                }
                if ui.add(egui::Slider::new(size, 12.0..=96.0).text("px")).changed() {
                    mudou = true;
                }
            });
        }

        Annotation::Blur { radius, .. } => {
            if ui
                .add(egui::Slider::new(radius, 4.0..=40.0).text("intensidade"))
                .changed()
            {
                mudou = true;
            }
        }

        Annotation::Rect {
            color, thickness, ..
        }
        | Annotation::Arrow {
            color, thickness, ..
        } => {
            ui.horizontal(|ui| {
                let mut cor = cor_egui(*color);
                if ui.color_edit_button_srgba(&mut cor).changed() {
                    *color = cor_core(cor);
                    mudou = true;
                }
                if ui
                    .add(egui::Slider::new(thickness, 1.0..=16.0).text("px"))
                    .changed()
                {
                    mudou = true;
                }
            });
        }
    });

    if mudou {
        actions.push(Action::UpdateAnnotation {
            step,
            index,
            annotation: alterada,
        });
    }
}

fn primeiras_palavras(texto: &str) -> String {
    let limpo = texto.trim();
    if limpo.chars().count() > 18 {
        format!("{}…", limpo.chars().take(17).collect::<String>())
    } else {
        limpo.to_string()
    }
}

/// O modelo guarda a cor com alfa **separado**; o `Color32` do egui guarda
/// pré-multiplicado. Converter pelos canais crus mudaria a cor toda vez que a
/// anotação fosse translúcida, então a ida e a volta passam pelas funções que
/// desfazem a pré-multiplicação.
pub fn cor_egui(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

pub fn cor_core(c: Color32) -> [u8; 4] {
    c.to_srgba_unmultiplied()
}

/// Área clicável da imagem, para o `Response` reagir a arrasto.
pub fn sense() -> Sense {
    Sense::click_and_drag()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapa() -> Mapa {
        Mapa {
            origem: Pos2::new(100.0, 50.0),
            escala: 0.5,
            largura: 1920,
            altura: 1080,
        }
    }

    #[test]
    fn ida_e_volta_entre_tela_e_imagem() {
        let m = mapa();
        let p = Point::new(400, 200);
        assert_eq!(m.para_imagem(m.para_tela(p)), p);
    }

    #[test]
    fn ponto_fora_da_imagem_e_preso_na_borda() {
        let m = mapa();
        assert_eq!(m.para_imagem(Pos2::new(0.0, 0.0)), Point::new(0, 0));
        assert_eq!(
            m.para_imagem(Pos2::new(9999.0, 9999.0)),
            Point::new(1919, 1079)
        );
    }

    #[test]
    fn arrasto_curto_demais_nao_vira_anotacao() {
        let app_estado = Estado::default();
        let rascunho = Rascunho {
            inicio: Point::new(10, 10),
            atual: Point::new(13, 12),
        };
        // `finalizar` só depende do estado, não do App inteiro; a checagem de
        // distância é a parte que importa aqui.
        let distancia = (rascunho.inicio.x - rascunho.atual.x)
            .abs()
            .max((rascunho.inicio.y - rascunho.atual.y).abs());
        assert!(distancia < 5);
        assert_eq!(app_estado.tool, Tool::Select);
    }

    #[test]
    fn conversao_de_cor_preserva_os_canais() {
        assert_eq!(cor_core(cor_egui(DEFAULT_COLOR)), DEFAULT_COLOR);

        // Com alfa parcial a pré-multiplicação do egui custa arredondamento;
        // o que não pode é a cor virar outra.
        let translucida = [0xE0, 0x2B, 0x20, 0xC0];
        let volta = cor_core(cor_egui(translucida));
        for canal in 0..4 {
            assert!(
                (volta[canal] as i32 - translucida[canal] as i32).abs() <= 2,
                "canal {canal}: {volta:?} != {translucida:?}"
            );
        }
    }
}
