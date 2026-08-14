//! Autosave e recuperação.
//!
//! Uma gravação vive em memória até alguém salvar. Uma sessão longa que termina
//! em queda de energia ou em pânico do aplicativo levava tudo junto — este
//! módulo é o seguro contra isso.
//!
//! O rascunho é um `.stepeasy` normal, escrito numa pasta do perfil do usuário.
//! Ele **não** é o arquivo do usuário: salvar o rascunho não limpa as
//! alterações pendentes nem faz o projeto adotar aquele caminho.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use stepeasy_core::Project;

/// Intervalo mínimo entre dois rascunhos.
const INTERVALO: Duration = Duration::from_secs(45);

/// Estado do autosave de uma sessão.
pub struct Autosave {
    pasta: Option<PathBuf>,
    ultimo: Instant,
    /// Revisão do projeto no último rascunho escrito.
    revisao_salva: u64,
    /// Rascunho encontrado ao abrir, à espera de decisão do usuário.
    pub pendente: Option<PathBuf>,
}

impl Autosave {
    /// Prepara a pasta de rascunhos e procura sobras da sessão anterior.
    pub fn new() -> Self {
        let pasta = eframe::storage_dir("stepeasy").map(|dir| dir.join("recuperacao"));

        if let Some(pasta) = &pasta {
            if let Err(err) = std::fs::create_dir_all(pasta) {
                tracing::warn!("sem pasta de recuperação em {}: {err}", pasta.display());
            }
        }

        let pendente = pasta.as_deref().and_then(mais_recente);
        if let Some(p) = &pendente {
            tracing::info!("rascunho de recuperação encontrado: {}", p.display());
        }

        Self {
            pasta,
            ultimo: Instant::now(),
            revisao_salva: 0,
            pendente,
        }
    }

    /// Caminho do rascunho deste projeto.
    fn caminho(&self, project: &Project) -> Option<PathBuf> {
        self.pasta
            .as_ref()
            .map(|dir| dir.join(format!("{}.stepeasy", project.recording.id)))
    }

    /// Escreve o rascunho se já passou tempo suficiente e algo mudou.
    ///
    /// Chamado a cada quadro; a conta de revisão é o que evita reescrever
    /// dezenas de megabytes de imagens sem que nada tenha mudado.
    pub fn tick(&mut self, project: &mut Project) {
        if !project.is_dirty() || project.revision() == self.revisao_salva {
            return;
        }
        if self.ultimo.elapsed() < INTERVALO {
            return;
        }
        self.forcar(project);
    }

    /// Escreve o rascunho agora, sem esperar o intervalo.
    ///
    /// Usado logo depois de parar de gravar: é o momento de maior exposição,
    /// com muitos passos recém-capturados e nada no disco ainda.
    pub fn forcar(&mut self, project: &mut Project) {
        let Some(caminho) = self.caminho(project) else {
            return;
        };
        let revisao = project.revision();

        match project.save_copy_to(&caminho) {
            Ok(()) => {
                self.revisao_salva = revisao;
                self.ultimo = Instant::now();
                tracing::debug!("rascunho salvo em {}", caminho.display());
            }
            Err(err) => tracing::warn!("falha ao salvar rascunho: {err}"),
        }
    }

    /// Apaga o rascunho — o trabalho já está guardado onde o usuário quis.
    pub fn descartar(&mut self, project: &Project) {
        if let Some(caminho) = self.caminho(project) {
            if caminho.exists() {
                if let Err(err) = std::fs::remove_file(&caminho) {
                    tracing::warn!("não foi possível apagar o rascunho: {err}");
                }
            }
        }
        self.revisao_salva = project.revision();
    }

    /// Esquece o rascunho pendente sem apagar nada do projeto atual.
    pub fn descartar_pendente(&mut self) {
        if let Some(caminho) = self.pendente.take() {
            if let Err(err) = std::fs::remove_file(&caminho) {
                tracing::warn!("não foi possível apagar o rascunho pendente: {err}");
            }
        }
    }
}

impl Default for Autosave {
    fn default() -> Self {
        Self::new()
    }
}

/// Rascunho mais recente da pasta, se houver algum.
fn mais_recente(pasta: &Path) -> Option<PathBuf> {
    let entradas = std::fs::read_dir(pasta).ok()?;

    entradas
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "stepeasy"))
        .filter_map(|p| {
            let modificado = p.metadata().ok()?.modified().ok()?;
            Some((modificado, p))
        })
        .max_by_key(|(modificado, _)| *modificado)
        .map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mais_recente_ignora_outras_extensoes_e_pega_o_ultimo() {
        let dir = std::env::temp_dir().join(format!("stepeasy-rec-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("a.stepeasy"), b"velho").unwrap();
        std::fs::write(dir.join("lixo.txt"), b"nao conta").unwrap();
        // Garante ordem de modificação distinta entre os dois rascunhos.
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.join("b.stepeasy"), b"novo").unwrap();

        let achado = mais_recente(&dir).expect("deveria achar um rascunho");
        assert_eq!(achado.file_name().unwrap(), "b.stepeasy");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pasta_vazia_ou_inexistente_nao_quebra() {
        let vazia = std::env::temp_dir().join(format!("stepeasy-rec-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&vazia).unwrap();
        assert_eq!(mais_recente(&vazia), None);
        std::fs::remove_dir_all(&vazia).ok();

        assert_eq!(mais_recente(Path::new("nao/existe/mesmo")), None);
    }
}
