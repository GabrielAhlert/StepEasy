//! Captura de tela em cima do `xcap`, comum a todas as plataformas.
//!
//! O `xcap` sabe capturar um monitor inteiro ou uma região dentro dele, em
//! coordenadas locais do monitor. O que falta — e é o que este módulo faz — é
//! traduzir um retângulo do espaço de tela virtual para os monitores que ele
//! atravessa e montar o resultado num único bitmap.
//!
//! A geometria de cada monitor pode vir de fora (no Windows ela é lida do
//! `GetMonitorInfoW`, que fala a mesma linguagem de coordenadas que o gancho
//! de mouse) ou do próprio `xcap`.

use image::RgbaImage;
use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::scope::{CaptureScope, MonitorInfo};

use crate::event::Frame;
use crate::{Error, Result};

struct Entry {
    info: MonitorInfo,
    monitor: xcap::Monitor,
}

/// Monitores conectados, prontos para capturar.
pub struct Screens {
    entries: Vec<Entry>,
}

impl Screens {
    /// Descobre os monitores pelo `xcap`.
    pub fn detect() -> Result<Self> {
        let monitors = xcap::Monitor::all().map_err(|e| Error::Screen(e.to_string()))?;
        let mut entries = Vec::new();
        for monitor in monitors {
            match info_from_xcap(&monitor) {
                Ok(info) => entries.push(Entry { info, monitor }),
                Err(err) => tracing::warn!("monitor ignorado: {err}"),
            }
        }
        if entries.is_empty() {
            return Err(Error::NoMonitors);
        }
        entries.sort_by_key(|e| (!e.info.is_primary, e.info.bounds.x, e.info.bounds.y));
        Ok(Self { entries })
    }

    /// Descobre os monitores com a geometria fornecida pela plataforma.
    ///
    /// Cada `MonitorInfo` é casado com o monitor do `xcap` que contém o seu
    /// centro; nomes de dispositivo não são comparáveis entre as duas fontes.
    pub fn with_geometry(infos: Vec<MonitorInfo>) -> Result<Self> {
        let mut entries = Vec::new();
        for info in infos {
            let center = center_of(&info.bounds);
            match xcap::Monitor::from_point(center.x, center.y) {
                Ok(monitor) => entries.push(Entry { info, monitor }),
                Err(err) => tracing::warn!("sem monitor xcap para {}: {err}", info.name),
            }
        }
        if entries.is_empty() {
            return Self::detect();
        }
        entries.sort_by_key(|e| (!e.info.is_primary, e.info.bounds.x, e.info.bounds.y));
        Ok(Self { entries })
    }

    pub fn infos(&self) -> Vec<MonitorInfo> {
        self.entries.iter().map(|e| e.info.clone()).collect()
    }

    /// Retângulo que cobre todos os monitores.
    pub fn virtual_bounds(&self) -> Rect {
        self.entries
            .iter()
            .fold(Rect::new(0, 0, 0, 0), |acc, e| acc.union(&e.info.bounds))
    }

    fn at(&self, point: Point) -> Option<&Entry> {
        self.entries.iter().find(|e| e.info.bounds.contains(point))
    }

    fn primary(&self) -> &Entry {
        self.entries
            .iter()
            .find(|e| e.info.is_primary)
            .unwrap_or(&self.entries[0])
    }

    /// Monitor que contém o ponto, ou o principal se o ponto estiver fora de
    /// todos (acontece em transições de resolução).
    pub fn monitor_bounds_at(&self, point: Point) -> Rect {
        self.at(point).unwrap_or_else(|| self.primary()).info.bounds
    }

    /// Resolve o escopo num retângulo do espaço virtual.
    pub fn resolve(
        &self,
        scope: &CaptureScope,
        at: Point,
        active_window: Option<Rect>,
    ) -> (Rect, bool) {
        let monitores: Vec<MonitorInfo> = self.infos();
        resolve_scope(&monitores, scope, at, active_window)
    }

