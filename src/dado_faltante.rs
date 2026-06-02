use std::sync::LazyLock;

use crate::{normalizar, IdentificadorPadroes};

fn fuzzy(word: &str, tam_min: usize) -> String {
    let chars: Vec<char> = word.chars().collect();

    let mut variants = Vec::new();

    // Prefixos normais
    for i in tam_min..=chars.len() {
        variants.push(chars[..i].iter().collect::<String>());
    }

    // Prefixos só com consoantes (mantendo a primeira letra)
    let mut consonant_prefix = String::new();

    for (i, c) in chars.iter().enumerate() {
        if i == 0 || !"AEIOU".contains(*c) {
            consonant_prefix.push(*c);

            if consonant_prefix.chars().count() >= tam_min {
                variants.push(consonant_prefix.clone());
            }
        }
    }

    variants.sort();
    variants.dedup();

    variants.join("|")
}

pub fn criar_identificador_dado_faltante() -> IdentificadorPadroes {
    let mut identificador = IdentificadorPadroes::default();
    identificador.adicionar(&[
        r"^SI|NS|NI|NA$".to_string(),
        // format!(r"^N(A|AO|O)? ?({})$", fuzzy("SEI", 2).as_str()),
        // format!(r"^N(A|AO|O)? ?({})$", fuzzy("SABIDO", 2).as_str()),
        // format!(r"^N(A|AO|O)? ?({})$", fuzzy("SABE", 3).as_str()),
        // format!(r"^N(A|AO|O)? ?({})$", fuzzy("INFORMADO", 2).as_str()),
        // format!(r"^N(A|AO|O)? ?({})$", fuzzy("LEMBRA", 3).as_str()),
        // format!(r"^N(A|AO|O)? ?({})$", fuzzy("FORNECIDO", 3).as_str()),
        // format!(r"^S(E|EM)? ?({})$", fuzzy("INFORMACAO", 3).as_str()),
        // format!(r"^({})$", fuzzy("DESCONHECIDO", 3)),
    ]);

    identificador.padroes.iter().for_each(|x| println!("{}", x));

    identificador
}

static IDENTIFICADOR: LazyLock<IdentificadorPadroes> =
    LazyLock::new(criar_identificador_dado_faltante);

pub fn identificar_dado_faltante(valor: &str) -> bool {
    let identificador = &*IDENTIFICADOR;
    identificador.identificar(&normalizar(valor))
}

pub fn identificar_dado_faltante_normalizado(valor: &str) -> bool {
    let identificador = &*IDENTIFICADOR;
    identificador.identificar(valor)
}
