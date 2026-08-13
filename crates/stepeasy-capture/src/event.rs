//! Eventos crus vindos dos ganchos de entrada e o resultado de uma captura.

use chrono::{DateTime, Utc};
use image::RgbaImage;
use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::model::MouseButton;

/// Evento de entrada, ainda sem screenshot nem acessibilidade.
///
/// O callback do gancho só constrói e enfileira isto: qualquer trabalho pesado
/// dentro do gancho trava a entrada do sistema inteiro.
#[derive(Debug, Clone)]
pub enum RawEvent {
    MouseDown {
        button: MouseButton,
        at: Point,
        time: DateTime<Utc>,
    },
    MouseUp {
        button: MouseButton,
        at: Point,
        time: DateTime<Utc>,
    },
    Wheel {
        /// Positivo = para cima / para a direita.
        delta: i32,
        horizontal: bool,
        at: Point,
        time: DateTime<Utc>,
    },
    Key {
        /// Virtual-key code da plataforma.
        vk: u32,
        /// Caractere digitado, quando a tecla produz texto.
        text: Option<String>,
        /// Nome legível da tecla ou combinação ("Enter", "Ctrl+S").
        combo: String,
        /// `true` se algum modificador (Ctrl/Alt/Win) estava pressionado.
        with_modifier: bool,
        at: Point,
        time: DateTime<Utc>,
    },
}

impl RawEvent {
    pub fn time(&self) -> DateTime<Utc> {
        match self {
            Self::MouseDown { time, .. }
            | Self::MouseUp { time, .. }
            | Self::Wheel { time, .. }
            | Self::Key { time, .. } => *time,
        }
    }

    pub fn at(&self) -> Point {
        match self {
            Self::MouseDown { at, .. }
            | Self::MouseUp { at, .. }
            | Self::Wheel { at, .. }
            | Self::Key { at, .. } => *at,
        }
    }
}

/// Uma captura de tela pronta, com a região do espaço virtual que ela cobre.
pub struct Frame {
    pub image: RgbaImage,
    /// Região capturada, em coordenadas de tela virtual. É o que permite
    /// converter a posição do cursor para dentro da imagem depois.
    pub rect: Rect,
    /// Título da janela capturada, quando o escopo é `ActiveWindow`.
    pub window_title: Option<String>,
    /// `true` quando o escopo pedido não pôde ser respeitado e caímos para o
    /// monitor sob o cursor (ex.: clique num menu popup fora da janela ativa).
    pub fallback: bool,
}

impl Frame {
    pub fn new(image: RgbaImage, rect: Rect) -> Self {
        Self {
            image,
            rect,
            window_title: None,
            fallback: false,
        }
    }

    pub fn with_window(mut self, title: Option<String>) -> Self {
        self.window_title = title;
        self
    }

    pub fn with_fallback(mut self, fallback: bool) -> Self {
        self.fallback = fallback;
        self
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("size", &(self.image.width(), self.image.height()))
            .field("rect", &self.rect)
            .field("window_title", &self.window_title)
            .field("fallback", &self.fallback)
            .finish()
    }
}