    /// Captura o retângulo pedido, costurando os monitores que ele atravessa.
    pub fn grab_rect(&self, rect: Rect) -> Result<RgbaImage> {
        if rect.is_empty() {
            return Err(Error::Screen("região de captura vazia".into()));
        }

        let mut canvas = RgbaImage::new(rect.width, rect.height);
        let mut capturou = false;

        for entry in &self.entries {
            let Some(parte) = entry.info.bounds.intersect(&rect) else {
                continue;
            };
            let local_x = (parte.x - entry.info.bounds.x) as u32;
            let local_y = (parte.y - entry.info.bounds.y) as u32;

            let capturado = entry
                .monitor
                .capture_region(local_x, local_y, parte.width, parte.height)
                .map_err(|e| Error::Screen(e.to_string()))?;

            let dest_x = (parte.x - rect.x) as u32;
            let dest_y = (parte.y - rect.y) as u32;
            blit(&mut canvas, &capturado, dest_x, dest_y);
            capturou = true;
        }

        if !capturou {
            return Err(Error::Screen(
                "a região pedida não está em nenhum monitor".into(),
            ));
        }
        Ok(canvas)
    }

    /// Captura conforme o escopo, já devolvendo o `Frame` pronto.
    pub fn grab(
        &self,
        scope: &CaptureScope,
        at: Point,
        active_window: Option<(Rect, Option<String>)>,
    ) -> Result<Frame> {
        let (window_rect, window_title) = match active_window {
            Some((rect, title)) => (Some(rect), title),
            None => (None, None),
        };
        let (rect, fallback) = self.resolve(scope, at, window_rect);
        let image = self.grab_rect(rect)?;

        Ok(Frame::new(image, rect)
            .with_window(window_title)
            .with_fallback(fallback))
    }
}

/// Traduz o escopo escolhido num retângulo do espaço de tela virtual, e diz se
/// foi preciso abrir mão do que o usuário pediu.
///
/// Fica como função livre sobre a lista de monitores, e não como método do
/// [`Screens`], para poder ser testada com monitores inventados: o `Screens`
/// depende do `xcap` e de telas de verdade, e um teste que reimplementasse
/// estas regras estaria conferindo a si mesmo.
///
/// `active_window` é o retângulo da janela em foco, que só a plataforma sabe
/// descobrir. Quando o escopo é `ActiveWindow` e o cursor está fora dessa
/// janela — clique num menu suspenso, que é outra janela —, a captura cai para
/// o monitor sob o cursor e sinaliza `fallback`.
pub fn resolve_scope(
    monitores: &[MonitorInfo],
    scope: &CaptureScope,
    at: Point,
    active_window: Option<Rect>,
) -> (Rect, bool) {
    let virtual_bounds = monitores
        .iter()
        .fold(Rect::new(0, 0, 0, 0), |acc, m| acc.union(&m.bounds));

    let sob_cursor = monitores
        .iter()
        .find(|m| m.bounds.contains(at))
        .or_else(|| monitores.iter().find(|m| m.is_primary))
        .or_else(|| monitores.first())
        .map_or(virtual_bounds, |m| m.bounds);

    match scope {
        CaptureScope::AllMonitors => (virtual_bounds, false),
        CaptureScope::MonitorUnderCursor => (sob_cursor, false),
        CaptureScope::Monitor { id } => match monitores.iter().find(|m| &m.id == id) {
            Some(m) => (m.bounds, false),
            None => (sob_cursor, true),
        },
        CaptureScope::ActiveWindow => match active_window {
            Some(rect) if !rect.is_empty() && rect.contains(at) => (rect, false),
            _ => (sob_cursor, true),
        },
        CaptureScope::Region { rect } => match rect.intersect(&virtual_bounds) {
            Some(r) => (r, r != *rect),
            None => (sob_cursor, true),
        },
    }
}

fn center_of(rect: &Rect) -> Point {
    Point::new(
        rect.x + rect.width as i32 / 2,
        rect.y + rect.height as i32 / 2,
    )
}

fn info_from_xcap(monitor: &xcap::Monitor) -> Result<MonitorInfo> {
    let map = |e: xcap::XCapError| Error::Screen(e.to_string());
    let bounds = Rect::new(
        monitor.x().map_err(map)?,
        monitor.y().map_err(map)?,
        monitor.width().map_err(map)?,
        monitor.height().map_err(map)?,
    );
    let name = monitor
        .friendly_name()
        .or_else(|_| monitor.name())
        .map_err(map)?;
    Ok(MonitorInfo {
        id: monitor.name().map_err(map)?,
        name,
        bounds,
        is_primary: monitor.is_primary().unwrap_or(false),
        scale_factor: monitor.scale_factor().unwrap_or(1.0),
    })
}

