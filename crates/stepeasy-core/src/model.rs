//! Modelo de dados de uma gravação.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::{Point, Rect};
use crate::scope::CaptureScope;

/// Versão do formato `.stepeasy` que este build escreve.
pub const FORMAT_VERSION: u32 = 1;

/// Uma gravação completa: metadados + passos ordenados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recording {
    pub id: Uuid,
    pub format_version: u32,
    pub app_version: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub modified_at: Option<DateTime<Utc>>,
    /// Escopo usado durante a captura.
    #[serde(default)]
    pub scope: CaptureScope,
    #[serde(default)]
    pub steps: Vec<Step>,
}

impl Recording {
    pub fn new(title: impl Into<String>, scope: CaptureScope) -> Self {
        Self {
            id: Uuid::new_v4(),
            format_version: FORMAT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            title: title.into(),
            description: String::new(),
            created_at: Utc::now(),
            modified_at: None,
            scope,
            steps: Vec::new(),
        }
    }

    /// Renumera `Step::index` em 1..=n. Deve ser chamado depois de qualquer
    /// operação que mexa na ordem ou na quantidade de passos.
    pub fn reindex(&mut self) {
        for (i, step) in self.steps.iter_mut().enumerate() {
            step.index = i as u32 + 1;
        }
    }

    pub fn step_by_id(&self, id: Uuid) -> Option<&Step> {
        self.steps.iter().find(|s| s.id == id)
    }

    pub fn step_by_id_mut(&mut self, id: Uuid) -> Option<&mut Step> {
        self.steps.iter_mut().find(|s| s.id == id)
    }

    pub fn position_of(&self, id: Uuid) -> Option<usize> {
        self.steps.iter().position(|s| s.id == id)
    }

    /// Primeiro número livre para nomear imagens novas neste pacote.
    ///
    /// Continuar uma gravação precisa disso: reiniciar do 1 sobrescreveria as
    /// capturas que já estão lá. Os nomes não têm relação com a ordem dos
    /// passos — reordenar não renomeia nada.
    pub fn next_image_index(&self) -> u32 {
        self.steps
            .iter()
            .filter_map(|s| s.image.as_ref())
            .filter_map(|i| crate::bundle::index_from_path(&i.path))
            .max()
            .map_or(1, |maior| maior + 1)
    }

    /// Nome de arquivo sugerido, sem extensão.
    pub fn slug(&self) -> String {
        let base: String = self
            .title
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect();
        let slug = base
            .split('-')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("-")
            .to_lowercase();
        if slug.is_empty() {
            format!("gravacao-{}", self.created_at.format("%Y%m%d-%H%M%S"))
        } else {
            slug
        }
    }
}

/// Um passo da gravação.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: Uuid,
    /// Posição 1-based, recalculada por [`Recording::reindex`].
    pub index: u32,
    pub kind: StepKind,
    pub timestamp: DateTime<Utc>,
    /// Texto exibido. Gerado a partir de `kind`/`target`, editável.
    pub caption: String,
    /// Marcado quando o usuário edita a legenda à mão — impede que a geração
    /// automática sobrescreva o texto dele.
    #[serde(default)]
    pub caption_edited: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub image: Option<ImageRef>,
    /// Posição do cursor no espaço de tela virtual.
    #[serde(default)]
    pub cursor: Option<Point>,
    /// Controle sob o cursor, quando a plataforma soube dizer.
    #[serde(default)]
    pub target: Option<UiTarget>,
    /// `true` quando o escopo pedido não pôde ser usado neste passo (ex.: clique
    /// num menu popup fora da janela ativa) e caímos para o monitor sob o cursor.
    #[serde(default)]
    pub scope_fallback: bool,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

impl Step {
    pub fn new(kind: StepKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            index: 0,
            kind,
            timestamp: Utc::now(),
            caption: String::new(),
            caption_edited: false,
            notes: String::new(),
            image: None,
            cursor: None,
            target: None,
            scope_fallback: false,
            annotations: Vec::new(),
        }
    }

    /// Passo escrito à mão pelo usuário, sem imagem.
    pub fn manual(text: impl Into<String>) -> Self {
        let mut step = Self::new(StepKind::Manual);
        step.caption = text.into();
        step.caption_edited = true;
        step
    }

    /// Converte a posição do cursor para coordenadas de pixel dentro da imagem
    /// deste passo. `None` se não há cursor, não há imagem, ou o cursor caiu
    /// fora do recorte.
    pub fn cursor_in_image(&self) -> Option<(u32, u32)> {
        let image = self.image.as_ref()?;
        image.source_rect.to_local(self.cursor?)
    }
}

/// O que o usuário fez neste passo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepKind {
    Click {
        button: MouseButton,
    },
    DoubleClick {
        button: MouseButton,
    },
    Drag {
        button: MouseButton,
        to: Point,
    },
    /// Digitação agrupada.
    Type {
        text: String,
    },
    /// Tecla ou combinação não-imprimível (ex.: "Ctrl+S", "Enter").
    Key {
        combo: String,
    },
    Scroll {
        direction: ScrollDirection,
        amount: i32,
    },
    /// Passo inserido manualmente no editor.
    Manual,
    /// Vários passos mesclados em um só.
    Merged {
        count: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Left => "esquerdo",
            Self::Right => "direito",
            Self::Middle => "do meio",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ScrollDirection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Up => "para cima",
            Self::Down => "para baixo",
            Self::Left => "para a esquerda",
            Self::Right => "para a direita",
        }
    }
}

