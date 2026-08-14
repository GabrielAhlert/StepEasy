//! Export para um HTML único e autocontido.
//!
//! As imagens vão embutidas em `data:` URI, então o arquivo pode ser enviado
//! por e-mail ou aberto em outra máquina sem levar pasta nenhuma junto. O CSS
//! acompanha o tema claro/escuro do sistema de quem abrir.

use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

use crate::error::Result;
use crate::model::Recording;
use crate::render;

use super::{step_text, ImageResolver};

const CSS: &str = r#"
:root{--bg:#f7f7f8;--card:#fff;--fg:#1b1b1f;--muted:#6b6b76;--line:#e3e3e8;--accent:#e02b20;--shadow:0 1px 2px rgba(0,0,0,.06),0 8px 24px rgba(0,0,0,.06)}
@media (prefers-color-scheme:dark){:root{--bg:#131316;--card:#1c1c21;--fg:#ececf1;--muted:#9a9aa6;--line:#2c2c34;--accent:#ff6b5e;--shadow:0 1px 2px rgba(0,0,0,.4),0 8px 24px rgba(0,0,0,.35)}}
*{box-sizing:border-box}
body{margin:0;padding:40px 20px;background:var(--bg);color:var(--fg);font:16px/1.6 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,Helvetica,Arial,sans-serif}
main{max-width:900px;margin:0 auto}
h1{font-size:2rem;line-height:1.25;margin:0 0 8px}
.meta{color:var(--muted);font-size:.875rem;margin-bottom:8px}
.desc{color:var(--muted);margin:0 0 32px}
.step{background:var(--card);border:1px solid var(--line);border-radius:14px;padding:20px 22px;margin-bottom:20px;box-shadow:var(--shadow)}
.head{display:flex;gap:14px;align-items:flex-start}
.num{flex:none;width:30px;height:30px;border-radius:50%;background:var(--accent);color:#fff;font-size:.875rem;font-weight:600;display:flex;align-items:center;justify-content:center}
.text{margin:3px 0 0;white-space:pre-wrap}
.notes{margin:12px 0 0 44px;padding-left:12px;border-left:3px solid var(--line);color:var(--muted);white-space:pre-wrap}
img{display:block;width:100%;height:auto;margin-top:16px;border:1px solid var(--line);border-radius:10px}
footer{color:var(--muted);font-size:.8125rem;text-align:center;margin-top:40px}
"#;

/// Escreve o HTML autocontido em `out_path`.
pub fn export(
    recording: &Recording,
    images: &mut dyn ImageResolver,
    out_path: impl AsRef<Path>,
) -> Result<()> {
    std::fs::write(out_path.as_ref(), render_string(recording, images)?)?;
    Ok(())
}

/// Mesma saída de [`export`], mas como string — usado pelos testes e por uma
/// eventual pré-visualização na UI.
pub fn render_string(recording: &Recording, images: &mut dyn ImageResolver) -> Result<String> {
    let mut html = String::with_capacity(64 * 1024);
    html.push_str("<!doctype html>\n<html lang=\"pt-BR\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\n");
    html.push_str(&format!("<title>{}</title>\n", escape(&recording.title)));
    html.push_str(&format!("<style>{CSS}</style>\n</head>\n<body>\n<main>\n"));

    html.push_str(&format!("<h1>{}</h1>\n", escape(&recording.title)));
    html.push_str(&format!(
        "<p class=\"meta\">{} passo(s) · gravado em {}</p>\n",
        recording.steps.len(),
        recording.created_at.format("%d/%m/%Y %H:%M")
    ));
    if super::has_description(recording) {
        html.push_str(&format!(
            "<p class=\"desc\">{}</p>\n",
            inline(recording.description.trim())
        ));
    }

    for step in &recording.steps {
        html.push_str("<section class=\"step\">\n<div class=\"head\">");
        html.push_str(&format!("<div class=\"num\">{}</div>", step.index));
        html.push_str(&format!(
            "<p class=\"text\">{}</p>",
            inline(&step_text(step))
        ));
        html.push_str("</div>\n");

        if !step.notes.trim().is_empty() {
            html.push_str(&format!(
                "<div class=\"notes\">{}</div>\n",
                inline(step.notes.trim())
            ));
        }

        if let Some(image) = &step.image {
            if let Some(bytes) = images.resolve(&image.path) {
                let bytes = render::compose(&bytes, step.cursor_in_image(), &step.annotations)?;
                html.push_str(&format!(
                    "<img alt=\"Passo {}\" src=\"data:image/png;base64,{}\">\n",
                    step.index,
                    STANDARD.encode(&bytes)
                ));
            } else {
                tracing::warn!("imagem ausente no export: {}", image.path);
            }
        }
        html.push_str("</section>\n");
    }

    html.push_str("<footer>Gerado com StepEasy</footer>\n</main>\n</body>\n</html>\n");
    Ok(html)
}

/// Escapa o mínimo necessário para texto dentro de elemento e de atributo.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapa e converte o `**negrito**` que as legendas geradas usam.
///
/// Não é um parser de Markdown: só pares de `**` na mesma linha viram
/// `<strong>`; um `**` solto fica literal.
fn inline(s: &str) -> String {
    let escaped = escape(s);
    let mut out = String::with_capacity(escaped.len());
    let mut rest = escaped.as_str();
    loop {
        let Some(start) = rest.find("**") else {
            out.push_str(rest);
            return out;
        };
        let after = &rest[start + 2..];
        let Some(end) = after.find("**") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..start]);
        out.push_str("<strong>");
        out.push_str(&after[..end]);
        out.push_str("</strong>");
        rest = &after[end + 2..];
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::bundle::image_path;
    use crate::geometry::Rect;
    use crate::model::{ImageRef, Step};
    use crate::render::encode_png;
    use crate::scope::CaptureScope;

    #[test]
    fn embute_imagem_em_base64() {
        let mut rec = Recording::new("Tutorial", CaptureScope::default());
        let mut step = Step::manual("Clique em **Salvar**");
        step.image = Some(ImageRef {
            path: image_path(1),
            thumb_path: None,
            width: 10,
            height: 10,
            source_rect: Rect::new(0, 0, 10, 10),
        });
        rec.steps.push(step);
        rec.reindex();

        let mut images: HashMap<String, Vec<u8>> = HashMap::new();
        images.insert(
            image_path(1),
            encode_png(&image::RgbaImage::new(10, 10)).unwrap(),
        );

        let html = render_string(&rec, &mut images).unwrap();
        assert!(html.contains("src=\"data:image/png;base64,"));
        assert!(html.contains("Clique em <strong>Salvar</strong>"));
        assert!(html.contains("prefers-color-scheme:dark"));
    }

    #[test]
    fn escapa_html_do_usuario() {
        let mut rec = Recording::new("<script>alert(1)</script>", CaptureScope::default());
        rec.steps.push(Step::manual("a & b > c"));
        rec.reindex();

        let html = render_string(&rec, &mut HashMap::new()).unwrap();
        assert!(!html.contains("<script>alert"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("a &amp; b &gt; c"));
    }

    #[test]
    fn asterisco_solto_fica_literal() {
        assert_eq!(inline("2 ** 3 é oito"), "2 ** 3 é oito");
        assert_eq!(inline("no **botão**"), "no <strong>botão</strong>");
    }
}
