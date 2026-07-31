use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::LazyLock;

use crate::{metaphone::metaphone, normalizar, Padronizador};

static PADRONIZADOR: LazyLock<Padronizador> = LazyLock::new(criar_padronizador);

static MUNICIPIOS_MAP: LazyLock<FxHashMap<String, String>> = LazyLock::new(criar_municipio_map);

const ALIAS: &[(&str, &str)] = &[
    ("TRAJANO DE MORAIS", "TRAJANO DE MORAES"), // https://pt.wikipedia.org/wiki/Trajano_de_Moraes#cite_note-6
    ("POXOREO", "POXOREU"),                     // https://pt.wikipedia.org/wiki/Poxor%C3%A9u
    ("SERIDO", "SAO VICENTE DO SERIDO"), // https://pt.wikipedia.org/wiki/S%C3%A3o_Vicente_do_Serid%C3%B3#cite_note-7
    ("AUGUSTO SEVERO", "CAMPO GRANDE"), // https://pt.wikipedia.org/wiki/Campo_Grande_(Rio_Grande_do_Norte)
    ("FLORINIA", "FLORINEA"),           // https://pt.wikipedia.org/wiki/Flor%C3%ADnea
    ("FORTALEZA DO TABOCAO", "TABOCAO"), // https://pt.wikipedia.org/wiki/Taboc%C3%A3o
    ("SAO VALERIO DA NATIVIDADE", "SAO VALERIO"), // https://pt.wikipedia.org/wiki/S%C3%A3o_Val%C3%A9rio
    ("CAMPO DE SANTANA", "TACIMA"),               // https://pt.wikipedia.org/wiki/Tacima
    ("JANUARIO CICCO", "BOA SAUDE"),              // https://pt.wikipedia.org/wiki/Boa_Sa%C3%BAde
    ("SAO DOMINGOS DE POMBAL", "SAO DOMINGOS"), // https://pt.wikipedia.org/wiki/S%C3%A3o_Domingos_(Para%C3%ADba)
];

pub fn criar_padronizador() -> Padronizador {
    let mut padronizador = Padronizador::default();

    padronizador
        .adicionar(r"\b0+(\d+)\b", "$1") // Remove zeros na frente
        .adicionar(r"\s{2,}", " ") // Remove espaços extra
        // Remove qualquer carácter que não aparece na tabela de municípios
        // PS: a normalização já tira acentos
        .adicionar(r"[^ A-Z0-9'-]", "");

    padronizador.preparar();
    padronizador
}

fn padronizar_para_pareamento(mun: &str) -> String {
    // Ao chegar aqui, a string já deve ter passado pela etapa de padronização usual,
    // já sem acentos, em maiúscula e sem sinais de pontuação que não sejam
    // hífen ou aspas simples.

    const PREPOSICOES: &[&str] = &["DE", "DA", "DO", "DAS", "DOS", "DEL", "E"];

    // --- 1. Troca hífens por espaços ---
    let sem_hifen = mun.replace("-", " ");

    // --- 2. Remove letras duplicadas consecutivas (exceto SS e RR) ---
    let mut chars: Vec<char> = sem_hifen.chars().collect();
    chars.dedup_by(|a, b| *a == *b && *a != 'S' && *a != 'R');
    let dedup: String = chars.into_iter().collect();

    // --- 3. Remove preposições ---
    let mut resultado = String::with_capacity(dedup.len());
    for palavra in dedup.split_whitespace() {
        if !PREPOSICOES.contains(&palavra) {
            if !resultado.is_empty() {
                resultado.push(' ');
            }
            resultado.push_str(palavra);
        }
    }

    metaphone(&resultado)
}

fn nomes_alternativos(mun: &str) -> Vec<String> {
    let mut alternativas = FxHashSet::default();

    if mun.contains('\'') {
        alternativas.insert(mun.replace('\'', " "));
        alternativas.insert(mun.replace('\'', ""));
        for (i, _) in mun.match_indices("'") {
            let alternativa = format!(
                "{}{} {}",
                mun.get(..i).unwrap_or(""),
                mun.get(i + 1..i + 2).unwrap_or(""),
                mun.get(i + 1..).unwrap_or("")
            );
            alternativas.insert(padronizar_para_pareamento(&alternativa));
        }
    }

    alternativas.into_iter().sorted().collect()
}

