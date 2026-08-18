//! Núcleo do StepEasy: modelo de dados, formato `.stepeasy`, edição e export.
//!
//! Esta crate não conhece nem plataforma nem interface — é o que permite
//! testar toda a lógica de edição e de arquivo sem abrir uma janela.

// Os textos ficam em `locales/`, na raiz do workspace, compartilhados com a
// interface. Um arquivo por idioma: traduzir e copiar `en.yml` e trocar os
// valores, sem tocar em Rust. Veja TRANSLATING.md.
rust_i18n::i18n!("../../locales", fallback = "en");

pub mod bundle;
pub mod caption;
pub mod edit;
pub mod error;
pub mod export;
pub mod geometry;
pub mod model;
pub mod project;
pub mod render;
pub mod scope;

/// Utilidades de teste compartilhadas entre os módulos.
#[cfg(test)]
pub(crate) mod teste {
    /// Roda `f` com o idioma fixado.
    ///
    /// O idioma ativo do `rust-i18n` é um global do processo e o `cargo test`
    /// roda em paralelo: sem o mutex, um teste troca o idioma no meio da
    /// execução do outro e a falha aparece de forma intermitente, longe de
    /// quem a causou.
    pub fn com_idioma<R>(locale: &str, f: impl FnOnce() -> R) -> R {
        static TRAVA: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guarda = TRAVA.lock().unwrap_or_else(|e| e.into_inner());
        rust_i18n::set_locale(locale);
        f()
    }
}

pub use error::{Error, Result};
pub use geometry::{Point, Rect};
pub use model::{
    Annotation, ImageRef, MouseButton, Recording, ScrollDirection, Step, StepKind, UiTarget,
    FORMAT_VERSION,
};
pub use project::Project;
pub use scope::{CaptureScope, MonitorId, MonitorInfo};

/// Implementa o resolvedor de imagens do export em cima do projeto aberto.
impl export::ImageResolver for Project {
    fn resolve(&mut self, path: &str) -> Option<Vec<u8>> {
        self.blob_opt(path).map(<[u8]>::to_vec)
    }
}
