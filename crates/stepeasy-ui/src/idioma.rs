//! Escolha do idioma da interface.
//!
//! Os textos vivem em `locales/`, na raiz do repositório, e são compartilhados
//! com a `stepeasy-core` — trocar o idioma aqui muda também as legendas
//! geradas para os passos.
//!
//! Acrescentar um idioma **não passa por este arquivo**: basta o `.yml` novo em
//! `locales/`, que o `rust-i18n` compila junto e ele aparece sozinho no
//! seletor. É o que permite receber traduções sem tocar em Rust.

/// Idioma usado quando não há preferência salva e o sistema não ajuda.
pub const PADRAO: &str = "en";

/// Códigos disponíveis, em ordem alfabética, com o padrão sempre presente.
pub fn disponiveis() -> Vec<String> {
    let mut códigos: Vec<String> = rust_i18n::available_locales!()
        .into_iter()
        .map(|c| c.to_string())
        .collect();
    códigos.sort();
    if códigos.is_empty() {
        códigos.push(PADRAO.to_string());
    }
    códigos
}

/// Nome do idioma **na própria língua** — "Português (Brasil)", não
/// "Portuguese". Quem procura o próprio idioma numa lista procura pelo nome
/// que reconhece.
pub fn nome(codigo: &str) -> String {
    let nome = rust_i18n::t!("idioma.nome", locale = codigo).to_string();
    if nome.contains("idioma.nome") {
        codigo.to_string()
    } else {
        nome
    }
}

pub fn atual() -> String {
    rust_i18n::locale().to_string()
}

pub fn aplicar(codigo: &str) {
    rust_i18n::set_locale(codigo);
}

/// Escolhe o idioma na abertura: preferência salva, senão o do sistema, senão
/// o padrão.
pub fn inicial(salvo: Option<String>) -> String {
    let disponiveis = disponiveis();

    if let Some(salvo) = salvo {
        if disponiveis.contains(&salvo) {
            return salvo;
        }
    }
    if let Some(sistema) = do_sistema(&disponiveis) {
        return sistema;
    }
    PADRAO.to_string()
}

/// Casa o idioma do sistema com o que existe traduzido.
///
/// Aceita a correspondência exata (`pt-BR`) e também a da língua sem a região
/// (`pt-PT` cai em `pt-BR`): um português de Portugal entende bem melhor a
/// interface em português do Brasil do que em inglês.
fn do_sistema(disponiveis: &[String]) -> Option<String> {
    let bruto = idioma_do_sistema()?;
    escolher(&bruto, disponiveis)
}

pub(crate) fn escolher(bruto: &str, disponiveis: &[String]) -> Option<String> {
    let bruto = bruto.replace('_', "-");

    if let Some(exato) = disponiveis.iter().find(|d| d.eq_ignore_ascii_case(&bruto)) {
        return Some(exato.clone());
    }

    let lingua = bruto.split('-').next()?.to_ascii_lowercase();
    disponiveis
        .iter()
        .find(|d| {
            d.split('-')
                .next()
                .is_some_and(|l| l.eq_ignore_ascii_case(&lingua))
        })
        .cloned()
}

#[cfg(windows)]
fn idioma_do_sistema() -> Option<String> {
    use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

    // O Windows devolve algo como "pt-BR\0" num buffer de tamanho fixo.
    let mut buffer = [0u16; 85];
    let escritos = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if escritos <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..escritos as usize - 1]))
}

#[cfg(not(windows))]
fn idioma_do_sistema() -> Option<String> {
    // Convenção POSIX: LANGUAGE tem prioridade, e o valor vem como "pt_BR.UTF-8".
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(valor) = std::env::var(var) {
            let limpo = valor.split(['.', ':']).next().unwrap_or_default();
            if !limpo.is_empty() && limpo != "C" && limpo != "POSIX" {
                return Some(limpo.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lista() -> Vec<String> {
        vec!["en".to_string(), "pt-BR".to_string()]
    }

    #[test]
    fn correspondencia_exata() {
        assert_eq!(escolher("pt-BR", &lista()).as_deref(), Some("pt-BR"));
        assert_eq!(escolher("en", &lista()).as_deref(), Some("en"));
    }

    #[test]
    fn aceita_o_formato_posix_com_underline() {
        assert_eq!(escolher("pt_BR", &lista()).as_deref(), Some("pt-BR"));
    }

    #[test]
    fn cai_para_a_lingua_quando_a_regiao_nao_existe() {
        // Português de Portugal em português do Brasil é melhor que inglês.
        assert_eq!(escolher("pt-PT", &lista()).as_deref(), Some("pt-BR"));
        assert_eq!(escolher("en-GB", &lista()).as_deref(), Some("en"));
    }

    #[test]
    fn idioma_sem_traducao_nao_casa() {
        assert_eq!(escolher("ja-JP", &lista()), None);
    }

    #[test]
    fn preferencia_salva_vence_o_sistema() {
        assert_eq!(inicial(Some("pt-BR".into())), "pt-BR");
    }

    #[test]
    fn preferencia_invalida_e_ignorada() {
        // Um idioma removido do repositório não pode travar a abertura.
        let escolhido = inicial(Some("xx-YY".into()));
        assert!(disponiveis().contains(&escolhido));
    }

    #[test]
    fn os_dois_idiomas_do_projeto_estao_disponiveis() {
        let d = disponiveis();
        assert!(d.contains(&"en".to_string()), "{d:?}");
        assert!(d.contains(&"pt-BR".to_string()), "{d:?}");
    }

    #[test]
    fn cada_idioma_diz_o_proprio_nome() {
        assert_eq!(nome("pt-BR"), "Português (Brasil)");
        assert_eq!(nome("en"), "English");
    }
}
