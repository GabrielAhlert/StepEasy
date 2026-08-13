//! Operações de edição da gravação, com undo/redo.
//!
//! O histórico guarda cópias inteiras do [`Recording`]. Isso parece caro, mas
//! o `Recording` só tem metadados — as imagens vivem no [`crate::project`] e
//! nunca são clonadas. Uma gravação de 200 passos dá algumas centenas de KB,
//! e em troca ganhamos undo correto de graça, sem inverso por operação.

use uuid::Uuid;

use crate::caption;
use crate::model::{Recording, Step, StepKind};

/// Limite de níveis de desfazer.
const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone)]
struct Snapshot {
    label: String,
    recording: Recording,
}

/// Pilha de desfazer/refazer.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Executa `f` sobre a gravação registrando o estado anterior no histórico.
    ///
    /// `label` é o texto mostrado na UI ("Excluir passo", "Reordenar").
    pub fn edit<R>(
        &mut self,
        recording: &mut Recording,
        label: impl Into<String>,
        f: impl FnOnce(&mut Recording) -> R,
    ) -> R {
        self.undo.push(Snapshot {
            label: label.into(),
            recording: recording.clone(),
        });
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();

        let out = f(recording);
        recording.reindex();
        out
    }

    /// Rótulo da próxima operação a ser desfeita.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Desfaz, devolvendo o rótulo da operação desfeita.
    pub fn undo(&mut self, recording: &mut Recording) -> Option<String> {
        let snapshot = self.undo.pop()?;
        self.redo.push(Snapshot {
            label: snapshot.label.clone(),
            recording: std::mem::replace(recording, snapshot.recording),
        });
        Some(snapshot.label)
    }

    /// Refaz, devolvendo o rótulo da operação refeita.
    pub fn redo(&mut self, recording: &mut Recording) -> Option<String> {
        let snapshot = self.redo.pop()?;
        self.undo.push(Snapshot {
            label: snapshot.label.clone(),
            recording: std::mem::replace(recording, snapshot.recording),
        });
        Some(snapshot.label)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}

/// Move o passo da posição `from` para a posição `to`.
pub fn move_step(recording: &mut Recording, from: usize, to: usize) {
    if from >= recording.steps.len() || to >= recording.steps.len() || from == to {
        return;
    }
    let step = recording.steps.remove(from);
    recording.steps.insert(to, step);
}

/// Remove os passos indicados. Ids inexistentes são ignorados.
pub fn delete_steps(recording: &mut Recording, ids: &[Uuid]) {
    recording.steps.retain(|s| !ids.contains(&s.id));
}

/// Duplica um passo logo abaixo do original, com novo id.
pub fn duplicate_step(recording: &mut Recording, id: Uuid) -> Option<Uuid> {
    let pos = recording.position_of(id)?;
    let mut copy = recording.steps[pos].clone();
    copy.id = Uuid::new_v4();
    let new_id = copy.id;
    recording.steps.insert(pos + 1, copy);
    Some(new_id)
}

/// Insere um passo manual (sem imagem) na posição `at`.
pub fn insert_manual(recording: &mut Recording, at: usize, text: impl Into<String>) -> Uuid {
    let step = Step::manual(text);
    let id = step.id;
    let at = at.min(recording.steps.len());
    recording.steps.insert(at, step);
    id
}

/// Funde os passos indicados em um só, na posição do primeiro deles.
///
/// O passo resultante fica com a imagem e o alvo do primeiro (é o que o leitor
/// vê primeiro) e com as legendas de todos, uma por linha. Só faz sentido com
/// dois ou mais passos.
pub fn merge_steps(recording: &mut Recording, ids: &[Uuid]) -> Option<Uuid> {
    let mut positions: Vec<usize> = ids
        .iter()
        .filter_map(|id| recording.position_of(*id))
        .collect();
    positions.sort_unstable();
    positions.dedup();
    if positions.len() < 2 {
        return None;
    }

    let first = positions[0];
    let captions: Vec<String> = positions
        .iter()
        .map(|&i| {
            let step = &recording.steps[i];
            if step.caption.trim().is_empty() {
                caption::generate(step)
            } else {
                step.caption.clone()
            }
        })
        .filter(|c| !c.trim().is_empty())
        .collect();

    let count = positions.len() as u32;
    let merged_id = recording.steps[first].id;

    // Remove de trás para a frente para os índices não deslizarem.
    for &pos in positions.iter().skip(1).rev() {
        recording.steps.remove(pos);
    }

    let target = &mut recording.steps[first];
    target.kind = StepKind::Merged { count };
    target.caption = captions.join("\n");
    target.caption_edited = true;
    Some(merged_id)
}

/// Divide um passo mesclado de volta? Não — mesclagem é destrutiva e o caminho
/// de volta é o undo. Esta função existe só para dividir a **digitação** de um
/// passo `Type` em dois, no ponto `at` (em caracteres).
pub fn split_typing(recording: &mut Recording, id: Uuid, at: usize) -> Option<Uuid> {
    let pos = recording.position_of(id)?;
    let StepKind::Type { text } = recording.steps[pos].kind.clone() else {
        return None;
    };
    let chars: Vec<char> = text.chars().collect();
    if at == 0 || at >= chars.len() {
        return None;
    }
    let (head, tail): (String, String) = (
        chars[..at].iter().collect(),
        chars[at..].iter().collect(),
    );

    let mut second = recording.steps[pos].clone();
    second.id = Uuid::new_v4();
    second.kind = StepKind::Type { text: tail };
    second.caption_edited = false;
    caption::refresh(&mut second);
    let new_id = second.id;

    let first = &mut recording.steps[pos];
    first.kind = StepKind::Type { text: head };
    first.caption_edited = false;
    caption::refresh(first);

    recording.steps.insert(pos + 1, second);
    Some(new_id)
}

