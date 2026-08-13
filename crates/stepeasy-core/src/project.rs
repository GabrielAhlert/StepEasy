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
    /// Caminho do `.stepeasy` em disco, se já foi salvo.
    path: Option<PathBuf>,
    /// `true` quando há alterações não salvas.
    dirty: bool,
    /// Bytes já carregados ou ainda não gravados, por caminho interno.
    blobs: HashMap<String, Vec<u8>>,
}

impl Project {
    /// Projeto novo, ainda sem arquivo em disco.
    pub fn new(recording: Recording) -> Self {
        Self {
            recording,
            path: None,
            dirty: true,
            blobs: HashMap::new(),
        }
    }

    /// Abre um `.stepeasy`. Só o manifesto é lido agora.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let recording = BundleReader::open(&path)?.recording()?;
        Ok(Self {
            recording,
            path: Some(path),
            dirty: false,
            blobs: HashMap::new(),
        })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Marca alterações pendentes. Chamado por quem edita a gravação.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Registra os bytes de um arquivo interno (usado pelo gravador).
    pub fn put_blob(&mut self, name: impl Into<String>, bytes: Vec<u8>) {
        self.blobs.insert(name.into(), bytes);
        self.dirty = true;
    }

    /// Bytes de um arquivo interno, carregando do zip na primeira vez.
    pub fn blob(&mut self, name: &str) -> Result<&[u8]> {
        if !self.blobs.contains_key(name) {
            let path = self
                .path
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
        let path = self
            .path
            .clone()
            .ok_or_else(|| Error::other("o projeto ainda não tem um arquivo; use \"Salvar como\""))?;
        self.save_as(path)
    }

    /// Salva em `path` e passa a apontar para ele.
    ///
    /// Escreve num arquivo temporário ao lado do destino e só então substitui,
    /// para que uma falha no meio do caminho não destrua a gravação anterior —
    /// que pode, inclusive, ser a origem das imagens que estamos copiando.
    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();

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
            std::fs::remove_file(&path)?;
        }
        std::fs::rename(&tmp, &path)?;

        self.path = Some(path);
        self.dirty = false;
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
    fn save_sem_caminho_da_erro() {
        let mut proj = Project::new(Recording::new("x", CaptureScope::default()));
        assert!(proj.save().is_err());
    }
}
