//! Captura de entrada, de tela e de metadados de acessibilidade.
//!
//! Tudo aqui passa por três traits, implementadas por plataforma:
//!
//! - [`InputWatcher`] — escuta cliques e teclas globalmente;
//! - [`ScreenGrabber`] — tira a captura conforme o [`CaptureScope`];
//! - [`UiProbe`] — descobre qual controle está sob o cursor.
//!
//! No Windows as três têm implementação real. Nos outros sistemas existe um
//! stub que compila e captura tela, mas ainda não escuta entrada — é o que
//! mantém o workspace verde em Linux/macOS enquanto o suporte não chega.

pub mod event;
pub mod screens;
pub mod session;

#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod stub;

use crossbeam_channel::Sender;
use stepeasy_core::scope::{CaptureScope, MonitorInfo};
use stepeasy_core::UiTarget;

pub use event::{Frame, RawEvent};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("não foi possível instalar o gancho de entrada: {0}")]
    Hook(String),

    #[error("falha ao capturar a tela: {0}")]
    Screen(String),

    #[error("nenhum monitor encontrado")]
    NoMonitors,

    #[error("a janela em foco não pôde ser lida")]
    NoActiveWindow,

    #[error("captura de entrada ainda não implementada nesta plataforma")]
    Unsupported,

    #[error(transparent)]
    Core(#[from] stepeasy_core::Error),
}

/// Escuta global de mouse e teclado.
pub trait InputWatcher: Send {
    /// Instala os ganchos e passa a enviar eventos por `tx`.
    fn start(&mut self, tx: Sender<RawEvent>) -> Result<()>;
    /// Remove os ganchos. Idempotente.
    fn stop(&mut self);
}

/// Captura de tela conforme o escopo escolhido.
pub trait ScreenGrabber: Send {
    /// Monitores conectados, na ordem em que devem aparecer na UI.
    fn monitors(&self) -> Result<Vec<MonitorInfo>>;
    /// Captura a região correspondente a `scope`, com o cursor em `at`.
    fn grab(&self, scope: &CaptureScope, at: stepeasy_core::Point) -> Result<Frame>;
}

/// Consulta de acessibilidade.
pub trait UiProbe: Send {
    fn element_at(&self, point: stepeasy_core::Point) -> Option<UiTarget>;
}

/// Conjunto de implementações da plataforma atual.
pub struct Platform {
    pub input: Box<dyn InputWatcher>,
    pub screen: Box<dyn ScreenGrabber>,
    pub probe: Box<dyn UiProbe>,
}

/// Monta as implementações do sistema em que o programa está rodando.
pub fn platform() -> Platform {
    #[cfg(windows)]
    {
        windows::platform()
    }
    #[cfg(not(windows))]
    {
        stub::platform()
    }
}

/// `true` quando esta plataforma consegue gravar de verdade.
pub const fn is_supported() -> bool {
    cfg!(windows)
}
