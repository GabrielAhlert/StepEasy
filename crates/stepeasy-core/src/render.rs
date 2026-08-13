//! Composição do marcador de clique sobre a imagem do passo.
//!
//! O editor desenha o marcador por cima da textura, sem tocar no PNG. Já no
//! export a imagem sai sozinha do aplicativo, então o marcador precisa estar
//! gravado nos pixels — é isto que este módulo faz.

use image::{ImageFormat, Rgba, RgbaImage};

use crate::error::Result;

/// Raio externo do anel, em pixels.
const RADIUS: f32 = 24.0;
/// Espessura do traço colorido.
const STROKE: f32 = 5.0;
/// Contorno branco de cada lado, para o anel aparecer sobre fundo claro e escuro.
const OUTLINE: f32 = 1.5;

const ACCENT: [u8; 3] = [0xE0, 0x2B, 0x20];

/// Decodifica o PNG, desenha o anel centrado em `(x, y)` e devolve um PNG novo.
pub fn with_click_marker(png: &[u8], x: u32, y: u32) -> Result<Vec<u8>> {
    let mut img = image::load_from_memory(png)?.to_rgba8();
    draw_marker(&mut img, x as f32, y as f32);
    encode_png(&img)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn imagem_lisa(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([20, 20, 20, 255]));
        encode_png(&img).unwrap()
    }

    #[test]
    fn marcador_pinta_o_anel_e_preserva_o_centro() {
        let png = with_click_marker(&imagem_lisa(120, 120), 60, 60).unwrap();
        let img = image::load_from_memory(&png).unwrap().to_rgba8();

        // Centro do anel continua com o fundo original.
        assert_eq!(img.get_pixel(60, 60)[0], 20);
        // Sobre o traço, a cor de destaque domina.
        let no_traco = img.get_pixel(60 + RADIUS as u32 - 2, 60);
        assert!(no_traco[0] > 120, "esperava vermelho, veio {no_traco:?}");
        // Longe do marcador nada mudou.
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
}
