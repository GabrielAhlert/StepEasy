//! Rasteriza `assets/logo/stepeasy_icon.svg` nos ícones que o aplicativo usa.
//!
//! Roda à mão quando o logo mudar; a saída fica versionada para que compilar o
//! StepEasy não dependa de um rasterizador de SVG.
//!
//! `cargo run -p stepeasy --example gerar_icones`

use std::path::{Path, PathBuf};

use resvg::tiny_skia;
use resvg::usvg;

/// Tamanhos que entram no `.ico` do executável. O Windows escolhe conforme o
/// contexto: 16 px na barra de título, 256 px na visualização grande.
const TAMANHOS_ICO: [u32; 6] = [16, 24, 32, 48, 64, 256];
/// Tamanho do PNG usado como ícone da janela pelo `eframe`.
const TAMANHO_JANELA: u32 = 256;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raiz = raiz_do_projeto();
    let svg = raiz.join("assets/logo/stepeasy_icon.svg");
    let destino = raiz.join("assets/icons");
    std::fs::create_dir_all(&destino)?;

    let dados = std::fs::read(&svg)?;
    let arvore = usvg::Tree::from_data(&dados, &usvg::Options::default())?;

    let caixa = caixa_pintada(&arvore);

    // PNG da janela.
    let janela = rasterizar(&arvore, TAMANHO_JANELA, caixa);
    let caminho_png = destino.join(format!("stepeasy-{TAMANHO_JANELA}.png"));
    janela.save(&caminho_png)?;
    println!("gerado: {}", caminho_png.display());

    // ICO com todos os tamanhos, para o executável e o Explorer.
    let quadros: Vec<image::DynamicImage> = TAMANHOS_ICO
        .iter()
        .map(|lado| image::DynamicImage::ImageRgba8(rasterizar(&arvore, *lado, caixa)))
        .collect();
    let caminho_ico = destino.join("stepeasy.ico");
    escrever_ico(&quadros, &caminho_ico)?;
    println!("gerado: {}", caminho_ico.display());

    Ok(())
}

/// Margem livre em volta do desenho, como fração do lado.
const MARGEM: f32 = 0.06;

/// Desenha a árvore SVG num quadrado de `lado` pixels.
///
/// O enquadramento é pelo **conteúdo**, não pela `viewBox`: o desenho não ocupa
/// a arte toda e usar a viewBox deixaria folga desigual, o que a 16 px vira um
/// ícone pequeno e torto no canto.
fn rasterizar(arvore: &usvg::Tree, lado: u32, caixa: usvg::Rect) -> image::RgbaImage {
    let util = lado as f32 * (1.0 - 2.0 * MARGEM);
    let escala = util / caixa.width().max(caixa.height());

    // Centraliza o que sobra no eixo mais curto.
    let dx = (lado as f32 - caixa.width() * escala) / 2.0 - caixa.x() * escala;
    let dy = (lado as f32 - caixa.height() * escala) / 2.0 - caixa.y() * escala;

    let mut pixmap = tiny_skia::Pixmap::new(lado, lado).expect("lado > 0");
    resvg::render(
        arvore,
        tiny_skia::Transform::from_row(escala, 0.0, 0.0, escala, dx, dy),
        &mut pixmap.as_mut(),
    );

    image::RgbaImage::from_raw(lado, lado, pixmap.take())
        .expect("o pixmap tem exatamente lado*lado pixels RGBA")
}

/// Área que o desenho de fato pinta, em unidades do SVG.
///
/// Vem dos pixels e não da árvore de propósito: o arquivo tem um
/// `<rect fill="transparent">` cobrindo a arte inteira, que entra na caixa
/// declarada e faria o enquadramento voltar a ser o da `viewBox`. Medir a
/// opacidade resolve isso e vale para qualquer SVG que venha depois.
fn caixa_pintada(arvore: &usvg::Tree) -> usvg::Rect {
    const AMOSTRA: u32 = 512;

    let tamanho = arvore.size();
    let escala = AMOSTRA as f32 / tamanho.width().max(tamanho.height());

    let mut pixmap = tiny_skia::Pixmap::new(AMOSTRA, AMOSTRA).expect("AMOSTRA > 0");
    resvg::render(
        arvore,
        tiny_skia::Transform::from_scale(escala, escala),
        &mut pixmap.as_mut(),
    );

    let (mut x0, mut y0, mut x1, mut y1) = (AMOSTRA, AMOSTRA, 0u32, 0u32);
    for (i, px) in pixmap.pixels().iter().enumerate() {
        if px.alpha() == 0 {
            continue;
        }
        let (x, y) = (i as u32 % AMOSTRA, i as u32 / AMOSTRA);
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x);
        y1 = y1.max(y);
    }

    if x1 < x0 || y1 < y0 {
        // SVG vazio: cai para a viewBox em vez de estourar.
        return usvg::Rect::from_xywh(0.0, 0.0, tamanho.width(), tamanho.height())
            .expect("tamanho positivo");
    }

    usvg::Rect::from_xywh(
        x0 as f32 / escala,
        y0 as f32 / escala,
        (x1 - x0 + 1) as f32 / escala,
        (y1 - y0 + 1) as f32 / escala,
    )
    .expect("largura e altura positivas")
}

/// Escreve um `.ico` com vários tamanhos.
///
/// Cada quadro vai como PNG dentro do container, que é o que o Windows aceita
/// desde o Vista e evita ter de montar as máscaras do formato BMP antigo.
fn escrever_ico(quadros: &[image::DynamicImage], destino: &Path) -> std::io::Result<()> {
    use std::io::Write;

    let mut png_por_quadro = Vec::new();
    for quadro in quadros {
        let mut bytes = std::io::Cursor::new(Vec::new());
        quadro
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("quadro RGBA válido");
        png_por_quadro.push(bytes.into_inner());
    }

    let mut arquivo = std::io::BufWriter::new(std::fs::File::create(destino)?);

    // Cabeçalho: reservado, tipo 1 (ícone), quantidade de imagens.
    arquivo.write_all(&0u16.to_le_bytes())?;
    arquivo.write_all(&1u16.to_le_bytes())?;
    arquivo.write_all(&(quadros.len() as u16).to_le_bytes())?;

    // O primeiro byte de dados vem depois do cabeçalho e de todas as entradas.
    let mut offset = 6 + 16 * quadros.len() as u32;
    for (quadro, png) in quadros.iter().zip(&png_por_quadro) {
        // 256 é gravado como 0 — o campo tem um byte só.
        let lado = |v: u32| if v >= 256 { 0u8 } else { v as u8 };
        arquivo.write_all(&[lado(quadro.width()), lado(quadro.height())])?;
        arquivo.write_all(&[0, 0])?; // paleta e reservado
        arquivo.write_all(&1u16.to_le_bytes())?; // planos
        arquivo.write_all(&32u16.to_le_bytes())?; // bits por pixel
        arquivo.write_all(&(png.len() as u32).to_le_bytes())?;
        arquivo.write_all(&offset.to_le_bytes())?;
        offset += png.len() as u32;
    }

    for png in &png_por_quadro {
        arquivo.write_all(png)?;
    }
    arquivo.flush()
}

/// Sobe do diretório da crate até a raiz do workspace.
fn raiz_do_projeto() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/stepeasy fica dois níveis abaixo da raiz")
        .to_path_buf()
}
