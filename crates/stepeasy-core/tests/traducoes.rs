//! Guarda das traduções.
//!
//! Uma tradução da comunidade envelhece sozinha: alguém acrescenta uma tela em
//! inglês, os outros idiomas ficam para trás, e meses depois metade do
//! aplicativo aparece em duas línguas ao mesmo tempo. Estes testes quebram o CI
//! no momento em que isso começa, que é a única hora em que ainda é barato
//! corrigir.
//!
//! `en.yml` é a referência: é o arquivo que o tradutor copia.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Idioma que serve de referência para todos os outros.
const REFERENCIA: &str = "en";

fn pasta_locales() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/stepeasy-core fica dois níveis abaixo da raiz")
        .join("locales")
}

/// Todos os idiomas encontrados, pelo nome do arquivo.
fn idiomas() -> BTreeMap<String, PathBuf> {
    std::fs::read_dir(pasta_locales())
        .expect("a pasta locales/ precisa existir")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "yml"))
        .filter_map(|p| {
            let nome = p.file_stem()?.to_str()?.to_string();
            Some((nome, p))
        })
        .collect()
}

/// Achata o YAML em chaves com ponto, como o `rust-i18n` faz internamente.
fn chaves(caminho: &Path) -> BTreeMap<String, String> {
    let bruto = std::fs::read_to_string(caminho).expect("arquivo de idioma legível");
    let raiz: serde_yml::Value = serde_yml::from_str(&bruto).expect("YAML válido");

    let mut saida = BTreeMap::new();
    achatar(String::new(), &raiz, &mut saida);
    // `_version` é metadado do formato, não texto para traduzir.
    saida.remove("_version");
    saida
}

fn achatar(prefixo: String, valor: &serde_yml::Value, saida: &mut BTreeMap<String, String>) {
    match valor {
        serde_yml::Value::Mapping(mapa) => {
            for (chave, v) in mapa {
                let chave = chave.as_str().unwrap_or_default();
                let completa = if prefixo.is_empty() {
                    chave.to_string()
                } else {
                    format!("{prefixo}.{chave}")
                };
                achatar(completa, v, saida);
            }
        }
        outro => {
            let texto = outro
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| format!("{outro:?}"));
            saida.insert(prefixo, texto);
        }
    }
}

/// Nomes entre `%{...}` que a frase espera receber.
fn parametros(texto: &str) -> BTreeSet<String> {
    let mut nomes = BTreeSet::new();
    let mut resto = texto;
    while let Some(inicio) = resto.find("%{") {
        let apos = &resto[inicio + 2..];
        let Some(fim) = apos.find('}') else { break };
        nomes.insert(apos[..fim].trim().to_string());
        resto = &apos[fim + 1..];
    }
    nomes
}

#[test]
fn existe_pelo_menos_o_idioma_de_referencia() {
    let idiomas = idiomas();
    assert!(
        idiomas.contains_key(REFERENCIA),
        "locales/{REFERENCIA}.yml não existe; é o arquivo que os tradutores copiam"
    );
}

#[test]
fn todo_idioma_tem_todas_as_chaves_da_referencia() {
    let idiomas = idiomas();
    let referencia = chaves(&idiomas[REFERENCIA]);

    for (nome, caminho) in &idiomas {
        if nome == REFERENCIA {
            continue;
        }
        let atual = chaves(caminho);

        let faltando: Vec<_> = referencia
            .keys()
            .filter(|k| !atual.contains_key(*k))
            .collect();
        assert!(
            faltando.is_empty(),
            "locales/{nome}.yml está sem {} chave(s): {faltando:?}\n\
             Copie-as de locales/{REFERENCIA}.yml e traduza.",
            faltando.len()
        );

        let sobrando: Vec<_> = atual
            .keys()
            .filter(|k| !referencia.contains_key(*k))
            .collect();
        assert!(
            sobrando.is_empty(),
            "locales/{nome}.yml tem chave(s) que não existem em {REFERENCIA}.yml: {sobrando:?}\n\
             Provavelmente foram renomeadas ou removidas do código.",
        );
    }
}

#[test]
fn nenhum_texto_ficou_vazio() {
    for (nome, caminho) in idiomas() {
        for (chave, texto) in chaves(&caminho) {
            assert!(
                !texto.trim().is_empty(),
                "locales/{nome}.yml: a chave {chave} está vazia"
            );
        }
    }
}

#[test]
fn os_parametros_batem_entre_os_idiomas() {
    // É o erro mais caro da tradução: trocar `%{n}` por `%{count}` compila,
    // passa no teste de chaves, e só aparece como um `%{n}` cru na tela do
    // usuário — em produção, no idioma que ninguém da equipe fala.
    let idiomas = idiomas();
    let referencia = chaves(&idiomas[REFERENCIA]);

    for (nome, caminho) in &idiomas {
        if nome == REFERENCIA {
            continue;
        }
        for (chave, texto) in chaves(caminho) {
            let Some(esperado) = referencia.get(&chave) else {
                continue;
            };
            let esperados = parametros(esperado);
            let obtidos = parametros(&texto);
            assert_eq!(
                obtidos, esperados,
                "locales/{nome}.yml, chave {chave}: os parâmetros não batem com {REFERENCIA}.yml.\n\
                 Esperado {esperados:?}, encontrado {obtidos:?}.\n\
                 O texto entre %{{}} é o nome de um valor que o programa insere — traduza em volta, nunca dentro."
            );
        }
    }
}

#[test]
fn o_extrator_de_parametros_funciona() {
    assert_eq!(parametros("sem nada"), BTreeSet::new());
    assert_eq!(
        parametros("Clicou %{alvo} da janela %{janela}"),
        ["alvo".to_string(), "janela".to_string()].into()
    );
    // Chave malformada não pode derrubar o teste inteiro.
    assert_eq!(parametros("%{sem fechar"), BTreeSet::new());
}
