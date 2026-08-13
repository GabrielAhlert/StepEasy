//! Utilidades Win32 usadas por mais de um módulo: geometria de monitores, da
//! janela em foco e nome do processo.

use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::scope::MonitorInfo;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    GA_ROOT, MONITORINFOF_PRIMARY,
};

/// Monitores conectados, em coordenadas de tela virtual — as mesmas que o
/// gancho de mouse entrega.
///
/// É por isso que a geometria não vem do `xcap`: ele lê a posição do
/// `DEVMODE`, que em arranjos com DPIs diferentes não bate com o espaço em que
/// o cursor é reportado.
pub fn monitors() -> Vec<MonitorInfo> {
    let mut out: Vec<MonitorInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut out as *mut Vec<MonitorInfo> as isize),
        );
    }
    out.sort_by_key(|m| (!m.is_primary, m.bounds.x, m.bounds.y));
    out
}

unsafe extern "system" fn enum_proc(
    monitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<MonitorInfo>);

    let mut info = MONITORINFOEXW {
        monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    if GetMonitorInfoW(monitor, &mut info.monitorInfo as *mut _ as *mut _).as_bool() {
        let r = info.monitorInfo.rcMonitor;
        let device = String::from_utf16_lossy(&info.szDevice)
            .trim_end_matches('\0')
            .to_string();
        let is_primary = info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0;

        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

        let index = out.len() + 1;
        out.push(MonitorInfo {
            id: if device.is_empty() {
                format!("monitor-{index}")
            } else {
                device
            },
            name: format!("Tela {index}"),
            bounds: Rect::new(
                r.left,
                r.top,
                (r.right - r.left).max(0) as u32,
                (r.bottom - r.top).max(0) as u32,
            ),
            is_primary,
            scale_factor: dpi_x as f32 / 96.0,
        });
    }
    TRUE
}

/// Retângulo e título da janela em foco.
///
/// Usa `DWMWA_EXTENDED_FRAME_BOUNDS` em vez de `GetWindowRect`: desde o Vista
/// o retângulo clássico inclui a sombra invisível da borda, o que deixaria uma
/// faixa de área de trabalho aparecendo nas laterais da captura.
pub fn foreground_window() -> Option<(Rect, Option<String>)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() || !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        window_rect(hwnd).map(|rect| (rect, window_title(hwnd)))
    }
}

/// Título da janela de nível superior que contém o ponto.
pub fn window_title_at(point: Point) -> Option<String> {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(POINT {
            x: point.x,
            y: point.y,
        });
        if hwnd.is_invalid() {
            return None;
        }
        let root = GetAncestor(hwnd, GA_ROOT);
        window_title(if root.is_invalid() { hwnd } else { root })
    }
}

/// Nome do executável dono da janela sob o ponto, ex.: `notepad.exe`.
pub fn process_name_at(point: Point) -> Option<String> {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::WindowFromPoint(POINT {
            x: point.x,
            y: point.y,
        });
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }

        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 260];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
        .is_ok();
        let _ = windows::Win32::Foundation::CloseHandle(handle);

        if !ok {
            return None;
        }
        let full = String::from_utf16_lossy(&buffer[..len as usize]);
        full.rsplit('\\').next().map(str::to_string)
    }
}

unsafe fn window_rect(hwnd: HWND) -> Option<Rect> {
    let mut rect = RECT::default();
    let ok = DwmGetWindowAttribute(
        hwnd,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut rect as *mut RECT as *mut std::ffi::c_void,
        std::mem::size_of::<RECT>() as u32,
    )
    .is_ok();

    if !ok {
        // Janelas sem composição (algumas caixas de diálogo antigas) não
        // respondem ao DWM; aí o retângulo clássico é o que existe.
        let mut classic = RECT::default();
        windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut classic).ok()?;
        rect = classic;
    }

    let width = (rect.right - rect.left).max(0) as u32;
    let height = (rect.bottom - rect.top).max(0) as u32;
    if width == 0 || height == 0 {
        return None;
    }
    Some(Rect::new(rect.left, rect.top, width, height))
}

unsafe fn window_title(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 512];
    let len = GetWindowTextW(hwnd, &mut buffer);
    if len <= 0 {
        return None;
    }
    let title = String::from_utf16_lossy(&buffer[..len as usize]);
    (!title.trim().is_empty()).then_some(title)
}
