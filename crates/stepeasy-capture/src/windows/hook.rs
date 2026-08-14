//! Ganchos globais de mouse e teclado.
//!
//! `WH_MOUSE_LL` e `WH_KEYBOARD_LL` exigem uma thread com fila de mensagens, e
//! o callback roda **dentro** dessa fila: se ele demorar, o Windows derruba o
//! gancho e a entrada do sistema inteiro engasga. Por isso aqui só se monta o
//! [`RawEvent`] e se empurra pelo canal — screenshot e acessibilidade ficam
//! para a thread de trabalho.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use chrono::Utc;
use crossbeam_channel::Sender;
use stepeasy_core::geometry::Point;
use stepeasy_core::model::MouseButton;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardLayout, MapVirtualKeyExW, ToUnicodeEx, MAPVK_VK_TO_VSC,
    VIRTUAL_KEY, VK_CONTROL, VK_LMENU, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEWHEEL, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SYSKEYDOWN,
};

use crate::event::RawEvent;
use crate::{Error, InputWatcher, Result};

thread_local! {
    /// Canal da thread de ganchos. É `thread_local` porque o callback do
    /// Windows não recebe contexto e roda sempre nesta mesma thread.
    static SENDER: RefCell<Option<Sender<RawEvent>>> = const { RefCell::new(None) };
}

#[derive(Default)]
pub struct WindowsHooks {
    /// Id da thread dos ganchos, para conseguir mandá-la encerrar.
    thread_id: Arc<AtomicU32>,
    joiner: Option<std::thread::JoinHandle<()>>,
}

