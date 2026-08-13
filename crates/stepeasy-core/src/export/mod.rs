//! Exportadores. Cada um recebe a gravação e um resolvedor de imagens, para
//! não depender de como o projeto guarda os bytes.

pub mod html;
pub mod markdown;

use crate::model::Recording;

/// Fonte dos bytes de uma imagem do pacote, pelo caminho interno.
///
/// Implementado pelo `Project`, mas os testes usam um `HashMap` direto.
pub trait ImageResolver {
    fn resolve(&mut self, path: &str) -> Option<Vec<u8>>;
}

impl ImageResolver for std::collections::HashMap<String, Vec<u8>> {
    fn resolve(&mut self, path: &str) -> Option<Vec<u8>> {
        self.get(path).cloned()
    }
}

/// Formatos disponíveis no menu de exportação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Html,
}

impl Format {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Markdown => "Markdown (.md)",
            Self::Html => "HTML autocontido (.html)",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Html => "html",
        }
    }
}

/// Texto de um passo pronto para export: usa a legenda ou, se vazia, gera uma.
pub(crate) fn step_text(step: &crate::model::Step) -> String {
    if step.caption.trim().is_empty() {
        crate::caption::generate(step)
    } else {
        step.caption.clone()
    }
}

/// Cabeçalho comum: título e descrição.
pub(crate) fn has_description(recording: &Recording) -> bool {
    !recording.description.trim().is_empty()
}
