//! Ícones desenhados à mão.
//!
//! As fontes que o egui embute não cobrem setas como `↶`, `↷` ou `⤒`: elas
//! saem como retângulo vazio. Em vez de embutir uma fonte de ícones inteira só
//! por causa de quatro botões, cada ícone aqui é desenhado com o `Painter` —
//! acompanha a cor do tema, escala com o zoom e não tem glifo para faltar.

use egui::{Color32, Pos2, Response, Sense, Shape, Stroke, Ui, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Seta circular anti-horária.
    Undo,
    /// Seta circular horária.
    Redo,
    ChevronUp,
    ChevronDown,
    Plus,
    Close,
    /// Sol — tema claro.
    Sun,
    /// Lua — tema escuro.
    Moon,
}

/// Botão quadrado com um ícone desenhado, no tamanho padrão da interface.
pub fn button(ui: &mut Ui, icon: Icon) -> Response {
    sized_button(ui, icon, 30.0)
}

/// Botão pequeno, para a barra de status.
pub fn small_button(ui: &mut Ui, icon: Icon) -> Response {
    sized_button(ui, icon, 20.0)
}

fn sized_button(ui: &mut Ui, icon: Icon, lado: f32) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(lado), Sense::click());

    let visuals = ui.style().interact(&response);
    let painter = ui.painter();
    painter.rect(
        rect,
        visuals.corner_radius,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );

    draw(painter, icon, rect.center(), lado * 0.30, visuals.fg_stroke.color);
    response
}

/// Desenha o ícone centrado em `center`, dentro de um quadrado de lado `2*raio`.
pub fn draw(painter: &egui::Painter, icon: Icon, center: Pos2, raio: f32, cor: Color32) {
    let stroke = Stroke::new((raio * 0.24).max(1.3), cor);

    match icon {
        Icon::Undo => seta_circular(painter, center, raio, stroke, true),
        Icon::Redo => seta_circular(painter, center, raio, stroke, false),
        Icon::ChevronUp => chevron(painter, center, raio, stroke, true),
        Icon::ChevronDown => chevron(painter, center, raio, stroke, false),
        Icon::Plus => cruz(painter, center, raio, stroke, false),
        Icon::Close => cruz(painter, center, raio, stroke, true),
        Icon::Sun => sol(painter, center, raio, stroke),
        Icon::Moon => lua(painter, center, raio, cor),
    }
}

/// Ponto do círculo de raio `r` em torno de `c`, no ângulo `graus`.
///
/// O eixo Y da tela cresce para baixo, então 90° é embaixo e 270° em cima.
fn no_circulo(c: Pos2, r: f32, graus: f32) -> Pos2 {
    let rad = graus.to_radians();
    Pos2::new(c.x + r * rad.cos(), c.y + r * rad.sin())
}

/// Seta circular: arco de `inicio` a `fim` com ponta na extremidade final.
fn seta_circular(
    painter: &egui::Painter,
    center: Pos2,
    raio: f32,
    stroke: Stroke,
    espelhado: bool,
) {
    const INICIO: f32 = 55.0;
    const FIM: f32 = 340.0;
    const PASSOS: usize = 24;

    let espelhar = |p: Pos2| {
        if espelhado {
            Pos2::new(2.0 * center.x - p.x, p.y)
        } else {
            p
        }
    };

    let mut pontos = Vec::with_capacity(PASSOS + 1);
    for i in 0..=PASSOS {
        let t = i as f32 / PASSOS as f32;
        pontos.push(espelhar(no_circulo(
            center,
            raio,
            INICIO + (FIM - INICIO) * t,
        )));
    }
    let ponta = *pontos.last().expect("o arco sempre tem pontos");
    let anterior = pontos[pontos.len() - 2];
    painter.add(Shape::line(pontos, stroke));

    // Ponta da seta: duas hastes abrindo a partir da direção do arco.
    let direcao = (ponta - anterior).normalized();
    let tamanho = raio * 0.62;
    for angulo in [150.0_f32, -150.0_f32] {
        let rad = angulo.to_radians();
        let haste = Vec2::new(
            direcao.x * rad.cos() - direcao.y * rad.sin(),
            direcao.x * rad.sin() + direcao.y * rad.cos(),
        ) * tamanho;
        painter.line_segment([ponta, ponta + haste], stroke);
    }
}

fn chevron(painter: &egui::Painter, center: Pos2, raio: f32, stroke: Stroke, para_cima: bool) {
    let sinal = if para_cima { 1.0 } else { -1.0 };
    let meia_largura = raio * 0.72;
    let meia_altura = raio * 0.42;

    // Haste vertical, para o botão não virar só um "v" solto.
    painter.line_segment(
        [
            Pos2::new(center.x, center.y - raio * 0.75 * sinal),
            Pos2::new(center.x, center.y + raio * 0.75 * sinal),
        ],
        stroke,
    );
    painter.add(Shape::line(
        vec![
            Pos2::new(center.x - meia_largura, center.y - meia_altura * sinal),
            Pos2::new(center.x, center.y - raio * 0.85 * sinal),
            Pos2::new(center.x + meia_largura, center.y - meia_altura * sinal),
        ],
        stroke,
    ));
}

fn cruz(painter: &egui::Painter, center: Pos2, raio: f32, stroke: Stroke, girada: bool) {
    let d = raio * 0.78;
    if girada {
        painter.line_segment(
            [
                Pos2::new(center.x - d, center.y - d),
                Pos2::new(center.x + d, center.y + d),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(center.x + d, center.y - d),
                Pos2::new(center.x - d, center.y + d),
            ],
            stroke,
        );
    } else {
        painter.line_segment(
            [
                Pos2::new(center.x - d, center.y),
                Pos2::new(center.x + d, center.y),
            ],
            stroke,
        );
        painter.line_segment(
            [
                Pos2::new(center.x, center.y - d),
                Pos2::new(center.x, center.y + d),
            ],
            stroke,
        );
    }
}

fn sol(painter: &egui::Painter, center: Pos2, raio: f32, stroke: Stroke) {
    painter.circle_stroke(center, raio * 0.44, stroke);
    for i in 0..8 {
        let graus = i as f32 * 45.0;
        painter.line_segment(
            [
                no_circulo(center, raio * 0.68, graus),
                no_circulo(center, raio * 0.98, graus),
            ],
            stroke,
        );
    }
}

fn lua(painter: &egui::Painter, center: Pos2, raio: f32, cor: Color32) {
    // Crescente desenhado como um arco **espesso**: com traço fino sai um "C"
    // de letra, não uma lua. Recortar um círculo com outro daria a ponta fina
    // de verdade, mas exigiria saber a cor do fundo — que muda conforme o
    // botão está sob o cursor ou pressionado.
    let grosso = Stroke::new(raio * 0.52, cor);
    let raio_arco = raio * 0.62;

    let mut pontos = Vec::with_capacity(25);
    for i in 0..=24 {
        let graus = 55.0 + 250.0 * (i as f32 / 24.0);
        pontos.push(no_circulo(center, raio_arco, graus));
    }
    painter.add(Shape::line(pontos, grosso));

    // Arredonda as duas pontas, que num traço reto ficariam cortadas em bisel.
    for graus in [55.0, 305.0] {
        painter.circle_filled(no_circulo(center, raio_arco, graus), grosso.width / 2.0, cor);
    }
}
