//! Leitura e escrita do pacote `.stepeasy`.
//!
//! O pacote é um zip comum:
//!
//! ```text
//! manifest.json          Recording serializado
//! images/step-0001.png   captura em tamanho original
//! thumbs/step-0001.jpg   miniatura de 320 px para a timeline
//! ```
//!
//! PNG e JPEG já vêm comprimidos, então são armazenados sem deflate; só o
//! manifesto é comprimido.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{Error, Result};
use crate::model::{Recording, FORMAT_VERSION};

/// Nome do manifesto dentro do pacote.
pub const MANIFEST: &str = "manifest.json";
/// Extensão sem ponto.
pub const EXTENSION: &str = "stepeasy";

/// Caminho canônico da imagem de um passo, pelo índice 1-based.
pub fn image_path(index: u32) -> String {
    format!("images/step-{index:04}.png")
}

/// Caminho canônico da miniatura de um passo, pelo índice 1-based.
pub fn thumb_path(index: u32) -> String {
    format!("thumbs/step-{index:04}.jpg")
}

/// Extrai o número de um caminho gerado por [`image_path`] ou [`thumb_path`].
///
/// É o que permite continuar uma gravação sem sobrescrever as imagens que já
/// estão no pacote: basta continuar a numeração de onde ela parou.
pub fn index_from_path(path: &str) -> Option<u32> {
    let arquivo = path.rsplit('/').next()?;
    let resto = arquivo.strip_prefix("step-")?;
    let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();
    digitos.parse().ok()
}

/// Leitor de um pacote aberto. Mantém o zip aberto para carregar imagens sob
/// demanda em vez de trazer a gravação inteira para a memória.
pub struct BundleReader<R: Read + Seek> {
    archive: ZipArchive<R>,
}

impl BundleReader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        Self::new(BufReader::new(file))
    }
}

impl<R: Read + Seek> BundleReader<R> {
    pub fn new(reader: R) -> Result<Self> {
        Ok(Self {
            archive: ZipArchive::new(reader)?,
        })
    }

    /// Lê e valida o manifesto.
    pub fn recording(&mut self) -> Result<Recording> {
        let mut raw = String::new();
        self.archive
            .by_name(MANIFEST)
            .map_err(|_| Error::MissingManifest)?
            .read_to_string(&mut raw)?;

        let recording: Recording = serde_json::from_str(&raw)?;
        if recording.format_version > FORMAT_VERSION {
            return Err(Error::FutureFormat {
                found: recording.format_version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(recording)
    }

    /// Lê um arquivo qualquer do pacote pelo caminho relativo.
    pub fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut entry = self
            .archive
            .by_name(name)
            .map_err(|_| Error::MissingEntry(name.to_string()))?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        Ok(buf)
    }

    pub fn contains(&mut self, name: &str) -> bool {
        self.archive.by_name(name).is_ok()
    }

    /// Todos os caminhos contidos no pacote.
    pub fn entries(&self) -> Vec<String> {
        self.archive.file_names().map(str::to_string).collect()
    }
}

/// Escritor de pacote. O manifesto é sempre escrito primeiro para que leitores
/// que só querem os metadados não precisem varrer o zip inteiro.
pub struct BundleWriter<W: Write + Seek> {
    zip: ZipWriter<W>,
}

impl BundleWriter<BufWriter<File>> {
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = File::create(path.as_ref())?;
        Ok(Self::new(BufWriter::new(file)))
    }
}

impl<W: Write + Seek> BundleWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            zip: ZipWriter::new(writer),
        }
    }

    pub fn write_manifest(&mut self, recording: &Recording) -> Result<()> {
        let json = serde_json::to_vec_pretty(recording)?;
        self.zip.start_file(
            MANIFEST,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )?;
        self.zip.write_all(&json)?;
        Ok(())
    }

    /// Grava um binário já comprimido (PNG/JPEG) sem recomprimir.
    pub fn write_blob(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        self.zip.start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )?;
        self.zip.write_all(bytes)?;
        Ok(())
    }

    pub fn finish(self) -> Result<()> {
        self.zip.finish()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::model::Step;
    use crate::scope::CaptureScope;

    fn round_trip(recording: &Recording, blobs: &[(&str, &[u8])]) -> Result<Recording> {
        let mut buf = Vec::new();
        {
            let mut writer = BundleWriter::new(Cursor::new(&mut buf));
            writer.write_manifest(recording)?;
            for (name, bytes) in blobs {
                writer.write_blob(name, bytes)?;
            }
            writer.finish()?;
        }
        BundleReader::new(Cursor::new(buf))?.recording()
    }

    #[test]
    fn round_trip_preserva_gravacao() {
        let mut rec = Recording::new("Emitir nota", CaptureScope::ActiveWindow);
        rec.steps.push(Step::manual("Abra o sistema"));
        rec.steps.push(Step::manual("Clique em Emitir"));
        rec.reindex();

        let back = round_trip(&rec, &[("images/step-0001.png", b"fake-png")]).unwrap();
        assert_eq!(back, rec);
    }

    #[test]
    fn recusa_formato_futuro() {
        let mut rec = Recording::new("Do futuro", CaptureScope::default());
        rec.format_version = FORMAT_VERSION + 7;
        let err = round_trip(&rec, &[]).unwrap_err();
        assert!(matches!(err, Error::FutureFormat { .. }), "{err:?}");
    }

    #[test]
    fn manifesto_ausente_da_erro_claro() {
        let mut buf = Vec::new();
        {
            let mut writer = BundleWriter::new(Cursor::new(&mut buf));
            writer.write_blob("images/step-0001.png", b"x").unwrap();
            writer.finish().unwrap();
        }
        let err = BundleReader::new(Cursor::new(buf))
            .unwrap()
            .recording()
            .unwrap_err();
        assert!(matches!(err, Error::MissingManifest), "{err:?}");
    }

    #[test]
    fn caminhos_sao_zero_padded() {
        assert_eq!(image_path(7), "images/step-0007.png");
        assert_eq!(thumb_path(1234), "thumbs/step-1234.jpg");
    }

    #[test]
    fn indice_e_lido_de_volta_do_caminho() {
        assert_eq!(index_from_path(&image_path(7)), Some(7));
        assert_eq!(index_from_path(&thumb_path(1234)), Some(1234));
        assert_eq!(index_from_path("manifest.json"), None);
        assert_eq!(index_from_path("images/outro.png"), None);
    }
}
