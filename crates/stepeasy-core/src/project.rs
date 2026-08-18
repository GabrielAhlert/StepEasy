//! Projeto aberto: a gravação em memória + o acesso às imagens.
//!
//! As imagens não ficam todas carregadas. Um projeto aberto de um `.stepeasy`
//! lê cada PNG do zip sob demanda e mantém um cache; um projeto recém-gravado
//! já tem os bytes em memória. As duas origens convivem no mesmo cache, que é
//! o que permite editar uma gravação antiga e acrescentar passos novos nela.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::bundle::{BundleReader, BundleWriter, EXTENSION};
use crate::error::{Error, Result};
use crate::model::Recording;

pub struct Project {
    pub recording: Recording,
    /// Arquivo de onde as imagens são lidas sob demanda.
    ///
    /// Nem sempre é o arquivo do usuário: ao recuperar um rascunho, ele aponta
    /// para dentro da pasta do aplicativo. Quem quer saber "onde este projeto
    /// está salvo" deve usar [`Project::path`], não este campo.
    origem: Option<PathBuf>,
    /// `true` quando `origem` é um rascunho interno, e não um arquivo escolhido
    /// pelo usuário.
    origem_temporaria: bool,
    /// `true` quando há alterações não salvas.
    dirty: bool,
    /// Conta quantas alterações o projeto sofreu.
    ///
    /// O autosave usa isto para não reescrever o mesmo conteúdo de minuto em
    /// minuto: `dirty` continua verdadeiro enquanto não se salva, mas a revisão
    /// só muda quando algo de fato mudou.
    revision: u64,
    /// Bytes já carregados ou ainda não gravados, por caminho interno.
    blobs: HashMap<String, Vec<u8>>,
}

impl Project {
    /// Projeto novo, ainda sem arquivo em disco.
    pub fn new(recording: Recording) -> Self {
        Self {
            recording,
            origem: None,
            origem_temporaria: false,
            dirty: true,
            revision: 1,
            blobs: HashMap::new(),
        }
    }

