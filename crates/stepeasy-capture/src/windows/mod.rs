//! Implementação Windows das três traits de captura.

mod hook;
mod screen;
mod uia;
mod win;

use crate::Platform;

pub fn platform() -> Platform {
    Platform {
        input: Box::new(hook::WindowsHooks::default()),
        screen: Box::new(screen::WindowsGrabber),
        probe: Box::new(uia::WindowsProbe),
    }
}
