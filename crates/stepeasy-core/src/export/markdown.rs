//! Export para Markdown.
//!
//! Gera `<nome>.md` e uma pasta `<nome>_imagens/` ao lado, com as capturas já
//! contendo o marcador de clique.

use std::path::Path;

use crate::error::Result;
use crate::model::Recording;
use crate::render;

use super::{step_text, ImageResolver};

/// Escreve o `.md` e as imagens. Devolve o caminho do arquivo gerado.
pub fn export(
    recording: &Recording,
    images: &mut dyn ImageResolver,
    out_path: impl AsRef<Path>,
) -> Result<()> {
    let out_path = out_path.as_ref();
    let stem = out_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| recording.slug());
    let dir_name = format!("{stem}_imagens");
    let img_dir = out_path.with_file_name(&dir_name);

    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", recording.title));
    if super::has_description(recording) {
        md.push_str(&format!("{}\n\n", recording.description.trim()));
    }
    md.push_str(&format!(
        "_{} passo(s) · gravado em {}_\n\n",
        recording.steps.len(),
        recording.created_at.format("%d/%m/%Y %H:%M")
    ));

    let mut escreveu_imagem = false;
    for step in &recording.steps {
        md.push_str(&format!("## {}. {}\n\n", step.index, step_text(step)));

        if !step.notes.trim().is_empty() {
            md.push_str(&format!("> {}\n\n", step.notes.trim().replace('\n', "\n> ")));
        }

        let Some(image) = &step.image else { continue };
        let Some(bytes) = images.resolve(&image.path) else {
            tracing::warn!("imagem ausente no export: {}", image.path);
            continue;
        };

        let bytes = render::compose(&bytes, step.cursor_in_image(), &step.annotations)?;

        if !escreveu_imagem {
            std::fs::create_dir_all(&img_dir)?;
            escreveu_imagem = true;
        }
        let file_name = format!("step-{:04}.png", step.index);
        std::fs::write(img_dir.join(&file_name), &bytes)?;
        md.push_str(&format!(
            "![Passo {}]({}/{})\n\n",
            step.index, dir_name, file_name
        ));
    }

    std::fs::write(out_path, md)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::bundle::image_path;
    use crate::geometry::{Point, Rect};
    use crate::model::{ImageRef, MouseButton, Step, StepKind};
    use crate::render::encode_png;
    use crate::scope::CaptureScope;

    #[test]
    fn gera_md_com_imagens_numeradas_pela_ordem_atual() {
        let dir = std::env::temp_dir().join(format!("stepeasy-md-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut rec = Recording::new("Emitir nota", CaptureScope::AllMonitors);
        rec.description = "Fluxo completo".into();

        let mut com_imagem = Step::new(StepKind::Click {
            button: MouseButton::Left,
        });
        com_imagem.caption = "Clicou em **Emitir**".into();
        com_imagem.cursor = Some(Point::new(30, 30));
        com_imagem.image = Some(ImageRef {
            path: image_path(1),
            thumb_path: None,
            width: 100,
            height: 100,
            source_rect: Rect::new(0, 0, 100, 100),
        });
        rec.steps.push(com_imagem);
        rec.steps.push(Step::manual("Confira o total"));
        rec.reindex();

        let png = encode_png(&image::RgbaImage::new(100, 100)).unwrap();
        let mut images: HashMap<String, Vec<u8>> = HashMap::new();
        images.insert(image_path(1), png);

        let out = dir.join("nota.md");
        export(&rec, &mut images, &out).unwrap();

        let md = std::fs::read_to_string(&out).unwrap();
        assert!(md.starts_with("# Emitir nota"));
        assert!(md.contains("Fluxo completo"));
        assert!(md.contains("## 1. Clicou em **Emitir**"));
        assert!(md.contains("![Passo 1](nota_imagens/step-0001.png)"));
        assert!(md.contains("## 2. Confira o total"));
        assert!(dir.join("nota_imagens/step-0001.png").exists());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn sem_imagens_nao_cria_pasta() {
        let dir = std::env::temp_dir().join(format!("stepeasy-md-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut rec = Recording::new("Só texto", CaptureScope::default());
        rec.steps.push(Step::manual("Faça algo"));
        rec.reindex();

        let out = dir.join("t.md");
        export(&rec, &mut HashMap::new(), &out).unwrap();
        assert!(!dir.join("t_imagens").exists());

        std::fs::remove_dir_all(&dir).ok();
    }
}
