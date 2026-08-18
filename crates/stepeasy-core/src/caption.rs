//! Geração do texto de cada passo a partir do evento e do controle alvo.
//!
//! **Nenhuma frase é montada em código.** O idioma fornece a sentença inteira,
//! com espaços nomeados, e decide sozinho ordem e preposição:
//!
//! ```text
//! pt-BR   clique: "Clicou com o botão %{botao} %{alvo}"
//!         alvo.com_nome: "%{tipo} **%{nome}**"
//!         controle.botao: "no botão"
//!         → Clicou com o botão esquerdo no botão **Salvar**
//!
//! en      clique: "Clicked %{alvo} with the %{botao} mouse button"
//!         alvo.com_nome: "the **%{nome}** %{tipo}"
//!         controle.botao: "button"
//!         → Clicked the **Save** button with the left mouse button
//! ```
//!
//! Repare que o "no/na" do português é **dado**, não lógica: ele mora no valor
//! de `controle.botao`. Uma versão anterior deste módulo decidia o artigo por
//! gênero em código, o que tornava a tradução impossível — em inglês o artigo
//! some e a ordem do tipo e do nome inverte.

use rust_i18n::t;

use crate::model::{MouseButton, ScrollDirection, Step, StepKind, UiTarget};

/// Chave de tradução do tipo de controle, vinda da plataforma.
///
/// A camada de acessibilidade entrega uma destas em `UiTarget::control_type`,
/// e não um texto pronto — texto pronto não se traduz.
pub const CONTROLES: &[&str] = &[
    "botao",
    "caixa_selecao",
    "caixa_combinacao",
    "caixa_edicao",
    "link",
    "imagem",
    "item_lista",
    "lista",
    "menu",
    "barra_menus",
    "item_menu",
    "opcao",
    "barra_rolagem",
    "conjunto_guias",
    "guia",
    "texto",
    "barra_ferramentas",
    "arvore",
    "item_arvore",
    "janela",
    "item",
    "tabela",
    "documento",
    "botao_divisao",
    "deslizante",
    "seletor_numerico",
    "barra_progresso",
    "grupo",
    "painel",
    "elemento",
];

/// Monta a legenda de um passo no idioma ativo.
pub fn generate(step: &Step) -> String {
    let target = step.target.as_ref().filter(|t| !t.is_blank());
    let alvo = describe_target(target);
    let (x, y) = step.cursor.map_or((0, 0), |p| (p.x, p.y));

    let frase = match &step.kind {
        StepKind::Click { button } => match (&alvo, step.cursor) {
            (Some(alvo), _) => t!("legenda.clique", botao = botao(*button), alvo = alvo),
            (None, Some(_)) => t!("legenda.clique_em", botao = botao(*button), x = x, y = y),
            (None, None) => t!("legenda.clique_sem_alvo", botao = botao(*button)),
        },

        StepKind::DoubleClick { button } => match (&alvo, step.cursor) {
            (Some(alvo), _) => t!("legenda.duplo_clique", botao = botao(*button), alvo = alvo),
            (None, Some(_)) => {
                t!(
                    "legenda.duplo_clique_em",
                    botao = botao(*button),
                    x = x,
                    y = y
                )
            }
            (None, None) => t!("legenda.duplo_clique_sem_alvo", botao = botao(*button)),
        },

        StepKind::Drag { button, to } => match step.cursor {
            Some(_) => t!(
                "legenda.arrasto",
                botao = botao(*button),
                x = x,
                y = y,
                x2 = to.x,
                y2 = to.y
            ),
            None => t!(
                "legenda.arrasto_sem_origem",
                botao = botao(*button),
                x2 = to.x,
                y2 = to.y
            ),
        },

        StepKind::Type { text } => {
            let texto = text.trim_end();
            match &alvo {
                Some(alvo) => t!("legenda.digitou", texto = texto, alvo = alvo),
                None => t!("legenda.digitou_sem_alvo", texto = texto),
            }
        }

        StepKind::Key { combo } => t!("legenda.tecla", combo = combo),

        StepKind::Scroll { direction, .. } => match &alvo {
            Some(alvo) => t!(
                "legenda.rolagem",
                direcao = direcao(*direction),
                alvo = alvo
            ),
            None => t!("legenda.rolagem_sem_alvo", direcao = direcao(*direction)),
        },

        StepKind::Manual => return step.caption.clone(),

        StepKind::Merged { count } => t!("legenda.agrupado", n = count),
    };

    with_window(frase.to_string(), target)
}

/// Reaplica a geração automática, respeitando edições manuais.
pub fn refresh(step: &mut Step) {
    if step.caption_edited || matches!(step.kind, StepKind::Manual) {
        return;
    }
    step.caption = generate(step);
}

fn botao(button: MouseButton) -> String {
    match button {
        MouseButton::Left => t!("legenda.botao.esquerdo"),
        MouseButton::Right => t!("legenda.botao.direito"),
        MouseButton::Middle => t!("legenda.botao.meio"),
    }
    .to_string()
}

fn direcao(direction: ScrollDirection) -> String {
    match direction {
        ScrollDirection::Up => t!("legenda.direcao.cima"),
        ScrollDirection::Down => t!("legenda.direcao.baixo"),
        ScrollDirection::Left => t!("legenda.direcao.esquerda"),
        ScrollDirection::Right => t!("legenda.direcao.direita"),
    }
    .to_string()
}