impl InputWatcher for WindowsHooks {
    fn start(&mut self, tx: Sender<RawEvent>) -> Result<()> {
        if self.joiner.is_some() {
            return Ok(());
        }

        let thread_id = self.thread_id.clone();
        let (ready_tx, ready_rx) = crossbeam_channel::bounded::<Result<()>>(1);

        let handle = std::thread::Builder::new()
            .name("stepeasy-hooks".into())
            .spawn(move || {
                SENDER.with(|s| *s.borrow_mut() = Some(tx));
                thread_id.store(
                    unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
                    Ordering::SeqCst,
                );

                let hooks = unsafe { install() };
                match hooks {
                    Ok((mouse, keyboard)) => {
                        let _ = ready_tx.send(Ok(()));
                        unsafe { pump() };
                        unsafe {
                            let _ = UnhookWindowsHookEx(mouse);
                            let _ = UnhookWindowsHookEx(keyboard);
                        }
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                    }
                }
                SENDER.with(|s| *s.borrow_mut() = None);
            })
            .map_err(|e| Error::Hook(e.to_string()))?;

        // Espera a instalação para poder relatar a falha de verdade ao usuário.
        match ready_rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(Ok(())) => {
                self.joiner = Some(handle);
                Ok(())
            }
            Ok(Err(err)) => Err(err),
            Err(_) => Err(Error::Hook(
                "a thread de ganchos não respondeu a tempo".into(),
            )),
        }
    }

    fn stop(&mut self) {
        let id = self.thread_id.swap(0, Ordering::SeqCst);
        if id != 0 {
            unsafe {
                let _ = PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(handle) = self.joiner.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for WindowsHooks {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe fn install() -> Result<(HHOOK, HHOOK)> {
    let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
        .map_err(|e| Error::Hook(format!("mouse: {e}")))?;
    let keyboard = match SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) {
        Ok(h) => h,
        Err(e) => {
            let _ = UnhookWindowsHookEx(mouse);
            return Err(Error::Hook(format!("teclado: {e}")));
        }
    };
    Ok((mouse, keyboard))
}

unsafe fn pump() {
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

fn emit(event: RawEvent) {
    SENDER.with(|s| {
        if let Some(tx) = s.borrow().as_ref() {
            let _ = tx.send(event);
        }
    });
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let data = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let at = Point::new(data.pt.x, data.pt.y);
        let time = Utc::now();

        let event = match wparam.0 as u32 {
            WM_LBUTTONDOWN => Some(RawEvent::MouseDown {
                button: MouseButton::Left,
                at,
                time,
            }),
            WM_LBUTTONUP => Some(RawEvent::MouseUp {
                button: MouseButton::Left,
                at,
                time,
            }),
            WM_RBUTTONDOWN => Some(RawEvent::MouseDown {
                button: MouseButton::Right,
                at,
                time,
            }),
            WM_RBUTTONUP => Some(RawEvent::MouseUp {
                button: MouseButton::Right,
                at,
                time,
            }),
            WM_MBUTTONDOWN => Some(RawEvent::MouseDown {
                button: MouseButton::Middle,
                at,
                time,
            }),
            WM_MBUTTONUP => Some(RawEvent::MouseUp {
                button: MouseButton::Middle,
                at,
                time,
            }),
            WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                // O delta vem na palavra alta do mouseData, com sinal.
                let delta = ((data.mouseData >> 16) & 0xFFFF) as i16 as i32;
                Some(RawEvent::Wheel {
                    delta,
                    horizontal: wparam.0 as u32 == WM_MOUSEHWHEEL,
                    at,
                    time,
                })
            }
            _ => None,
        };

        if let Some(event) = event {
            emit(event);
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        let data = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = VIRTUAL_KEY(data.vkCode as u16);

        // Modificadores sozinhos não viram passo.
        if !is_modifier(vk) {
            let ctrl = pressed(VK_CONTROL);
            let alt = pressed(VK_MENU);
            let win = pressed(VK_LWIN) || pressed(VK_RWIN);
            let shift = pressed(VK_SHIFT);
            let with_modifier = ctrl || alt || win;

            let text = if with_modifier {
                None
            } else {
                translate(vk, data.scanCode, shift)
            };

            let mut combo = String::new();
            if ctrl {
                combo.push_str("Ctrl+");
            }
            if alt {
                combo.push_str("Alt+");
            }
            if win {
                combo.push_str("Win+");
            }
            if shift && (with_modifier || text.is_none()) {
                combo.push_str("Shift+");
            }
            combo.push_str(&key_name(vk, text.as_deref()));

            let mut pos = windows::Win32::Foundation::POINT::default();
            let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pos);

            emit(RawEvent::Key {
                vk: data.vkCode,
                text,
                combo,
                with_modifier,
                at: Point::new(pos.x, pos.y),
                time: Utc::now(),
            });
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

fn is_modifier(vk: VIRTUAL_KEY) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        VK_CAPITAL, VK_LCONTROL, VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT,
    };
    matches!(
        vk,
        VK_SHIFT
            | VK_CONTROL
            | VK_MENU
            | VK_LSHIFT
            | VK_RSHIFT
            | VK_LCONTROL
            | VK_RCONTROL
            | VK_LMENU
            | VK_RMENU
            | VK_LWIN
            | VK_RWIN
            | VK_CAPITAL
    )
}

fn pressed(vk: VIRTUAL_KEY) -> bool {
    // O bit alto indica "pressionado agora".
    unsafe { (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0 }
}

/// Converte a tecla no caractere que ela produziria, respeitando o layout de
/// teclado ativo (ç, acentos, teclado ABNT2).
fn translate(vk: VIRTUAL_KEY, scan: u32, shift: bool) -> Option<String> {
    unsafe {
        let layout = GetKeyboardLayout(0);

        let mut state = [0u8; 256];
        if shift {
            state[VK_SHIFT.0 as usize] = 0x80;
        }
        if (GetAsyncKeyState(windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL.0 as i32)
            as u16
            & 0x0001)
            != 0
        {
            state[windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL.0 as usize] = 0x01;
        }

        let scan = if scan != 0 {
            scan
        } else {
            MapVirtualKeyExW(vk.0 as u32, MAPVK_VK_TO_VSC, Some(layout))
        };

        let mut buffer = [0u16; 8];
        // O último parâmetro 1<<2 pede para **não** alterar o estado de teclas
        // mortas do teclado: sem isso, digitar um acento aqui roubaria o acento
        // do aplicativo que está recebendo a digitação.
        let len = ToUnicodeEx(vk.0 as u32, scan, &state, &mut buffer, 1 << 2, Some(layout));
        if len <= 0 {
            return None;
        }
        let s = String::from_utf16_lossy(&buffer[..len as usize]);
        // Descarta controles (Enter, Tab, Backspace vêm como texto de controle).
        if s.chars().all(|c| c.is_control()) {
            return None;
        }
        Some(s)
    }
}

/// Nome legível da tecla, para montar combinações e passos de tecla.
fn key_name(vk: VIRTUAL_KEY, text: Option<&str>) -> String {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    let nomeado = match vk {
        VK_RETURN => "Enter",
        VK_TAB => "Tab",
        VK_ESCAPE => "Esc",
        VK_BACK => "Backspace",
        VK_DELETE => "Delete",
        VK_INSERT => "Insert",
        VK_HOME => "Home",
        VK_END => "End",
        VK_PRIOR => "Page Up",
        VK_NEXT => "Page Down",
        VK_UP => "Seta para cima",
        VK_DOWN => "Seta para baixo",
        VK_LEFT => "Seta para a esquerda",
        VK_RIGHT => "Seta para a direita",
        VK_SPACE => "Espaço",
        VK_SNAPSHOT => "Print Screen",
        VK_F1 => "F1",
        VK_F2 => "F2",
        VK_F3 => "F3",
        VK_F4 => "F4",
        VK_F5 => "F5",
        VK_F6 => "F6",
        VK_F7 => "F7",
        VK_F8 => "F8",
        VK_F9 => "F9",
        VK_F10 => "F10",
        VK_F11 => "F11",
        VK_F12 => "F12",
        _ => "",
    };

    if !nomeado.is_empty() {
        return nomeado.to_string();
    }
    if let Some(t) = text {
        return t.to_uppercase();
    }
    // Sem nome e sem texto: cai para a letra/dígito do código virtual.
    match vk.0 {
        0x30..=0x39 | 0x41..=0x5A => char::from(vk.0 as u8).to_string(),
        outro => format!("Tecla {outro}"),
    }
}
