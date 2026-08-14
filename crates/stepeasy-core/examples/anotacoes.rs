//! Gera uma imagem com os quatro tipos de anotação, para conferir a olho como
//! elas saem no export.
//!
//! `cargo run -p stepeasy-core --example anotacoes -- saida.png`

use image::{Rgba, RgbaImage};
use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::model::Annotation;
use stepeasy_core::render;

fn main() -> stepeasy_core::Result<()> {
    let saida = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "anotacoes.png".to_string());

    // Um "screenshot" sintético: faixas claras e escuras mais um bloco de
    // texto, para dar para julgar contraste e o efeito do borrão.
    let mut base = RgbaImage::from_pixel(720, 380, Rgba([246, 246, 248, 255]));
    for (x, y, px) in base.enumerate_pixels_mut() {
        if y > 300 || (x / 40 + y / 40) % 2 == 0 {
            *px = Rgba([214, 218, 226, 255]);
        }
    }
    let mut com_texto = base.clone();
    escrever(&mut com_texto, 40, 60, "CPF 000.000.000-00", 26.0);
    escrever(&mut com_texto, 40, 100, "Cliente: Fulano de Tal", 26.0);

    let anotacoes = vec![
        Annotation::Blur {
            rect: Rect::new(34, 52, 340, 84),
            radius: 10.0,
        },
        Annotation::Rect {
            rect: Rect::new(420, 180, 250, 120),
            color: [0xE0, 0x2B, 0x20, 0xFF],
            thickness: 4.0,
        },
        Annotation::Arrow {
            from: Point::new(200, 330),
            to: Point::new(430, 250),
            color: [0x1A, 0x7F, 0x37, 0xFF],
            thickness: 5.0,
        },
        Annotation::Text {
            at: Point::new(60, 200),
            text: "Clique aqui para confirmar".into(),
            color: [0x11, 0x11, 0x18, 0xFF],
            size: 30.0,
        },
    ];

    let png = render::compose(
        &render::encode_png(&com_texto)?,
        Some((545, 240)),
        &anotacoes,
    )?;
    std::fs::write(&saida, png)?;
    println!("gerado: {saida}");
    Ok(())
}

/// Escreve na imagem base usando o mesmo caminho do export, só para o exemplo
/// ter algo parecido com texto de tela para borrar.
fn escrever(img: &mut RgbaImage, x: i32, y: i32, texto: &str, tamanho: f32) {
    let png = render::encode_png(img).expect("imagem válida");
    let com_texto = render::compose(
        &png,
        None,
        &[Annotation::Text {
            at: Point::new(x, y),
            text: texto.into(),
            color: [0x22, 0x22, 0x2A, 0xFF],
            size: tamanho,
        }],
    )
    .expect("texto rasterizado");
    *img = image::load_from_memory(&com_texto)
        .expect("png recém-gerado")
        .to_rgba8();
}
