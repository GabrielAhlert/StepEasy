//! Geração do texto de cada passo a partir do evento e do controle alvo.
//!
//! A i18n entra na M6; por enquanto os textos são pt-BR literais, mas todos
//! nascem aqui, num ponto só, para a troca ser mecânica depois.

use crate::model::{Step, StepKind, UiTarget};

/// Monta a legenda de um passo.
///
/// Com metadados de acessibilidade: *"Clicou no botão **Salvar** da janela
/// **Bloco de Notas**"*. Sem eles, cai para as coordenadas do clique.
pub fn generate(step: &Step) -> String {
    let target = step.target.as_ref().filter(|t| !t.is_blank());

    match &step.kind {
        StepKind::Click { button } => click_caption("Clicou", *button, step, target),
        StepKind::DoubleClick { button } => click_caption("Clicou duas vezes", *button, step, target),
        StepKind::Drag { button, to } => {
            let origem = step
                .cursor
                .map(|p| format!(" de ({}, {})", p.x, p.y))
                .unwrap_or_default();
            let texto = format!(
                "Arrastou com o botão {}{} até ({}, {})",
                button.label(),
                origem,
                to.x,
                to.y
            );
            with_window(texto, target)
        }
        StepKind::Type { text } => {
            let onde = describe_target(target)
                .map(|d| format!(" {d}"))
                .unwrap_or_default();
            with_window(format!("Digitou \"{}\"{}", text.trim_end(), onde), target)
        }
        StepKind::Key { combo } => with_window(format!("Pressionou {combo}"), target),
        StepKind::Scroll { direction, amount } => {
            let onde = describe_target(target)
                .map(|d| format!(" {d}"))
                .unwrap_or_default();
            let _ = amount;
            with_window(format!("Rolou a tela {}{}", direction.label(), onde), target)
        }
        StepKind::Manual => step.caption.clone(),
        StepKind::Merged { count } => format!("{count} ações agrupadas"),
    }
}

/// Reaplica a geração automática, respeitando edições manuais.
pub fn refresh(step: &mut Step) {
    if step.caption_edited || matches!(step.kind, StepKind::Manual) {
        return;
    }
    step.caption = generate(step);
}

fn click_caption(
    verbo: &str,
    button: crate::model::MouseButton,
    step: &Step,
    target: Option<&UiTarget>,
) -> String {
    let texto = match describe_target(target) {
        Some(alvo) => format!("{verbo} com o botão {} {alvo}", button.label()),
        None => match step.cursor {
            Some(p) => format!(
                "{verbo} com o botão {} em ({}, {})",
                button.label(),
                p.x,
                p.y
            ),
            None => format!("{verbo} com o botão {}", button.label()),
        },
    };
    with_window(texto, target)
}

/// Devolve a frase do alvo **já com a preposição**: "no botão **Salvar**",
/// "na caixa de edição", "em **Nome do arquivo:**".
fn describe_target(target: Option<&UiTarget>) -> Option<String> {
    let t = target?;
    match (t.control_type.as_deref(), t.name.as_deref()) {
        (Some(tipo), Some(nome)) if !nome.trim().is_empty() => {
            Some(format!("{} **{}**", artigo(tipo), nome.trim()))
        }
        (Some(tipo), _) => Some(artigo(tipo)),
        (None, Some(nome)) if !nome.trim().is_empty() => Some(format!("em **{}**", nome.trim())),
        _ => None,
    }
}

/// Prefixa o tipo de controle com o artigo correto ("no botão", "na caixa").
fn artigo(tipo: &str) -> String {
    const FEMININOS: &[&str] = &[
        "caixa",
        "lista",
        "guia",
        "barra",
        "janela",
        "tabela",
        "célula",
        "imagem",
        "árvore",
        "opção",
    ];
    let primeira = tipo.split_whitespace().next().unwrap_or(tipo).to_lowercase();
    if FEMININOS.contains(&primeira.as_str()) {
        format!("na {tipo}")
    } else {
        format!("no {tipo}")
    }
}

/// Acrescenta " da janela **Título**" quando conhecemos a janela.
fn with_window(texto: String, target: Option<&UiTarget>) -> String {
    match target.and_then(|t| t.window_title.as_deref()) {
        Some(titulo) if !titulo.trim().is_empty() => {
            format!("{texto} da janela **{}**", titulo.trim())
        }
        _ => texto,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::{MouseButton, ScrollDirection};

    fn com_alvo(kind: StepKind, target: UiTarget) -> Step {
        let mut s = Step::new(kind);
        s.cursor = Some(Point::new(842, 391));
        s.target = Some(target);
        s
    }

    #[test]
    fn clique_com_acessibilidade() {
        let step = com_alvo(
            StepKind::Click {
                button: MouseButton::Left,
            },
            UiTarget {
                name: Some("Salvar".into()),
                control_type: Some("botão".into()),
                window_title: Some("Bloco de Notas".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            generate(&step),
            "Clicou com o botão esquerdo no botão **Salvar** da janela **Bloco de Notas**"
        );
    }

    #[test]
    fn clique_sem_acessibilidade_usa_coordenadas() {
        let mut step = Step::new(StepKind::Click {
            button: MouseButton::Right,
        });
        step.cursor = Some(Point::new(842, 391));
        assert_eq!(generate(&step), "Clicou com o botão direito em (842, 391)");
    }

    #[test]
    fn artigo_feminino_para_caixa() {
        let step = com_alvo(
            StepKind::Type {
                text: "relatorio.pdf".into(),
            },
            UiTarget {
                name: Some("Nome do arquivo:".into()),
                control_type: Some("caixa de edição".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            generate(&step),
            "Digitou \"relatorio.pdf\" na caixa de edição **Nome do arquivo:**"
        );
    }

    #[test]
    fn refresh_nao_sobrescreve_edicao_manual() {
        let mut step = Step::new(StepKind::Key {
            combo: "Ctrl+S".into(),
        });
        refresh(&mut step);
        assert_eq!(step.caption, "Pressionou Ctrl+S");

        step.caption = "Salve o arquivo".into();
        step.caption_edited = true;
        refresh(&mut step);
        assert_eq!(step.caption, "Salve o arquivo");
    }

    #[test]
    fn scroll_sem_alvo() {
        let step = Step::new(StepKind::Scroll {
            direction: ScrollDirection::Down,
            amount: 3,
        });
        assert_eq!(generate(&step), "Rolou a tela para baixo");
    }
}