/// Referência a uma imagem dentro do pacote `.stepeasy`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageRef {
    /// Caminho relativo dentro do zip, ex.: `images/step-0007.png`.
    pub path: String,
    /// Caminho da miniatura, ex.: `thumbs/step-0007.jpg`.
    #[serde(default)]
    pub thumb_path: Option<String>,
    pub width: u32,
    pub height: u32,
    /// Região da tela virtual que originou a imagem. É o que permite converter
    /// a posição do cursor para coordenadas locais mesmo depois de reordenar.
    pub source_rect: Rect,
}

/// Controle sob o cursor, obtido da API de acessibilidade da plataforma.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct UiTarget {
    /// Nome do controle ("Salvar", "Nome do arquivo:").
    #[serde(default)]
    pub name: Option<String>,
    /// Tipo do controle já traduzido ("botão", "campo de texto").
    #[serde(default)]
    pub control_type: Option<String>,
    /// Título da janela de nível superior.
    #[serde(default)]
    pub window_title: Option<String>,
    /// Executável dono da janela, ex.: `notepad.exe`.
    #[serde(default)]
    pub process_name: Option<String>,
    /// Retângulo do controle, para desenhar destaque no editor.
    #[serde(default)]
    pub bounds: Option<Rect>,
}

impl UiTarget {
    /// `true` quando não há nada de útil para montar uma legenda.
    pub fn is_blank(&self) -> bool {
        self.name.is_none() && self.control_type.is_none() && self.window_title.is_none()
    }
}

/// Marcação desenhada sobre a captura de um passo.
///
/// **As coordenadas são em pixels da própria imagem**, com origem no canto
/// superior esquerdo dela — e não no espaço de tela virtual como o resto do
/// modelo. É o que mantém a anotação grudada no que ela aponta mesmo que os
/// passos sejam reordenados ou que a imagem venha de outro monitor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Annotation {
    /// Contorno retangular, para cercar a região que importa.
    Rect {
        rect: Rect,
        color: [u8; 4],
        thickness: f32,
    },
    /// Seta apontando de `from` para `to`; a ponta fica em `to`.
    Arrow {
        from: Point,
        to: Point,
        color: [u8; 4],
        thickness: f32,
    },
    /// Texto com halo de contraste, para ficar legível sobre qualquer fundo.
    Text {
        at: Point,
        text: String,
        color: [u8; 4],
        size: f32,
    },
    /// Borra a região — é como se esconde senha, CPF e nome de cliente antes
    /// de mandar o tutorial para fora.
    Blur { rect: Rect, radius: f32 },
}

impl Annotation {
    /// Nome da ferramenta, para a lista do editor.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rect { .. } => "Retângulo",
            Self::Arrow { .. } => "Seta",
            Self::Text { .. } => "Texto",
            Self::Blur { .. } => "Borrão",
        }
    }

    /// Retângulo que a anotação ocupa, para acerto de clique e para mover.
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rect { rect, .. } | Self::Blur { rect, .. } => *rect,
            Self::Arrow { from, to, .. } => Rect::from_corners(*from, *to),
            Self::Text { at, text, size, .. } => {
                // Estimativa suficiente para acerto de clique: a medição exata
                // exigiria a fonte, que é assunto do módulo de renderização.
                let largura = (text.chars().count() as f32 * size * 0.55).max(*size);
                Rect::new(at.x, at.y, largura as u32, (size * 1.3) as u32)
            }
        }
    }

    /// Desloca a anotação inteira.
    pub fn translate(&mut self, dx: i32, dy: i32) {
        match self {
            Self::Rect { rect, .. } | Self::Blur { rect, .. } => {
                rect.x += dx;
                rect.y += dy;
            }
            Self::Arrow { from, to, .. } => {
                from.x += dx;
                from.y += dy;
                to.x += dx;
                to.y += dy;
            }
            Self::Text { at, .. } => {
                at.x += dx;
                at.y += dy;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reindex_numera_de_um() {
        let mut rec = Recording::new("Teste", CaptureScope::default());
        rec.steps.push(Step::manual("a"));
        rec.steps.push(Step::manual("b"));
        rec.reindex();
        assert_eq!(rec.steps[0].index, 1);
        assert_eq!(rec.steps[1].index, 2);
    }

    #[test]
    fn cursor_in_image_usa_source_rect() {
        let mut step = Step::new(StepKind::Click {
            button: MouseButton::Left,
        });
        step.cursor = Some(Point::new(-900, 300));
        step.image = Some(ImageRef {
            path: "images/step-0001.png".into(),
            thumb_path: None,
            width: 1920,
            height: 1080,
            source_rect: Rect::new(-1920, 0, 1920, 1080),
        });
        assert_eq!(step.cursor_in_image(), Some((1020, 300)));
    }

    #[test]
    fn proximo_indice_de_imagem_continua_de_onde_parou() {
        use crate::bundle::image_path;
        use crate::geometry::Rect;

        let mut rec = Recording::new("t", CaptureScope::default());
        assert_eq!(rec.next_image_index(), 1, "gravação vazia começa no 1");

        for numero in [1u32, 2, 7] {
            let mut step = Step::new(StepKind::Manual);
            step.image = Some(ImageRef {
                path: image_path(numero),
                thumb_path: None,
                width: 10,
                height: 10,
                source_rect: Rect::new(0, 0, 10, 10),
            });
            rec.steps.push(step);
        }
        // Passo sem imagem não atrapalha a conta.
        rec.steps.push(Step::manual("escrito à mão"));

        assert_eq!(rec.next_image_index(), 8);
    }

    #[test]
    fn slug_cai_para_timestamp_quando_titulo_e_vazio() {
        let rec = Recording::new("   ", CaptureScope::default());
        assert!(rec.slug().starts_with("gravacao-"));

        let rec = Recording::new("Como emitir NF-e!", CaptureScope::default());
        assert_eq!(rec.slug(), "como-emitir-nf-e");
    }
}