/// Traduz a chave de tipo de controle. Chave desconhecida vira "elemento", que
/// é melhor do que mostrar o identificador cru para o usuário.
fn tipo_de_controle(chave: &str) -> String {
    if CONTROLES.contains(&chave) {
        // A chave precisa viver até o fim da chamada; `t!` empresta e devolve
        // um `Cow` preso ao argumento.
        let key = format!("legenda.controle.{chave}");
        t!(&key).to_string()
    } else {
        t!("legenda.controle.elemento").to_string()
    }
}

/// A frase do alvo, já no formato que o idioma pede.
fn describe_target(target: Option<&UiTarget>) -> Option<String> {
    let t = target?;
    let nome = t.name.as_deref().map(str::trim).filter(|n| !n.is_empty());

    match (t.control_type.as_deref(), nome) {
        (Some(chave), Some(nome)) => Some(
            t!(
                "legenda.alvo.com_nome",
                tipo = tipo_de_controle(chave),
                nome = nome
            )
            .to_string(),
        ),
        (Some(chave), None) => {
            Some(t!("legenda.alvo.so_tipo", tipo = tipo_de_controle(chave)).to_string())
        }
        (None, Some(nome)) => Some(t!("legenda.alvo.so_nome", nome = nome).to_string()),
        (None, None) => None,
    }
}

/// Envolve a frase com a menção à janela, quando ela é conhecida.
fn with_window(frase: String, target: Option<&UiTarget>) -> String {
    match target
        .and_then(|t| t.window_title.as_deref())
        .map(str::trim)
    {
        Some(janela) if !janela.is_empty() => {
            t!("legenda.janela", frase = frase, janela = janela).to_string()
        }
        _ => frase,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    use crate::teste::com_idioma;

    fn passo_com_alvo(kind: StepKind, target: UiTarget) -> Step {
        let mut s = Step::new(kind);
        s.cursor = Some(Point::new(842, 391));
        s.target = Some(target);
        s
    }

    fn botao_salvar() -> UiTarget {
        UiTarget {
            name: Some("Salvar".into()),
            control_type: Some("botao".into()),
            window_title: Some("Bloco de Notas".into()),
            ..Default::default()
        }
    }

    #[test]
    fn clique_em_portugues() {
        let step = passo_com_alvo(
            StepKind::Click {
                button: MouseButton::Left,
            },
            botao_salvar(),
        );
        assert_eq!(
            com_idioma("pt-BR", || generate(&step)),
            "Clicou com o botão esquerdo no botão **Salvar** da janela **Bloco de Notas**"
        );
    }

    #[test]
    fn o_mesmo_passo_em_ingles_inverte_a_ordem() {
        // É este teste que prova que a gramática saiu do código: o tipo vem
        // depois do nome, e a preposição do português desaparece.
        let step = passo_com_alvo(
            StepKind::Click {
                button: MouseButton::Left,
            },
            botao_salvar(),
        );
        assert_eq!(
            com_idioma("en", || generate(&step)),
            "Clicked the **Salvar** button with the left mouse button \
             in the **Bloco de Notas** window"
        );
    }

    #[test]
    fn sem_acessibilidade_usa_coordenadas() {
        let mut step = Step::new(StepKind::Click {
            button: MouseButton::Right,
        });
        step.cursor = Some(Point::new(842, 391));
        assert_eq!(
            com_idioma("pt-BR", || generate(&step)),
            "Clicou com o botão direito em (842, 391)"
        );
    }

    #[test]
    fn digitacao_com_alvo() {
        let step = passo_com_alvo(
            StepKind::Type {
                text: "relatorio.pdf".into(),
            },
            UiTarget {
                name: Some("Nome do arquivo:".into()),
                control_type: Some("caixa_edicao".into()),
                ..Default::default()
            },
        );
        assert_eq!(
            com_idioma("pt-BR", || generate(&step)),
            "Digitou \"relatorio.pdf\" na caixa de edição **Nome do arquivo:**"
        );
    }

    #[test]
    fn tipo_de_controle_desconhecido_nao_vaza_a_chave() {
        let step = passo_com_alvo(
            StepKind::Click {
                button: MouseButton::Left,
            },
            UiTarget {
                control_type: Some("widget_exotico_do_futuro".into()),
                ..Default::default()
            },
        );
        let texto = com_idioma("pt-BR", || generate(&step));
        assert!(
            texto.contains("elemento") && !texto.contains("widget_exotico"),
            "vazou a chave crua: {texto}"
        );
    }

    #[test]
    fn refresh_nao_sobrescreve_edicao_manual() {
        com_idioma("pt-BR", || {
            let mut step = Step::new(StepKind::Key {
                combo: "Ctrl+S".into(),
            });
            refresh(&mut step);
            assert_eq!(step.caption, "Pressionou Ctrl+S");

            step.caption = "Salve o arquivo".into();
            step.caption_edited = true;
            refresh(&mut step);
            assert_eq!(step.caption, "Salve o arquivo");
        });
    }

    #[test]
    fn rolagem_sem_alvo() {
        let step = Step::new(StepKind::Scroll {
            direction: ScrollDirection::Down,
            amount: 3,
        });
        assert_eq!(
            com_idioma("pt-BR", || generate(&step)),
            "Rolou a tela para baixo"
        );
    }

    #[test]
    fn todo_tipo_de_controle_tem_traducao_nos_dois_idiomas() {
        for locale in ["pt-BR", "en"] {
            for chave in CONTROLES {
                let texto = com_idioma(locale, || tipo_de_controle(chave));
                assert!(
                    !texto.contains("legenda.controle"),
                    "{locale}: falta tradução para {chave}"
                );
            }
        }
    }
}
