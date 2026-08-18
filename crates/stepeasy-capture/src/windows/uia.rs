//! Acessibilidade via UI Automation.
//!
//! É o que separa "Clicou em (842, 391)" de "Clicou no botão **Salvar**".
//! O objeto `IUIAutomation` é criado uma vez por thread e reaproveitado —
//! `CoCreateInstance` a cada clique custaria mais que o próprio screenshot.

use std::cell::RefCell;

use stepeasy_core::geometry::{Point, Rect};
use stepeasy_core::UiTarget;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationElement};

use crate::UiProbe;

use super::win;

thread_local! {
    static AUTOMATION: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
}

#[derive(Default)]
pub struct WindowsProbe;

impl UiProbe for WindowsProbe {
    fn element_at(&self, point: Point) -> Option<UiTarget> {
        let mut target = UiTarget {
            window_title: win::window_title_at(point),
            process_name: win::process_name_at(point),
            ..Default::default()
        };

        if let Some(element) = element_at(point) {
            unsafe {
                target.name = element
                    .CurrentName()
                    .ok()
                    .map(|s| s.to_string())
                    .filter(|s| !s.trim().is_empty());

                target.control_type = element
                    .CurrentControlType()
                    .ok()
                    .map(|id| control_type_key(id).to_string());

                if let Ok(rect) = element.CurrentBoundingRectangle() {
                    let width = (rect.right - rect.left).max(0) as u32;
                    let height = (rect.bottom - rect.top).max(0) as u32;
                    if width > 0 && height > 0 {
                        target.bounds = Some(Rect::new(rect.left, rect.top, width, height));
                    }
                }
            }
        }

        (!target.is_blank()).then_some(target)
    }
}

fn element_at(point: Point) -> Option<IUIAutomationElement> {
    AUTOMATION.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            unsafe {
                // A thread de trabalho é nossa, então o modelo multithread é o
                // certo: não há bomba de mensagens aqui para servir um STA.
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                match CoCreateInstance::<_, IUIAutomation>(
                    &CUIAutomation,
                    None,
                    CLSCTX_INPROC_SERVER,
                ) {
                    Ok(automation) => *slot = Some(automation),
                    Err(err) => {
                        tracing::warn!("UI Automation indisponível: {err}");
                        return None;
                    }
                }
            }
        }
        let automation = slot.as_ref()?;
        unsafe {
            automation
                .ElementFromPoint(POINT {
                    x: point.x,
                    y: point.y,
                })
                .ok()
        }
    })
}

/// Converte o id de tipo de controle do UIA na **chave** usada pelas
/// traduções.
///
/// De propósito não devolve texto pronto: o texto é escolhido na hora de
/// montar a legenda, no idioma ativo. Devolver "botão" aqui prenderia toda a
/// gravação ao português — inclusive dentro do arquivo salvo.
/// A lista de chaves válidas está em `stepeasy_core::caption::CONTROLES`.
// As constantes do UIA usam camel case; casar com elas exige silenciar o lint.
#[allow(non_upper_case_globals)]
fn control_type_key(id: windows::Win32::UI::Accessibility::UIA_CONTROLTYPE_ID) -> &'static str {
    use windows::Win32::UI::Accessibility::*;

    match id {
        UIA_ButtonControlTypeId => "botao",
        UIA_CheckBoxControlTypeId => "caixa_selecao",
        UIA_ComboBoxControlTypeId => "caixa_combinacao",
        UIA_EditControlTypeId => "caixa_edicao",
        UIA_HyperlinkControlTypeId => "link",
        UIA_ImageControlTypeId => "imagem",
        UIA_ListItemControlTypeId => "item_lista",
        UIA_ListControlTypeId => "lista",
        UIA_MenuControlTypeId => "menu",
        UIA_MenuBarControlTypeId => "barra_menus",
        UIA_MenuItemControlTypeId => "item_menu",
        UIA_RadioButtonControlTypeId => "opcao",
        UIA_ScrollBarControlTypeId => "barra_rolagem",
        UIA_TabControlTypeId => "conjunto_guias",
        UIA_TabItemControlTypeId => "guia",
        UIA_TextControlTypeId => "texto",
        UIA_ToolBarControlTypeId => "barra_ferramentas",
        UIA_TreeControlTypeId => "arvore",
        UIA_TreeItemControlTypeId => "item_arvore",
        UIA_WindowControlTypeId => "janela",
        UIA_DataItemControlTypeId => "item",
        UIA_DataGridControlTypeId => "tabela",
        UIA_TableControlTypeId => "tabela",
        UIA_DocumentControlTypeId => "documento",
        UIA_SplitButtonControlTypeId => "botao_divisao",
        UIA_SliderControlTypeId => "deslizante",
        UIA_SpinnerControlTypeId => "seletor_numerico",
        UIA_ProgressBarControlTypeId => "barra_progresso",
        UIA_GroupControlTypeId => "grupo",
        UIA_PaneControlTypeId => "painel",
        _ => "elemento",
    }
}
