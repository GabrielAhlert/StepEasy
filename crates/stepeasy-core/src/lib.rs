//! Núcleo do StepEasy: modelo de dados, formato `.stepeasy`, edição e export.
//!
//! Esta crate não conhece nem plataforma nem interface — é o que permite
//! testar toda a lógica de edição e de arquivo sem abrir uma janela.

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