/// Copia `src` para dentro de `dst` na posição indicada, recortando o excesso.
fn blit(dst: &mut RgbaImage, src: &RgbaImage, x: u32, y: u32) {
    let largura = src.width().min(dst.width().saturating_sub(x));
    let altura = src.height().min(dst.height().saturating_sub(y));
    for row in 0..altura {
        for col in 0..largura {
            dst.put_pixel(x + col, y + row, *src.get_pixel(col, row));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tela(id: &str, x: i32, y: i32, w: u32, h: u32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            name: id.into(),
            bounds: Rect::new(x, y, w, h),
            is_primary: primary,
            scale_factor: 1.0,
        }
    }

    /// Monitor principal com um segundo à esquerda, que é o arranjo que
    /// produz coordenadas negativas — onde os erros costumam aparecer.
    fn duas_telas() -> Vec<MonitorInfo> {
        vec![
            tela("primaria", 0, 0, 1920, 1080, true),
            tela("esquerda", -1920, 0, 1920, 1080, false),
        ]
    }

    #[test]
    fn todas_as_telas_cobre_o_canvas_virtual() {
        let (rect, fb) = resolve_scope(
            &duas_telas(),
            &CaptureScope::AllMonitors,
            Point::new(0, 0),
            None,
        );
        assert_eq!(rect, Rect::new(-1920, 0, 3840, 1080));
        assert!(!fb);
    }

    #[test]
    fn monitor_sob_o_cursor_segue_o_clique() {
        let telas = duas_telas();
        let (rect, _) = resolve_scope(
            &telas,
            &CaptureScope::MonitorUnderCursor,
            Point::new(-500, 20),
            None,
        );
        assert_eq!(rect, Rect::new(-1920, 0, 1920, 1080));

        let (rect, _) = resolve_scope(
            &telas,
            &CaptureScope::MonitorUnderCursor,
            Point::new(500, 20),
            None,
        );
        assert_eq!(rect, Rect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn janela_ativa_cai_para_o_monitor_quando_o_clique_e_fora_dela() {
        let telas = duas_telas();
        let janela = Rect::new(100, 100, 800, 600);

        let (rect, fb) = resolve_scope(
            &telas,
            &CaptureScope::ActiveWindow,
            Point::new(200, 200),
            Some(janela),
        );
        assert_eq!(rect, janela);
        assert!(!fb);

        // Clique num menu suspenso, fora da janela em foco.
        let (rect, fb) = resolve_scope(
            &telas,
            &CaptureScope::ActiveWindow,
            Point::new(1500, 900),
            Some(janela),
        );
        assert_eq!(rect, Rect::new(0, 0, 1920, 1080));
        assert!(fb, "deveria sinalizar que o escopo não foi respeitado");
    }

    #[test]
    fn monitor_removido_no_meio_da_gravacao_cai_para_o_do_cursor() {
        let (rect, fb) = resolve_scope(
            &duas_telas(),
            &CaptureScope::Monitor { id: "sumiu".into() },
            Point::new(10, 10),
            None,
        );
        assert_eq!(rect, Rect::new(0, 0, 1920, 1080));
        assert!(fb);
    }

    #[test]
    fn regiao_e_recortada_ao_que_existe_de_tela() {
        let pedido = Rect::new(1800, 0, 400, 400);
        let (rect, fb) = resolve_scope(
            &duas_telas(),
            &CaptureScope::Region { rect: pedido },
            Point::new(0, 0),
            None,
        );
        assert_eq!(rect, Rect::new(1800, 0, 120, 400));
        assert!(fb);
    }

    #[test]
    fn cursor_fora_de_qualquer_tela_cai_para_a_principal() {
        // Acontece de verdade entre uma troca de resolução e a próxima.
        let (rect, _) = resolve_scope(
            &duas_telas(),
            &CaptureScope::MonitorUnderCursor,
            Point::new(9999, 9999),
            None,
        );
        assert_eq!(rect, Rect::new(0, 0, 1920, 1080));
    }

    #[test]
    fn sem_monitor_nenhum_nao_entra_em_panico() {
        let (rect, _) = resolve_scope(
            &[],
            &CaptureScope::MonitorUnderCursor,
            Point::new(0, 0),
            None,
        );
        assert!(rect.is_empty());
    }

    #[test]
    fn blit_recorta_o_que_passa_da_borda() {
        let mut dst = RgbaImage::new(4, 4);
        let src = RgbaImage::from_pixel(3, 3, image::Rgba([9, 9, 9, 255]));
        blit(&mut dst, &src, 2, 2);
        assert_eq!(dst.get_pixel(3, 3)[0], 9);
        assert_eq!(dst.get_pixel(1, 1)[0], 0);
        assert_eq!((dst.width(), dst.height()), (4, 4));
    }
}
