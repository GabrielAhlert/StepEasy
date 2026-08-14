//! Composição do que fica **gravado nos pixels** da captura: o marcador de
//! clique e as anotações.
//!
//! O editor desenha tudo isso por cima da textura, sem tocar no PNG — é o que
//! deixa apagar uma seta sem perder qualidade. Já no export a imagem sai
//! sozinha do aplicativo, então aqui as marcações viram pixels de verdade.

use std::sync::OnceLock;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{ImageFormat, Rgba, RgbaImage};

use crate::error::Result;
use crate::geometry::{Point, Rect};
use crate::model::Annotation;

/// Raio externo do anel do clique, em pixels.
const RADIUS: f32 = 24.0;
/// Espessura do traço colorido do anel.
const STROKE: f32 = 5.0;
/// Contorno branco de cada lado, para o anel aparecer sobre fundo claro e escuro.
const OUTLINE: f32 = 1.5;

const ACCENT: [u8; 3] = [0xE0, 0x2B, 0x20];

/// Cor padrão das anotações novas (a mesma do marcador de clique).
pub const DEFAULT_COLOR: [u8; 4] = [0xE0, 0x2B, 0x20, 0xFF];

fn fonte() -> &'static FontRef<'static> {
    static FONTE: OnceLock<FontRef<'static>> = OnceLock::new();
    FONTE.get_or_init(|| {
        FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT)
            .expect("a fonte embutida no epaint é válida")
    })
}

/// Decodifica o PNG, aplica anotações e marcador de clique, devolve PNG novo.
///
/// As anotações são desenhadas na ordem da lista (então uma seta por cima de
/// um borrão continua visível) e o marcador de clique vem por último, para
/// nunca ficar escondido.
pub fn compose(
    png: &[u8],
    cursor: Option<(u32, u32)>,
    annotations: &[Annotation],
) -> Result<Vec<u8>> {
    let mut img = image::load_from_memory(png)?.to_rgba8();

    for annotation in annotations {
        draw_annotation(&mut img, annotation);
    }
    if let Some((x, y)) = cursor {
        draw_marker(&mut img, x as f32, y as f32);
    }
    encode_png(&img)
}

/// Atalho de [`compose`] sem anotações.
pub fn with_click_marker(png: &[u8], x: u32, y: u32) -> Result<Vec<u8>> {
    compose(png, Some((x, y)), &[])
}

/// Recorta a região e devolve **só ela**, já borrada.
///
/// É o que o editor usa para mostrar o borrão de verdade na tela em vez de uma
/// tarja: desfocar a captura inteira a cada quadro seria caro, mas desfocar um
/// pedaço uma vez por alteração é barato.
///
/// O desfoque só enxerga os pixels do próprio recorte, igual ao de [`compose`],
/// então a prévia e o export dão o mesmo resultado.
pub fn blurred_region(png: &[u8], rect: Rect, radius: f32) -> Result<RgbaImage> {
    let img = image::load_from_memory(png)?.to_rgba8();
    let limite = Rect::new(0, 0, img.width(), img.height());
    let Some(area) = rect.intersect(&limite) else {
        return Ok(RgbaImage::new(1, 1));
    };

    let mut recorte = image::imageops::crop_imm(
        &img,
        area.x as u32,
        area.y as u32,
        area.width,
        area.height,
    )
    .to_image();

    let inteiro = Rect::new(0, 0, recorte.width(), recorte.height());
    blur_region(&mut recorte, inteiro, radius);
    Ok(recorte)
}

