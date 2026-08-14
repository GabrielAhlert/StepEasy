//! Gera um `.stepeasy` sintético, sem precisar gravar nada.
//!
//! Serve para abrir o editor e mexer nas ferramentas — inclusive em Linux e
//! macOS, onde a captura de entrada ainda não existe.
//!
//! `cargo run -p stepeasy-core --example gravacao_exemplo -- exemplo.stepeasy`

use image::{Rgba, RgbaImage};
use stepeasy_core::bundle::{image_path, thumb_path};
use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::model::{ImageRef, MouseButton, Recording, Step, StepKind, UiTarget};
use stepeasy_core::scope::CaptureScope;
use stepeasy_core::{caption, render, Project};

const LARGURA: u32 = 900;
const ALTURA: u32 = 560;

fn main() -> stepeasy_core::Result<()> {
    let saida = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "exemplo.stepeasy".to_string());

    let mut recording = Recording::new("Gravação de exemplo", CaptureScope::ActiveWindow);
    recording.description = "Passos falsos para experimentar o editor.".into();

    let passos = [
        (
            StepKind::Click {
                button: MouseButton::Left,
            },
            "Arquivo",
            "item de menu",
            (120, 60),
        ),
        (
            StepKind::Type {
                text: "relatorio.pdf".into(),
            },
            "Nome do arquivo:",
            "caixa de edição",
            (420, 300),
        ),
        (
            StepKind::Key {
                combo: "Ctrl+S".into(),
            },
            "Salvar",
            "botão",
            (700, 470),
        ),
        (
            StepKind::Scroll {
                direction: stepeasy_core::ScrollDirection::Down,
                amount: 360,
            },
            "Lista de documentos",
            "lista",
            (450, 250),
        ),
    ];

    let mut project = Project::new(recording);

    for (i, (kind, alvo, tipo, (cx, cy))) in passos.into_iter().enumerate() {
        let numero = i as u32 + 1;
        let png = render::encode_png(&tela_falsa(numero))?;
        let thumb = render::thumbnail(&png, 320, 78).ok();

        let mut step = Step::new(kind);
        step.cursor = Some(Point::new(cx, cy));
        step.target = Some(UiTarget {
            name: Some(alvo.into()),
            control_type: Some(tipo.into()),
            window_title: Some("Programa de Exemplo".into()),
            process_name: Some("exemplo.exe".into()),
            bounds: None,
        });
        step.image = Some(ImageRef {
            path: image_path(numero),
            thumb_path: thumb.is_some().then(|| thumb_path(numero)),
            width: LARGURA,
            height: ALTURA,
            source_rect: Rect::new(0, 0, LARGURA, ALTURA),
        });
        caption::refresh(&mut step);

        project.put_blob(image_path(numero), png);
        if let Some(thumb) = thumb {
            project.put_blob(thumb_path(numero), thumb);
        }
        project.recording.steps.push(step);
    }

    project.recording.reindex();
    project.save_as(&saida)?;
    println!("gerado: {saida}");
    Ok(())
}

/// Uma "janela" desenhada na mão: barra de título, painel e um bloco de texto.
fn tela_falsa(numero: u32) -> RgbaImage {
    let mut img = RgbaImage::from_pixel(LARGURA, ALTURA, Rgba([250, 250, 252, 255]));

    for (x, y, px) in img.enumerate_pixels_mut() {
        if y < 40 {
            *px = Rgba([228, 230, 238, 255]);
        } else if (40..44).contains(&y) {
            *px = Rgba([206, 209, 220, 255]);
        } else if x > 620 && y > 80 {
            *px = Rgba([240, 241, 246, 255]);
        }
    }

    let png = render::encode_png(&img).expect("imagem válida");
    let com_texto = render::compose(
        &png,
        None,
        &[
            texto(24, 8, "Programa de Exemplo", 22.0),
            texto(40, 90, &format!("Passo {numero}"), 34.0),
            texto(40, 150, "CPF 000.000.000-00", 24.0),
            texto(40, 190, "Cliente: Fulano de Tal", 24.0),
        ],
    )
    .expect("texto rasterizado");

    image::load_from_memory(&com_texto)
        .expect("png recém-gerado")
        .to_rgba8()
}

fn texto(x: i32, y: i32, s: &str, tamanho: f32) -> stepeasy_core::Annotation {
    stepeasy_core::Annotation::Text {
        at: Point::new(x, y),
        text: s.into(),
        color: [0x22, 0x22, 0x2A, 0xFF],
        size: tamanho,
    }
}
