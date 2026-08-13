//! Captura de tela no Windows.

use stepeasy_core::geometry::Point;
use stepeasy_core::scope::{CaptureScope, MonitorInfo};

use crate::event::Frame;
use crate::screens::Screens;
use crate::{Result, ScreenGrabber};

use super::win;

#[derive(Default)]
pub struct WindowsGrabber;

impl ScreenGrabber for WindowsGrabber {
    fn monitors(&self) -> Result<Vec<MonitorInfo>> {
        let infos = win::monitors();
        if infos.is_empty() {
            return Ok(Screens::detect()?.infos());
        }
        Ok(infos)
    }

    fn grab(&self, scope: &CaptureScope, at: Point) -> Result<Frame> {
        // A lista de monitores é relida a cada captura: o usuário pode conectar
        // ou desconectar uma tela no meio da gravação, e uma lista velha faria
        // o recorte sair de um lugar que não existe mais.
        let screens = Screens::with_geometry(win::monitors())?;

        let active = if matches!(scope, CaptureScope::ActiveWindow) {
            win::foreground_window()
        } else {
            None
        };
        screens.grab(scope, at, active)
    }
}