/// Miniatura com lado maior igual a `max_side`, em JPEG.
pub fn thumbnail(png: &[u8], max_side: u32, quality: u8) -> Result<Vec<u8>> {
    let img = image::load_from_memory(png)?;
    let thumb = img.thumbnail(max_side, max_side).to_rgb8();

    let mut out = Vec::new();
    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    encoder.encode(
        thumb.as_raw(),
        thumb.width(),
        thumb.height(),
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

pub fn encode_png(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

// ------------------------------------------------------------- anotações

fn draw_annotation(img: &mut RgbaImage, annotation: &Annotation) {
    match annotation {
        Annotation::Rect {
            rect,
            color,
            thickness,
        } => stroke_rect(img, *rect, *thickness, *color),

        Annotation::Arrow {
            from,
            to,
            color,
            thickness,
        } => arrow(img, *from, *to, *thickness, *color),

        Annotation::Blur { rect, radius } => blur_region(img, *rect, *radius),

        Annotation::Text {
            at,
            text,
            color,
            size,
        } => text_with_halo(img, *at, text, *size, *color),
    }
}

/// Contorno retangular, desenhado como quatro traços.
fn stroke_rect(img: &mut RgbaImage, rect: Rect, thickness: f32, color: [u8; 4]) {
    if rect.is_empty() {
        return;
    }
    let (l, t) = (rect.x as f32, rect.y as f32);
    let (r, b) = (rect.right() as f32, rect.bottom() as f32);
    let cantos = [(l, t), (r, t), (r, b), (l, b)];
    for i in 0..4 {
        stroke_line(img, cantos[i], cantos[(i + 1) % 4], thickness, color);
    }
}

/// Seta: haste de `from` a `to` mais duas hastes na ponta.
fn arrow(img: &mut RgbaImage, from: Point, to: Point, thickness: f32, color: [u8; 4]) {
    let a = (from.x as f32, from.y as f32);
    let b = (to.x as f32, to.y as f32);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let comprimento = (dx * dx + dy * dy).sqrt();
    if comprimento < 1.0 {
        return;
    }

    stroke_line(img, a, b, thickness, color);

    // A ponta cresce com a espessura, mas nunca passa de um terço da haste —
    // senão uma seta curta vira um triângulo só.
    let tamanho = (thickness * 4.5).min(comprimento / 3.0).max(thickness * 2.0);
    let (ux, uy) = (dx / comprimento, dy / comprimento);
    for angulo in [150.0_f32, -150.0_f32] {
        let rad = angulo.to_radians();
        let (hx, hy) = (
            ux * rad.cos() - uy * rad.sin(),
            ux * rad.sin() + uy * rad.cos(),
        );
        stroke_line(
            img,
            b,
            (b.0 + hx * tamanho, b.1 + hy * tamanho),
            thickness,
            color,
        );
    }
}

/// Traço com extremidades arredondadas, suavizado pela distância ao segmento.
fn stroke_line(img: &mut RgbaImage, a: (f32, f32), b: (f32, f32), thickness: f32, color: [u8; 4]) {
    let raio = (thickness / 2.0).max(0.5);
    let (w, h) = (img.width() as i32, img.height() as i32);

    let x0 = ((a.0.min(b.0) - raio - 1.0).floor() as i32).max(0);
    let x1 = ((a.0.max(b.0) + raio + 1.0).ceil() as i32).min(w - 1);
    let y0 = ((a.1.min(b.1) - raio - 1.0).floor() as i32).max(0);
    let y1 = ((a.1.max(b.1) + raio + 1.0).ceil() as i32).min(h - 1);

    for py in y0..=y1 {
        for px in x0..=x1 {
            let d = dist_to_segment((px as f32 + 0.5, py as f32 + 0.5), a, b);
            let cobertura = (raio - d + 0.5).clamp(0.0, 1.0);
            if cobertura > 0.0 {
                blend_rgba(img.get_pixel_mut(px as u32, py as u32), color, cobertura);
            }
        }
    }
}

fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let (wx, wy) = (p.0 - a.0, p.1 - a.1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 <= f32::EPSILON {
        0.0
    } else {
        ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.0 + vx * t, a.1 + vy * t);
    ((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt()
}

/// Borra a região com um desfoque de caixa separável.
///
/// Não é gaussiano, mas para esconder um CPF ou um nome numa captura o que
/// importa é que o texto fique irrecuperável — e duas passagens de caixa já
/// destroem a forma das letras.
fn blur_region(img: &mut RgbaImage, rect: Rect, radius: f32) {
    let limite = Rect::new(0, 0, img.width(), img.height());
    let Some(area) = rect.intersect(&limite) else {
        return;
    };
    let raio = (radius.round() as i32).clamp(1, 64);

    // Passagem horizontal e depois vertical, ambas lendo de uma cópia da área
    // para o desfoque não se alimentar do próprio resultado.
    for eixo in 0..2 {
        let origem: Vec<[u8; 4]> = (0..area.height)
            .flat_map(|y| {
                (0..area.width).map(move |x| (x, y))
            })
            .map(|(x, y)| img.get_pixel(area.x as u32 + x, area.y as u32 + y).0)
            .collect();

        for y in 0..area.height as i32 {
            for x in 0..area.width as i32 {
                let mut soma = [0u32; 4];
                let mut n = 0u32;
                for passo in -raio..=raio {
                    let (sx, sy) = if eixo == 0 {
                        (x + passo, y)
                    } else {
                        (x, y + passo)
                    };
                    if sx < 0 || sy < 0 || sx >= area.width as i32 || sy >= area.height as i32 {
                        continue;
                    }
                    let px = origem[(sy as u32 * area.width + sx as u32) as usize];
                    for c in 0..4 {
                        soma[c] += px[c] as u32;
                    }
                    n += 1;
                }
                if n == 0 {
                    continue;
                }
                let media = Rgba([
                    (soma[0] / n) as u8,
                    (soma[1] / n) as u8,
                    (soma[2] / n) as u8,
                    255,
                ]);
                img.put_pixel(area.x as u32 + x as u32, area.y as u32 + y as u32, media);
            }
        }
    }
}

/// Texto com contorno de contraste, para sobreviver a qualquer fundo.
fn text_with_halo(img: &mut RgbaImage, at: Point, texto: &str, size: f32, color: [u8; 4]) {
    if texto.trim().is_empty() {
        return;
    }
    let halo = if luminancia(color) > 0.55 {
        [0, 0, 0, color[3]]
    } else {
        [255, 255, 255, color[3]]
    };

    let d = (size * 0.07).clamp(1.0, 3.0);
    for (dx, dy) in [
        (-d, 0.0),
        (d, 0.0),
        (0.0, -d),
        (0.0, d),
        (-d, -d),
        (d, -d),
        (-d, d),
        (d, d),
    ] {
        draw_text(img, (at.x as f32 + dx, at.y as f32 + dy), texto, size, halo);
    }
    draw_text(img, (at.x as f32, at.y as f32), texto, size, color);
}

fn draw_text(img: &mut RgbaImage, at: (f32, f32), texto: &str, size: f32, color: [u8; 4]) {
    let font = fonte();
    let scaled = font.as_scaled(PxScale::from(size));
    // `at` é o canto superior esquerdo; a fonte posiciona pela linha de base.
    let mut caret = at.0;
    let base = at.1 + scaled.ascent();
    let mut anterior: Option<ab_glyph::GlyphId> = None;

    for c in texto.chars() {
        if c == '\n' {
            continue;
        }
        let id = font.glyph_id(c);
        if let Some(prev) = anterior {
            caret += scaled.kern(prev, id);
        }
        let glyph = id.with_scale_and_position(size, ab_glyph::point(caret, base));
        if let Some(desenhado) = font.outline_glyph(glyph) {
            let caixa = desenhado.px_bounds();
            desenhado.draw(|gx, gy, cobertura| {
                let px = caixa.min.x + gx as f32;
                let py = caixa.min.y + gy as f32;
                if px < 0.0 || py < 0.0 || px >= img.width() as f32 || py >= img.height() as f32 {
                    return;
                }
                blend_rgba(
                    img.get_pixel_mut(px as u32, py as u32),
                    color,
                    cobertura.clamp(0.0, 1.0),
                );
            });
        }
        caret += scaled.h_advance(id);
        anterior = Some(id);
    }
}

fn luminancia(color: [u8; 4]) -> f32 {
    (0.2126 * color[0] as f32 + 0.7152 * color[1] as f32 + 0.0722 * color[2] as f32) / 255.0
}

// --------------------------------------------------------- marcador de clique

/// Anel com contorno branco dos dois lados, com bordas suavizadas pela
/// distância ao centro (evita o serrilhado de um círculo pintado no braço).
fn draw_marker(img: &mut RgbaImage, cx: f32, cy: f32) {
    let outer = RADIUS + OUTLINE;
    let (w, h) = (img.width() as i32, img.height() as i32);

    let x0 = ((cx - outer).floor() as i32).max(0);
    let x1 = ((cx + outer).ceil() as i32).min(w - 1);
    let y0 = ((cy - outer).floor() as i32).max(0);
    let y1 = ((cy + outer).ceil() as i32).min(h - 1);

    let inner = RADIUS - STROKE;

    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            // Cobertura do traço colorido e do contorno branco, ambas suavizadas
            // numa faixa de 1 px.
            let accent = band(dist, inner, RADIUS);
            let white = band(dist, inner - OUTLINE, inner).max(band(dist, RADIUS, outer));

            if accent <= 0.0 && white <= 0.0 {
                continue;
            }
            let (color, alpha) = if accent >= white {
                (ACCENT, accent)
            } else {
                ([0xFF, 0xFF, 0xFF], white)
            };
            blend(img.get_pixel_mut(px as u32, py as u32), color, alpha);
        }
    }
}

/// Cobertura de um pixel a `dist` do centro dentro do anel `[from, to]`.
fn band(dist: f32, from: f32, to: f32) -> f32 {
    let entra = ((dist - from) + 0.5).clamp(0.0, 1.0);
    let sai = ((to - dist) + 0.5).clamp(0.0, 1.0);
    (entra * sai).clamp(0.0, 1.0)
}

fn blend(pixel: &mut Rgba<u8>, color: [u8; 3], alpha: f32) {
    for i in 0..3 {
        let base = pixel[i] as f32;
        pixel[i] = (base + (color[i] as f32 - base) * alpha).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = 255;
}

fn blend_rgba(pixel: &mut Rgba<u8>, color: [u8; 4], cobertura: f32) {
    let alpha = cobertura * (color[3] as f32 / 255.0);
    if alpha <= 0.0 {
        return;
    }
    blend(pixel, [color[0], color[1], color[2]], alpha);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn imagem_lisa(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([20, 20, 20, 255]));
        encode_png(&img).unwrap()
    }

    fn decodifica(png: &[u8]) -> RgbaImage {
        image::load_from_memory(png).unwrap().to_rgba8()
    }

    #[test]
    fn marcador_pinta_o_anel_e_preserva_o_centro() {
        let png = with_click_marker(&imagem_lisa(120, 120), 60, 60).unwrap();
        let img = decodifica(&png);

        assert_eq!(img.get_pixel(60, 60)[0], 20);
        let no_traco = img.get_pixel(60 + RADIUS as u32 - 2, 60);
        assert!(no_traco[0] > 120, "esperava vermelho, veio {no_traco:?}");
        assert_eq!(img.get_pixel(5, 5)[0], 20);
    }

    #[test]
    fn marcador_na_borda_nao_estoura() {
        let png = with_click_marker(&imagem_lisa(60, 60), 0, 59).unwrap();
        let img = image::load_from_memory(&png).unwrap();
        assert_eq!((img.width(), img.height()), (60, 60));
    }

    #[test]
    fn thumbnail_respeita_o_lado_maior() {
        let jpg = thumbnail(&imagem_lisa(1920, 1080), 320, 80).unwrap();
        let img = image::load_from_memory(&jpg).unwrap();
        assert_eq!(img.width(), 320);
        assert!(img.height() <= 320);
    }

    #[test]
    fn retangulo_pinta_a_borda_e_nao_o_miolo() {
        let png = compose(
            &imagem_lisa(100, 100),
            None,
            &[Annotation::Rect {
                rect: Rect::new(20, 20, 60, 60),
                color: [255, 0, 0, 255],
                thickness: 3.0,
            }],
        )
        .unwrap();
        let img = decodifica(&png);

        assert!(img.get_pixel(50, 20)[0] > 150, "borda de cima sem cor");
        assert!(img.get_pixel(20, 50)[0] > 150, "borda da esquerda sem cor");
        assert_eq!(img.get_pixel(50, 50)[0], 20, "o miolo deveria ficar intacto");
    }

    #[test]
    fn seta_pinta_haste_e_ponta() {
        let png = compose(
            &imagem_lisa(100, 100),
            None,
            &[Annotation::Arrow {
                from: Point::new(10, 50),
                to: Point::new(90, 50),
                color: [0, 255, 0, 255],
                thickness: 4.0,
            }],
        )
        .unwrap();
        let img = decodifica(&png);

        assert!(img.get_pixel(50, 50)[1] > 150, "haste sem cor");
        // A ponta abre para trás e para os lados a partir de (90, 50).
        assert!(
            img.get_pixel(78, 43)[1] > 80 || img.get_pixel(78, 57)[1] > 80,
            "não achei as hastes da ponta"
        );
    }

    #[test]
    fn borrao_destroi_o_contraste_da_regiao() {
        // Tabuleiro de xadrez: depois de borrado, tudo tende ao cinza médio.
        let mut base = RgbaImage::new(60, 60);
        for (x, y, px) in base.enumerate_pixels_mut() {
            let claro = (x / 2 + y / 2) % 2 == 0;
            *px = if claro {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        let png = encode_png(&base).unwrap();

        let borrado = compose(
            &png,
            None,
            &[Annotation::Blur {
                rect: Rect::new(10, 10, 40, 40),
                radius: 6.0,
            }],
        )
        .unwrap();
        let img = decodifica(&borrado);

        let dentro = img.get_pixel(30, 30)[0] as i32;
        assert!(
            (dentro - 128).abs() < 40,
            "esperava cinza no miolo borrado, veio {dentro}"
        );
        // Fora da região o tabuleiro continua com contraste total.
        let fora: Vec<i32> = (0..8).map(|i| img.get_pixel(i, 2)[0] as i32).collect();
        assert!(
            fora.iter().any(|v| *v > 200) && fora.iter().any(|v| *v < 55),
            "a área de fora foi borrada junto: {fora:?}"
        );
    }

    #[test]
    fn texto_desenha_pixels_e_respeita_a_area() {
        let png = compose(
            &imagem_lisa(200, 60),
            None,
            &[Annotation::Text {
                at: Point::new(10, 10),
                text: "Atenção".into(),
                color: [255, 255, 255, 255],
                size: 28.0,
            }],
        )
        .unwrap();
        let img = decodifica(&png);

        let pintados = img
            .enumerate_pixels()
            .filter(|(_, _, p)| p[0] > 200)
            .count();
        assert!(pintados > 40, "texto não desenhou nada ({pintados} pixels)");

        // Nada foi pintado bem longe do ponto de origem.
        assert_eq!(img.get_pixel(195, 55)[0], 20);
    }

    #[test]
    fn regiao_borrada_tem_o_tamanho_do_recorte_e_perde_o_contraste() {
        let mut base = RgbaImage::new(80, 80);
        for (x, y, px) in base.enumerate_pixels_mut() {
            let claro = (x / 2 + y / 2) % 2 == 0;
            *px = if claro {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            };
        }
        let png = encode_png(&base).unwrap();

        let recorte = blurred_region(&png, Rect::new(20, 20, 40, 40), 6.0).unwrap();
        assert_eq!((recorte.width(), recorte.height()), (40, 40));

        let meio = recorte.get_pixel(20, 20)[0] as i32;
        assert!((meio - 128).abs() < 40, "esperava cinza, veio {meio}");
    }

    #[test]
    fn regiao_borrada_fora_da_imagem_nao_quebra() {
        let recorte = blurred_region(&imagem_lisa(40, 40), Rect::new(500, 500, 20, 20), 4.0)
            .unwrap();
        assert_eq!((recorte.width(), recorte.height()), (1, 1));
    }

    #[test]
    fn regiao_borrada_e_recortada_ao_que_cabe_na_imagem() {
        let recorte =
            blurred_region(&imagem_lisa(40, 40), Rect::new(30, 30, 40, 40), 4.0).unwrap();
        assert_eq!((recorte.width(), recorte.height()), (10, 10));
    }

    #[test]
    fn texto_vazio_nao_quebra() {
        let png = compose(
            &imagem_lisa(50, 50),
            None,
            &[Annotation::Text {
                at: Point::new(5, 5),
                text: "   ".into(),
                color: [255, 255, 255, 255],
                size: 20.0,
            }],
        )
        .unwrap();
        assert_eq!(decodifica(&png).get_pixel(25, 25)[0], 20);
    }

    #[test]
    fn anotacao_fora_da_imagem_e_recortada() {
        let png = compose(
            &imagem_lisa(40, 40),
            None,
            &[
                Annotation::Rect {
                    rect: Rect::new(-50, -50, 30, 30),
                    color: [255, 0, 0, 255],
                    thickness: 2.0,
                },
                Annotation::Blur {
                    rect: Rect::new(100, 100, 30, 30),
                    radius: 4.0,
                },
                Annotation::Arrow {
                    from: Point::new(-20, -20),
                    to: Point::new(200, 200),
                    color: [0, 0, 255, 255],
                    thickness: 2.0,
                },
            ],
        )
        .unwrap();
        let img = decodifica(&png);
        assert_eq!((img.width(), img.height()), (40, 40));
    }

    #[test]
    fn marcador_fica_por_cima_das_anotacoes() {
        let png = compose(
            &imagem_lisa(120, 120),
            Some((60, 60)),
            &[Annotation::Blur {
                rect: Rect::new(0, 0, 120, 120),
                radius: 8.0,
            }],
        )
        .unwrap();
        let img = decodifica(&png);
        let no_traco = img.get_pixel(60 + RADIUS as u32 - 2, 60);
        assert!(no_traco[0] > 120, "o borrão comeu o marcador: {no_traco:?}");
    }
}
