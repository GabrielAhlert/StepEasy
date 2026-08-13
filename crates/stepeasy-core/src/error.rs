//! Erros do núcleo, com mensagens já em português para irem direto à UI.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("o arquivo não é um pacote StepEasy válido: manifest.json não encontrado")]
    MissingManifest,

    #[error("arquivo ausente no pacote: {0}")]
    MissingEntry(String),

    #[error(
        "este pacote foi salvo por uma versão mais nova do StepEasy \
         (formato {found}, esta versão lê até {supported}). Atualize o aplicativo."
    )]
    FutureFormat { found: u32, supported: u32 },

    #[error("erro de leitura/escrita: {0}")]
    Io(#[from] std::io::Error),

    #[error("pacote corrompido: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("manifesto inválido: {0}")]
    Json(#[from] serde_json::Error),

    #[error("erro ao processar imagem: {0}")]
    Image(#[from] image::ImageError),

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
