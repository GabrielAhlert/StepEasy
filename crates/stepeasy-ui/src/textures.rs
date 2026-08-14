//! Cache de texturas das capturas.
//!
//! A timeline mostra dezenas de miniaturas e o painel central mostra uma
//! imagem grande. Subir tudo isso para a GPU a cada quadro seria absurdo, então
//! cada caminho interno do pacote vira uma textura carregada uma única vez.

use std::collections::HashMap;

use egui::{ColorImage, TextureHandle, TextureOptions};

#[derive(Default)]
pub struct Textures {
    map: HashMap<String, TextureHandle>,
}

impl Textures {
    /// Devolve a textura de `key`, decodificando `bytes` na primeira vez.
    pub fn get_or_load(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        bytes: &[u8],
    ) -> Option<TextureHandle> {
        if let Some(handle) = self.map.get(key) {
            return Some(handle.clone());
        }
        let image = decode(bytes)?;
        let handle = ctx.load_texture(key, image, TextureOptions::LINEAR);
        self.map.insert(key.to_string(), handle.clone());
        Some(handle)
    }

    /// Devolve a textura de `key`, construindo a imagem sob demanda.
    ///
    /// Usado pela prévia do borrão, cuja imagem é calculada e não lida de um
    /// arquivo: `montar` só roda quando a chave ainda não está no cache.
    pub fn get_or_build(
        &mut self,
        ctx: &egui::Context,
        key: &str,
        montar: impl FnOnce() -> Option<ColorImage>,
    ) -> Option<TextureHandle> {
        if let Some(handle) = self.map.get(key) {
            return Some(handle.clone());
        }
        let handle = ctx.load_texture(key, montar()?, TextureOptions::LINEAR);
        self.map.insert(key.to_string(), handle.clone());
        Some(handle)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// Descarta tudo que começa com `prefix`.
    ///
    /// Cada ajuste de intensidade do borrão gera uma chave nova; sem isso o
    /// cache cresceria um item por posição do controle deslizante.
    pub fn forget_prefix(&mut self, prefix: &str) {
        self.map.retain(|key, _| !key.starts_with(prefix));
    }

    /// Descarta a textura de um caminho — usado quando o passo é excluído.
    pub fn forget(&mut self, key: &str) {
        self.map.remove(key);
    }

    /// Esvazia o cache (troca de projeto).
    pub fn clear(&mut self) {
        self.map.clear();
    }
}

fn decode(bytes: &[u8]) -> Option<ColorImage> {
    match image::load_from_memory(bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            Some(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
        }
        Err(err) => {
            tracing::warn!("imagem inválida no cache de texturas: {err}");
            None
        }
    }
}