    /// Abre um `.stepeasy`. Só o manifesto é lido agora.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let recording = BundleReader::open(&path)?.recording()?;
        Ok(Self {
            recording,
            origem: Some(path),
            origem_temporaria: false,
            dirty: false,
            revision: 0,
            blobs: HashMap::new(),
        })
    }

    /// Arquivo do usuário, ou `None` se a gravação ainda não foi salva.
    ///
    /// Um rascunho recuperado responde `None` de propósito: ele **é** legível
    /// de um arquivo, mas não daquele que o usuário escolheu, e tratá-lo como
    /// destino salvaria o trabalho dentro da pasta do aplicativo.
    pub fn path(&self) -> Option<&Path> {
        if self.origem_temporaria {
            None
        } else {
            self.origem.as_deref()
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Marca a origem como rascunho interno.
    ///
    /// O caminho **continua valendo para ler as imagens** — é o que faz a
    /// recuperação funcionar, já que as capturas são carregadas sob demanda do
    /// zip. O que muda é o destino: o próximo salvamento pergunta onde guardar,
    /// em vez de escrever dentro da pasta do aplicativo.
    pub fn marcar_como_rascunho(&mut self) {
        self.origem_temporaria = true;
    }

    /// Número que muda a cada alteração do projeto.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Marca alterações pendentes. Chamado por quem edita a gravação.
    pub fn touch(&mut self) {
        self.dirty = true;
        self.revision += 1;
    }

    /// Registra os bytes de um arquivo interno (usado pelo gravador).
    pub fn put_blob(&mut self, name: impl Into<String>, bytes: Vec<u8>) {
        self.blobs.insert(name.into(), bytes);
        self.touch();
    }

    /// Bytes de um arquivo interno, carregando do zip na primeira vez.
    pub fn blob(&mut self, name: &str) -> Result<&[u8]> {
        if !self.blobs.contains_key(name) {
            let path = self
                .origem
                .as_ref()
                .ok_or_else(|| Error::MissingEntry(name.to_string()))?;
            let bytes = BundleReader::open(path)?.read(name)?;
            self.blobs.insert(name.to_string(), bytes);
        }
        Ok(&self.blobs[name])
    }

    /// Igual a [`Self::blob`], mas devolve `None` em vez de erro — conveniente
    /// para a UI, que não deve quebrar por causa de uma miniatura faltando.
    pub fn blob_opt(&mut self, name: &str) -> Option<&[u8]> {
        match self.blob(name) {
            Ok(_) => self.blobs.get(name).map(Vec::as_slice),
            Err(err) => {
                tracing::warn!("não foi possível ler {name}: {err}");
                None
            }
        }
    }

    /// Nome de arquivo sugerido para o "Salvar como".
    pub fn suggested_filename(&self) -> String {
        format!("{}.{EXTENSION}", self.recording.slug())
    }

    /// Salva no caminho já conhecido. Erro se o projeto nunca foi salvo.
    pub fn save(&mut self) -> Result<()> {
        let path = self.path().map(Path::to_path_buf).ok_or_else(|| {
            Error::other("o projeto ainda não tem um arquivo; use \"Salvar como\"")
        })?;
        self.save_as(path)
    }

    /// Salva em `path` e passa a apontar para ele.
    ///
    /// Escreve num arquivo temporário ao lado do destino e só então substitui,
    /// para que uma falha no meio do caminho não destrua a gravação anterior —
    /// que pode, inclusive, ser a origem das imagens que estamos copiando.
    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        self.write_to(&path)?;
        self.origem = Some(path);
        self.origem_temporaria = false;
        self.dirty = false;
        Ok(())
    }

    /// Escreve uma cópia em `path` **sem** passar a apontar para ela nem limpar
    /// as alterações pendentes.
    ///
    /// É o que o autosave usa: o rascunho de recuperação não é o arquivo do
    /// usuário, e salvá-lo não pode dar a impressão de que o trabalho já está
    /// guardado onde ele espera.
    pub fn save_copy_to(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.write_to(path.as_ref())
    }

    fn write_to(&mut self, path: &Path) -> Result<()> {
        // Materializa tudo o que o manifesto referencia antes de tocar no disco.
        let mut needed: Vec<String> = Vec::new();
        for step in &self.recording.steps {
            if let Some(image) = &step.image {
                needed.push(image.path.clone());
                if let Some(thumb) = &image.thumb_path {
                    needed.push(thumb.clone());
                }
            }
        }
        for name in &needed {
            self.blob(name)?;
        }

        self.recording.modified_at = Some(Utc::now());

        let tmp = path.with_extension(format!("{EXTENSION}.tmp"));
        {
            let mut writer = BundleWriter::create(&tmp)?;
            writer.write_manifest(&self.recording)?;
            for name in &needed {
                writer.write_blob(name, &self.blobs[name])?;
            }
            writer.finish()?;
        }
        // `rename` no Windows falha se o destino existe; remove antes.
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::image_path;
    use crate::geometry::Rect;
    use crate::model::{ImageRef, Step};
    use crate::scope::CaptureScope;

    fn projeto_com_imagem(dir: &Path) -> Project {
        let mut rec = Recording::new("Teste", CaptureScope::AllMonitors);
        let mut step = Step::manual("Passo 1");
        step.image = Some(ImageRef {
            path: image_path(1),
            thumb_path: None,
            width: 4,
            height: 4,
            source_rect: Rect::new(0, 0, 4, 4),
        });
        rec.steps.push(step);
        rec.reindex();

        let mut proj = Project::new(rec);
        proj.put_blob(image_path(1), b"conteudo-png".to_vec());
        let _ = dir;
        proj
    }

    #[test]
    fn salva_e_reabre_preservando_imagens() {
        let dir = std::env::temp_dir().join(format!("stepeasy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("teste.stepeasy");

        let mut proj = projeto_com_imagem(&dir);
        proj.save_as(&file).unwrap();
        assert!(!proj.is_dirty());

        let mut reaberto = Project::open(&file).unwrap();
        assert_eq!(reaberto.recording.steps.len(), 1);
        assert_eq!(reaberto.blob(&image_path(1)).unwrap(), b"conteudo-png");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn salvar_por_cima_do_proprio_arquivo_nao_perde_imagem() {
        let dir = std::env::temp_dir().join(format!("stepeasy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("teste.stepeasy");

        let mut proj = projeto_com_imagem(&dir);
        proj.save_as(&file).unwrap();

        // Reabre (imagem só no zip), edita e salva por cima.
        let mut reaberto = Project::open(&file).unwrap();
        reaberto.recording.title = "Outro título".into();
        reaberto.touch();
        reaberto.save().unwrap();

        let mut de_novo = Project::open(&file).unwrap();
        assert_eq!(de_novo.recording.title, "Outro título");
        assert_eq!(de_novo.blob(&image_path(1)).unwrap(), b"conteudo-png");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rascunho_recuperado_ainda_le_as_imagens() {
        // Regressao: a recuperacao zerava o caminho para forcar "Salvar como",
        // mas e desse caminho que as imagens sao lidas sob demanda. O resultado
        // era uma gravacao recuperada sem nenhuma captura, e um autosave que
        // falhava em seguida por nao conseguir materializar os blobs.
        let dir = std::env::temp_dir().join(format!("stepeasy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let rascunho = dir.join("rascunho.stepeasy");

        let mut original = projeto_com_imagem(&dir);
        original.save_copy_to(&rascunho).unwrap();

        // Abre o rascunho como a recuperacao faz.
        let mut recuperado = Project::open(&rascunho).unwrap();
        recuperado.marcar_como_rascunho();

        assert_eq!(
            recuperado.path(),
            None,
            "o rascunho nao pode ser oferecido como destino de salvamento"
        );
        assert_eq!(
            recuperado.blob(&image_path(1)).unwrap(),
            b"conteudo-png",
            "as imagens continuam vindo do rascunho"
        );

        // E salvar em outro lugar leva as imagens junto.
        let destino = dir.join("do-usuario.stepeasy");
        recuperado.save_as(&destino).unwrap();
        assert_eq!(recuperado.path(), Some(destino.as_path()));

        let mut relido = Project::open(&destino).unwrap();
        assert_eq!(relido.blob(&image_path(1)).unwrap(), b"conteudo-png");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn autosave_de_um_rascunho_recuperado_funciona() {
        // O sintoma que apareceu em uso: "falha ao salvar rascunho: arquivo
        // ausente no pacote".
        let dir = std::env::temp_dir().join(format!("stepeasy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut original = projeto_com_imagem(&dir);
        original
            .save_copy_to(dir.join("rascunho.stepeasy"))
            .unwrap();

        let mut recuperado = Project::open(dir.join("rascunho.stepeasy")).unwrap();
        recuperado.marcar_como_rascunho();
        recuperado.touch();

        recuperado
            .save_copy_to(dir.join("novo-rascunho.stepeasy"))
            .expect("o autosave precisa conseguir reescrever o rascunho");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_sem_caminho_da_erro() {
        let mut proj = Project::new(Recording::new("x", CaptureScope::default()));
        assert!(proj.save().is_err());
    }

    #[test]
    fn copia_nao_adota_o_caminho_nem_limpa_as_alteracoes() {
        let dir = std::env::temp_dir().join(format!("stepeasy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let rascunho = dir.join("rascunho.stepeasy");

        let mut proj = projeto_com_imagem(&dir);
        proj.save_copy_to(&rascunho).unwrap();

        assert!(rascunho.exists(), "a cópia deveria ter sido escrita");
        assert!(
            proj.is_dirty(),
            "salvar cópia não guarda o trabalho do usuário"
        );
        assert_eq!(proj.path(), None, "o projeto não deve adotar o rascunho");

        // E a cópia é um pacote válido.
        let mut lida = Project::open(&rascunho).unwrap();
        assert_eq!(lida.blob(&image_path(1)).unwrap(), b"conteudo-png");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn revisao_avanca_a_cada_alteracao_e_nao_ao_salvar() {
        let dir = std::env::temp_dir().join(format!("stepeasy-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();

        let mut proj = projeto_com_imagem(&dir);
        let antes = proj.revision();

        proj.touch();
        assert!(proj.revision() > antes, "editar precisa mudar a revisão");

        let depois_de_editar = proj.revision();
        proj.save_as(dir.join("t.stepeasy")).unwrap();
        assert_eq!(
            proj.revision(),
            depois_de_editar,
            "salvar não é alteração; senão o autosave reescreveria à toa"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
