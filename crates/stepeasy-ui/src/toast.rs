//! Avisos curtos no rodapé — confirmações de salvamento e erros de captura.

use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Error,
}

pub struct Toast {
    pub text: String,
    pub level: Level,
    born: Instant,
}

#[derive(Default)]
pub struct Toasts {
    current: Option<Toast>,
}

impl Toasts {
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(text.into(), Level::Info);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        let text = text.into();
        tracing::error!("{text}");
        self.push(text, Level::Error);
    }

    fn push(&mut self, text: String, level: Level) {
        self.current = Some(Toast {
            text,
            level,
            born: Instant::now(),
        });
    }

    /// Aviso ativo, já descartando o que expirou.
    pub fn active(&mut self) -> Option<&Toast> {
        if let Some(t) = &self.current {
            if t.born.elapsed() > TTL {
                self.current = None;
            }
        }
        self.current.as_ref()
    }

    pub fn dismiss(&mut self) {
        self.current = None;
    }
}
