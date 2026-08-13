//! Implementação de reserva para plataformas ainda não suportadas.
//!
//! A captura de tela funciona (o `xcap` cobre Linux e macOS); o que falta é o
//! gancho global de entrada e a ponte de acessibilidade. Assim o workspace
//! compila e roda fora do Windows, abrindo o editor e mostrando um aviso claro
//! em vez de simplesmente não existir.

use crossbeam_channel::Sender;
use stepeasy_core::geometry::Point;
use stepeasy_core::scope::{CaptureScope, MonitorInfo};
use stepeasy_core::UiTarget;

use crate::event::{Frame, RawEvent};
use crate::screens::Screens;
use crate::{Error, InputWatcher, Platform, Result, ScreenGrabber, UiProbe};

pub fn platform() -> Platform {
    Platform {
        input: Box::new(NoInput),
        screen: Box::new(XcapGrabber),
        probe: Box::new(NoProbe),
    }
}

struct NoInput;

impl InputWatcher for NoInput {
    fn start(&mut self, _tx: Sender<RawEvent>) -> Result<()> {
        Err(Error::Unsupported)
    }
    fn stop(&mut self) {}
}

struct NoProbe;

impl UiProbe for NoProbe {
    fn element_at(&self, _point: Point) -> Option<UiTarget> {
        None
    }
}

struct XcapGrabber;

impl ScreenGrabber for XcapGrabber {
    fn monitors(&self) -> Result<Vec<MonitorInfo>> {
        Ok(Screens::detect()?.infos())
    }

    fn grab(&self, scope: &CaptureScope, at: Point) -> Result<Frame> {
        // Sem API de janelas aqui: `ActiveWindow` cai para o monitor do cursor.
        Screens::detect()?.grab(scope, at, None)
    }
}