pub fn criar_municipio_map() -> FxHashMap<String, String> {
    // a include_str! embute a string no código em tempo de compilação.
    let municipios_csv: &str = include_str!("data/municipios.csv");
    let mut mapa = FxHashMap::<String, String>::default();

    // Como eu quero varrer esses dados duas vezes,
    // mantenho ele processado em memória.
    let mut dados_municipios = Vec::<(String, String, String)>::default();
    for linha in municipios_csv.lines().skip(1) {
        let [codigo, nome, uf] = linha
            .split(',')
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| format!("É esperado 3 campos na linha: {}", linha))
            .unwrap();

        dados_municipios.push((
            codigo.to_owned(),
            normalizar(nome).to_string(),
            uf.to_owned(),
        ));
    }

    // Primeiro adiciono os casos canônicos na listagem.
    // Quero garantir que eles não vão conflitar com os casos derivados.
    for (codigo, nome, _) in &dados_municipios {
        // Preciso dos nome originais para validar os município
        // Lembrando que o nome já está normalizado.
        mapa.insert(nome.clone(), nome.clone());

        // Adiciona código do ibge no mapa
        mapa.insert(codigo.to_string(), nome.clone());

        // Erro comum: código IBGE sem o último dígito
        if let Some(codigo_reduzido) = codigo.get(..codigo.len() - 1) {
            mapa.insert(codigo_reduzido.to_string(), nome.clone());
        }
    }

    // Segunda varredura para adicionar os casos alternativos.
    for (_, nome, _) in &dados_municipios {
        // Adiciono uma versão do nome, com o pré processamento agressivo,
        // quando não existir essa versão no dicionário.
        mapa.entry(padronizar_para_pareamento(nome.as_str()))
            .or_insert(nome.clone());

        // Adiciono mais uma versão alternativa, dessa vez, expandido os nomes:
        // SANT'ANA => SANT ANA, SANTANA, SANTA ANA
        for alternativo in nomes_alternativos(nome.as_str()) {
            mapa.entry(padronizar_para_pareamento(&alternativo))
                .or_insert(nome.clone());
        }
    }

    // Por fim, adiciono os ALIAS quando eles ainda não existirem nos demais casos.
    for (de, para) in ALIAS {
        // Alias puro:
        mapa.entry(de.to_string()).or_insert(para.to_string());

        // Com a padronização agressiva.
        mapa.entry(padronizar_para_pareamento(de))
            .or_insert(para.to_string());

        // Com os nomes alternativos junto com a padronização agressiva.
        for alternativo in nomes_alternativos(de) {
            mapa.entry(padronizar_para_pareamento(&alternativo))
                .or_insert(para.to_string());
        }
    }

    mapa
}

// ====== Funções Públicas =======