/// Edita a legenda marcando-a como manual.
pub fn set_caption(recording: &mut Recording, id: Uuid, text: impl Into<String>) {
    if let Some(step) = recording.step_by_id_mut(id) {
        step.caption = text.into();
        step.caption_edited = true;
    }
}

/// Volta a legenda para o texto gerado automaticamente.
pub fn reset_caption(recording: &mut Recording, id: Uuid) {
    if let Some(step) = recording.step_by_id_mut(id) {
        step.caption_edited = false;
        caption::refresh(step);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::CaptureScope;

    fn gravacao(n: usize) -> Recording {
        let mut rec = Recording::new("Teste", CaptureScope::default());
        for i in 1..=n {
            rec.steps.push(Step::manual(format!("Passo {i}")));
        }
        rec.reindex();
        rec
    }

    fn legendas(rec: &Recording) -> Vec<String> {
        rec.steps.iter().map(|s| s.caption.clone()).collect()
    }

    #[test]
    fn mover_reordena_e_renumera() {
        let mut rec = gravacao(5);
        let mut hist = History::new();
        hist.edit(&mut rec, "Reordenar", |r| move_step(r, 4, 1));

        assert_eq!(
            legendas(&rec),
            ["Passo 1", "Passo 5", "Passo 2", "Passo 3", "Passo 4"]
        );
        assert_eq!(rec.steps[1].index, 2);
    }

    #[test]
    fn undo_redo_percorre_varias_operacoes() {
        let mut rec = gravacao(5);
        let original = legendas(&rec);
        let mut hist = History::new();

        let quinto = rec.steps[4].id;
        hist.edit(&mut rec, "Reordenar", |r| move_step(r, 4, 1));
        hist.edit(&mut rec, "Excluir passo", |r| delete_steps(r, &[quinto]));
        let segundo = rec.steps[1].id;
        hist.edit(&mut rec, "Editar legenda", |r| {
            set_caption(r, segundo, "Novo texto")
        });

        assert_eq!(hist.undo_label(), Some("Editar legenda"));
        for _ in 0..3 {
            hist.undo(&mut rec);
        }
        assert_eq!(legendas(&rec), original);
        assert!(!hist.can_undo());

        for _ in 0..3 {
            hist.redo(&mut rec);
        }
        assert_eq!(rec.steps.len(), 4);
        assert_eq!(rec.steps[1].caption, "Novo texto");
    }

    #[test]
    fn nova_edicao_limpa_o_redo() {
        let mut rec = gravacao(3);
        let mut hist = History::new();
        hist.edit(&mut rec, "Reordenar", |r| move_step(r, 0, 2));
        hist.undo(&mut rec);
        assert!(hist.can_redo());

        hist.edit(&mut rec, "Inserir passo", |r| {
            insert_manual(r, 0, "Novo");
        });
        assert!(!hist.can_redo());
    }

    #[test]
    fn mesclar_junta_legendas_na_posicao_do_primeiro() {
        let mut rec = gravacao(4);
        let ids = vec![rec.steps[1].id, rec.steps[2].id];
        let mut hist = History::new();
        hist.edit(&mut rec, "Mesclar", |r| {
            merge_steps(r, &ids);
        });

        assert_eq!(rec.steps.len(), 3);
        assert_eq!(rec.steps[1].caption, "Passo 2\nPasso 3");
        assert!(matches!(rec.steps[1].kind, StepKind::Merged { count: 2 }));
        assert_eq!(legendas(&rec)[2], "Passo 4");
    }

    #[test]
    fn mesclar_um_passo_so_nao_faz_nada() {
        let mut rec = gravacao(3);
        let ids = vec![rec.steps[0].id];
        assert_eq!(merge_steps(&mut rec, &ids), None);
        assert_eq!(rec.steps.len(), 3);
    }

    #[test]
    fn dividir_digitacao() {
        let mut rec = Recording::new("t", CaptureScope::default());
        let mut step = Step::new(StepKind::Type {
            text: "relatorio.pdf".into(),
        });
        caption::refresh(&mut step);
        let id = step.id;
        rec.steps.push(step);
        rec.reindex();

        assert!(split_typing(&mut rec, id, 9).is_some());
        assert_eq!(rec.steps.len(), 2);
        assert_eq!(rec.steps[0].caption, "Digitou \"relatorio\"");
        assert_eq!(rec.steps[1].caption, "Digitou \".pdf\"");
    }

    #[test]
    fn reset_caption_volta_ao_automatico() {
        let mut rec = Recording::new("t", CaptureScope::default());
        let mut step = Step::new(StepKind::Key {
            combo: "Ctrl+S".into(),
        });
        caption::refresh(&mut step);
        let id = step.id;
        rec.steps.push(step);

        set_caption(&mut rec, id, "Salve tudo");
        assert!(rec.steps[0].caption_edited);
        reset_caption(&mut rec, id);
        assert_eq!(rec.steps[0].caption, "Pressionou Ctrl+S");
        assert!(!rec.steps[0].caption_edited);
    }
}
