//! Pareamento municipio-a-cidade contra o CNEFE padronizado (v0.4.1).
//!
//! Lê o conjunto canônico CNEFE (já normalizado com `enderecobr_rs`) e uma amostra crua de
//! (uf, municipio, logradouro), normaliza a amostra e conta quantas linhas pareiam com o CNEFE:
//! - `PAREAMENTO_BEFORE`: com o `Padronizador` base (`criar_padronizador_logradouros`).
//! - `PAREAMENTO_AFTER`: com base + candidatas de um TSV de regras (`--regras`).
//! - `NOVOS_PAREAMENTOS`: linhas que passam a parear só com as candidatas.
//!
//! Sem `polars`/`parquet`: os parquets CNEFE são exportados para TSV via duckdb; este bin lê TSV.
//!
//! Modo descoberta: `--dump <arquivo>` escreve TODAS as linhas não pareadas como
//! `uf \t municipio \t logradouro_cru \t logradouro_normalizado` para análise de padrões.

use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use enderecobr_rs::logradouro::criar_padronizador_logradouros;
use enderecobr_rs::{padronizar_estados_para_sigla, padronizar_municipios, Padronizador};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "Uso: avaliar-cnefe <canonicos.tsv> <amostra.tsv> <regras.tsv> [--dump <nao_pareados.tsv>]\n\
             \n  canonicos.tsv : CNEFE (estado, municipio, logradouro) já normalizado\n\
             \n  amostra.tsv   : (uf, municipio, logradouro) cru\n\
             \n  regras.tsv    : TSV de regras (base + candidatas) do logradouro\n\
             \n  --dump <path> : opcional; escreve todas as linhas não pareadas (uf,mun,crú,normalizado)"
        );
        std::process::exit(2);
    }
    let canonicos = &args[1];
    let amostra = &args[2];
    let regras = &args[3];
    let mut dump_path: Option<String> = None;
    let mut j = 4;
    while j < args.len() {
        if args[j] == "--dump" && j + 1 < args.len() {
            dump_path = Some(args[j + 1].clone());
            j += 2;
        } else {
            j += 1;
        }
    }

    let set = carregar_canonico(Path::new(canonicos))?;
    let base_pad = criar_padronizador_logradouros();
    let rules_pad = carregar_padronizador_tsv(Path::new(regras))?;
    let mut dump_w: Option<BufWriter<File>> = match &dump_path {
        Some(p) => Some(BufWriter::new(File::create(p)?)),
        None => None,
    };

    let mut total: u64 = 0;
    let mut antes: u64 = 0;
    let mut depois: u64 = 0;
    let mut nao_pareados: Vec<(String, String, String)> = Vec::new();

    let file = File::open(amostra)?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        let uf = cols[0];
        let mun = cols[1];
        let logr = cols[2];
        if logr.trim().is_empty() {
            continue;
        }
        total += 1;

        let k_antes = chave(&base_pad, uf, mun, logr);
        let k_depois = chave(&rules_pad, uf, mun, logr);

        if set.contains(&k_antes) {
            antes += 1;
        }
        if set.contains(&k_depois) {
            depois += 1;
        } else {
            if nao_pareados.len() < 25 {
                nao_pareados.push((uf.to_string(), mun.to_string(), logr.to_string()));
            }
            if let Some(w) = dump_w.as_mut() {
                let logr_n = base_pad.padronizar(logr);
                writeln!(w, "{}\t{}\t{}\t{}", uf, mun, logr, logr_n)?;
            }
        }
    }

    let novos = depois.saturating_sub(antes);
    println!(
        "PAREAMENTO_BEFORE={} PAREAMENTO_AFTER={} NOVOS_PAREAMENTOS={} TOTAL_AMOSTRA={}",
        antes, depois, novos, total
    );
    if !nao_pareados.is_empty() {
        println!("NAO_PAREADOS_EXEMPLO (uf|municipio|logradouro cru):");
        for (uf, mun, logr) in &nao_pareados {
            println!("  {}|{}|{}", uf, mun, logr);
        }
    }

    Ok(())
}

/// Constrói a chave de pareamento normalizando (uf, municipio, logradouro) com a lib.
fn chave(pad: &Padronizador, uf: &str, mun: &str, logr: &str) -> String {
    let uf_n = padronizar_estados_para_sigla(uf);
    let mun_n = padronizar_municipios(mun);
    let logr_n = pad.padronizar(logr);
    format!("{}|{}|{}", uf_n, mun_n, logr_n)
}

/// Carrega o conjunto canônico CNEFE. Os valores (estado, municipio, logradouro) JÁ vêm
/// normalizados com o `enderecobr_rs`; usamos como estão (referência fiel).
fn carregar_canonico(path: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut set = HashSet::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 3 {
            continue;
        }
        set.insert(format!("{}|{}|{}", cols[0], cols[1], cols[2]));
    }
    Ok(set)
}

/// Carrega um `Padronizador` a partir de um TSV de regras, sem tocar no core da lib.
/// Colunas: `regex \t subst \t ignorar \t status \t campo \t razao \t autor \t fonte`.
/// Linhas com `status=dobrada` são ignoradas (já estão no core). A ordem do arquivo é a ordem
/// de aplicação.
fn carregar_padronizador_tsv(path: &Path) -> Result<Padronizador, Box<dyn Error>> {
    let file = File::open(path)?;
    let mut pad = Padronizador::default();
    let mut triples: Vec<Vec<Option<String>>> = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 4 {
            continue;
        }
        if cols[3] == "dobrada" {
            continue;
        }
        let regex = cols[0];
        let subst = cols[1];
        let ignorar = if cols[2].is_empty() {
            None
        } else {
            Some(cols[2].to_string())
        };
        triples.push(vec![
            Some(regex.to_string()),
            Some(subst.to_string()),
            ignorar,
        ]);
    }
    let refs: Vec<Vec<Option<&str>>> = triples
        .iter()
        .map(|t| t.iter().map(|o| o.as_deref()).collect())
        .collect();
    let slice: Vec<&[Option<&str>]> = refs.iter().map(|v| v.as_slice()).collect();
    pad.adicionar_pares(&slice);
    Ok(pad)
}
