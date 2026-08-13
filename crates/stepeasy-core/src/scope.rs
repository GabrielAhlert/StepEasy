//! Escopo de captura: o que entra em cada screenshot.

use serde::{Deserialize, Serialize};

use crate::geometry::Rect;

/// Identificador estável de monitor dentro de uma sessão.
///
/// No Windows é derivado do nome do dispositivo (`\\.\DISPLAY1`); guardamos a
/// string para que uma gravação salva continue legível mesmo em outra máquina.
pub type MonitorId = String;

/// O que capturar a cada passo. Escolhido antes de gravar e mantido durante
/// toda a gravação.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum CaptureScope {
    /// Canvas virtual inteiro, todos os monitores lado a lado.
    AllMonitors,
    /// Um monitor fixo, escolhido pelo usuário.
    Monitor { id: MonitorId },
    /// Segue o monitor onde o clique aconteceu.
    #[default]
    MonitorUnderCursor,
    /// Apenas a janela em foco, recortada.
    ActiveWindow,
    /// Retângulo fixo definido pelo usuário.
    Region { rect: Rect },
}

impl CaptureScope {
    /// Rótulo curto para a UI e para o manifesto.
    pub fn label(&self) -> &'static str {
        match self {
            Self::AllMonitors => "Todas as telas",
            Self::Monitor { .. } => "Tela específica",
            Self::MonitorUnderCursor => "Tela sob o cursor",
            Self::ActiveWindow => "Janela ativa",
            Self::Region { .. } => "Região",
        }
    }

    /// Modos que produzem imagens de tamanho constante durante a gravação.
    ///
    /// `ActiveWindow` e `MonitorUnderCursor` variam conforme o foco/posição, o
    /// que o editor precisa saber para não assumir dimensões uniformes.
    pub fn has_fixed_size(&self) -> bool {
        matches!(
            self,
            Self::AllMonitors | Self::Monitor { .. } | Self::Region { .. }
        )
    }
}

/// Descrição de um monitor conectado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorInfo {
    pub id: MonitorId,
    /// Nome amigável para a UI (ex.: "Tela 1 — 2560×1440").
    pub name: String,
    pub bounds: Rect,
    pub is_primary: bool,
    /// Fator de escala do sistema (1.0 = 100%, 1.5 = 150%).
    pub scale_factor: f32,
}

impl MonitorInfo {
    /// Rótulo pronto para o seletor de monitores.
    pub fn display_label(&self) -> String {
        let primary = if self.is_primary { " (principal)" } else { "" };
        format!(
            "{} — {}×{}{}",
            self.name, self.bounds.width, self.bounds.height, primary
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializa_com_tag_de_modo() {
        let json = serde_json::to_string(&CaptureScope::ActiveWindow).unwrap();
        assert_eq!(json, r#"{"mode":"active_window"}"#);

        let scope = CaptureScope::Region {
            rect: Rect::new(0, 0, 800, 600),
        };
        let round: CaptureScope = serde_json::from_str(&serde_json::to_string(&scope).unwrap()).unwrap();
        assert_eq!(round, scope);
    }

    #[test]
    fn tamanho_fixo_por_modo() {
        assert!(CaptureScope::AllMonitors.has_fixed_size());
        assert!(!CaptureScope::ActiveWindow.has_fixed_size());
        assert!(!CaptureScope::MonitorUnderCursor.has_fixed_size());
    }
}