/// Padroniza uma string representando município brasileiros.
///
/// ```
/// use enderecobr_rs::padronizar_municipios;
/// assert_eq!(padronizar_municipios("3304557"), "RIO DE JANEIRO");
/// assert_eq!(padronizar_municipios("003304557"), "RIO DE JANEIRO");
/// assert_eq!(padronizar_municipios("  3304557  "), "RIO DE JANEIRO");
/// assert_eq!(padronizar_municipios("RIO DE JANEIRO"), "RIO DE JANEIRO");
/// assert_eq!(padronizar_municipios("rio de janeiro"), "RIO DE JANEIRO");
/// assert_eq!(padronizar_municipios("SÃO PAULO"), "SAO PAULO");
/// assert_eq!(padronizar_municipios("PARATI"), "PARATY");
/// assert_eq!(padronizar_municipios("AUGUSTO SEVERO"), "CAMPO GRANDE");
/// assert_eq!(padronizar_municipios("SAO VALERIO DA NATIVIDADE"), "SAO VALERIO");
/// assert_eq!(padronizar_municipios(""), "");
/// assert_eq!(padronizar_municipios("BANANA"), "");
/// assert_eq!(padronizar_municipios("PARATI!!!!"), "PARATY");
/// assert_eq!(padronizar_municipios("!!!!"), "");
/// assert_eq!(padronizar_municipios("LAGOA DANTA"), "LAGOA D'ANTA");
///
/// ```
///
/// # Detalhes
/// Operações realizadas durante a padronização:
/// - remoção de espaços em branco antes e depois das strings e remoção de espaços em excesso entre palavras;
/// - conversão de caracteres para caixa alta;
/// - remoção de zeros à esquerda;
/// - busca, a partir do código numérico, do nome completo de cada município;
/// - remoção de acentos e caracteres não ASCII, correção de erros ortográficos frequentes e atualização
///   de nomes conforme listagem de municípios do IBGE de 2022.
///
/// Note que existe uma etapa de compilação das expressões regulares utilizadas,
/// logo a primeira execução desta função pode demorar um pouco a mais.
///
pub fn padronizar_municipios(valor: &str) -> String {
    let padronizador = &*PADRONIZADOR;
    let res = padronizador.padronizar(valor);

    let municipios = &*MUNICIPIOS_MAP;
    municipios
        .get(&res)
        .or_else(|| municipios.get(&padronizar_para_pareamento(&res)))
        .cloned()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padroniza_corretamente() {
        assert_eq!(padronizar_municipios("3304557"), "RIO DE JANEIRO");
        assert_eq!(padronizar_municipios("330455"), "RIO DE JANEIRO");
        assert_eq!(padronizar_municipios("03304557"), "RIO DE JANEIRO");
        assert_eq!(padronizar_municipios("0330455"), "RIO DE JANEIRO");
        assert_eq!(padronizar_municipios(" 3304557 "), "RIO DE JANEIRO");
        assert_eq!(padronizar_municipios("rio de janeiro"), "RIO DE JANEIRO");
        assert_eq!(padronizar_municipios(""), ""); // string vazia → string vazia
        assert_eq!(padronizar_municipios("SÃO PAULO"), "SAO PAULO");
        assert_eq!(padronizar_municipios("MOJI MIRIM"), "MOGI MIRIM");
        assert_eq!(padronizar_municipios("PARATI"), "PARATY");
        assert_eq!(padronizar_municipios("PARATI!!!!"), "PARATY");
        assert_eq!(padronizar_municipios("BANANA"), "");
        assert_eq!(padronizar_municipios("LAGOA D'ANTA"), "LAGOA D'ANTA");
        assert_eq!(padronizar_municipios("LAGOA DANTA"), "LAGOA D'ANTA");
        assert_eq!(padronizar_municipios("LAGOA D ANTA"), "LAGOA D'ANTA");
        assert_eq!(padronizar_municipios("LAGOA DA ANTA"), "LAGOA D'ANTA");
    }

    #[test]
    fn testa_nomes_alternativos() {
        assert_eq!(
            nomes_alternativos("LAGOA D'ANTA DE SANT'ANA"),
            vec![
                "LAGOA ANTA SANTANA",
                "LAGOA D ANTA DE SANT ANA",
                "LAGOA DANTA DE SANTANA",
                "LAGOA DANTA SANTA ANA"
            ]
        );

        // assert_eq!(100, criar_municipio_map().len());
    }

    #[test]
    fn padroniza_casos_especificos() {
        // Testa os casos específicos que estavam anteriormente como expressões regulares
        assert_eq!(padronizar_municipios("MOJI MIRIM"), "MOGI MIRIM");
        assert_eq!(padronizar_municipios("GRAO PARA"), "GRAO-PARA");
        assert_eq!(padronizar_municipios("BIRITIBA-MIRIM"), "BIRITIBA MIRIM");
        assert_eq!(
            padronizar_municipios("SAO LUIS DO PARAITINGA"),
            "SAO LUIZ DO PARAITINGA"
        );
        assert_eq!(
            padronizar_municipios("TRAJANO DE MORAIS"),
            "TRAJANO DE MORAES"
        );
        assert_eq!(padronizar_municipios("PARATI"), "PARATY");
        assert_eq!(
            padronizar_municipios("LAGOA DO ITAENGA"),
            "LAGOA DE ITAENGA"
        );
        assert_eq!(
            padronizar_municipios("ELDORADO DOS CARAJAS"),
            "ELDORADO DO CARAJAS"
        );
        assert_eq!(
            padronizar_municipios("SANTANA DO LIVRAMENTO"),
            "SANT'ANA DO LIVRAMENTO"
        );
        assert_eq!(
            padronizar_municipios("BELEM DE SAO FRANCISCO"),
            "BELEM DO SAO FRANCISCO"
        );
        assert_eq!(
            padronizar_municipios("SANTO ANTONIO DO LEVERGER"),
            "SANTO ANTONIO DE LEVERGER"
        );
        assert_eq!(padronizar_municipios("POXOREO"), "POXOREU");
        assert_eq!(
            padronizar_municipios("SAO THOME DAS LETRAS"),
            "SAO TOME DAS LETRAS"
        );
        assert_eq!(
            padronizar_municipios("OLHO-D'AGUA DO BORGES"),
            "OLHO D'AGUA DO BORGES"
        );
        assert_eq!(padronizar_municipios("ITAPAGE"), "ITAPAJE");
        assert_eq!(
            padronizar_municipios("MUQUEM DE SAO FRANCISCO"),
            "MUQUEM DO SAO FRANCISCO"
        );
        assert_eq!(padronizar_municipios("DONA EUSEBIA"), "DONA EUZEBIA");
        assert_eq!(padronizar_municipios("PASSA-VINTE"), "PASSA VINTE");
        assert_eq!(
            padronizar_municipios("AMPARO DE SAO FRANCISCO"),
            "AMPARO DO SAO FRANCISCO"
        );
        assert_eq!(padronizar_municipios("BRASOPOLIS"), "BRAZOPOLIS");
        assert_eq!(padronizar_municipios("SERIDO"), "SAO VICENTE DO SERIDO");
        assert_eq!(padronizar_municipios("IGUARACI"), "IGUARACY");
        assert_eq!(padronizar_municipios("AUGUSTO SEVERO"), "CAMPO GRANDE");
        assert_eq!(padronizar_municipios("FLORINIA"), "FLORINEA");
        assert_eq!(padronizar_municipios("FORTALEZA DO TABOCAO"), "TABOCAO");
        assert_eq!(
            padronizar_municipios("SAO VALERIO DA NATIVIDADE"),
            "SAO VALERIO"
        );
    }
}
