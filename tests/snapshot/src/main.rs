use std::error::Error;
use std::rc::Rc;
use std::time::Instant;

use clap::Parser;
use enderecobr_rs::complemento::criar_padronizador_complemento;
use enderecobr_rs::logradouro::criar_padronizador_logradouros;
use enderecobr_rs::{
    padronizar_bairros, padronizar_complementos, padronizar_estados_para_nome,
    padronizar_logradouros, padronizar_municipios, padronizar_numeros, Padronizador,
};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use tabled::settings::Style;
use tabled::{Table, Tabled};

trait SerializadorSnapshot<T> {
    fn carregar(
        &self,
        base_path: &str,
        nome_teste: &str,
        etapa_teste: &str,
    ) -> Result<Vec<T>, String>;

    fn salvar(
        &self,
        base_path: &str,
        nome_teste: &str,
        etapa_teste: &str,
        valores: Vec<T>,
    ) -> Result<String, String>;
}

//////////////

#[derive(Default)]
struct SerializadorString;

impl SerializadorSnapshot<String> for SerializadorString {
    fn salvar(
        &self,
        base_path: &str,
        nome_teste: &str,
        etapa_teste: &str,
        valores: Vec<String>,
    ) -> Result<String, String> {
        let file_path = Path::new(base_path).join(format!("{}_{}.csv", nome_teste, etapa_teste));

        let mut file = File::create(&file_path).map_err(|e| {
            format!(
                "Erro ao salvar o arquivo {:}: {:}",
                file_path.to_str().unwrap_or(""),
                e
            )
        })?;
        for valor in valores {
            writeln!(file, "{}", valor).map_err(|e| e.to_string())?;
        }
        Ok(file_path
            .to_str()
            .ok_or("Caminho do arquivo inválido")?
            .to_string())
    }
    fn carregar(
        &self,
        base_path: &str,
        nome_teste: &str,
        etapa_teste: &str,
    ) -> Result<Vec<String>, String> {
        let file_path = Path::new(base_path).join(format!("{}_{}.csv", nome_teste, etapa_teste));

        let file = File::open(&file_path).map_err(|e| {
            format!(
                "Erro ao abrir arquivo {:}: {:}",
                file_path.to_str().unwrap_or(""),
                e
            )
        })?;

        Ok(BufReader::new(file)
            .lines()
            .map_while(|line| line.ok())
            .collect())
    }
}

///////////////////
// Classe principal
///////////////////

trait SnapshotTester {
    fn salvar_snapshot(&self, base_path: &str) -> Result<String, String>;
    fn comparar_snapshot(&self, base_path: &str) -> Result<(), String>;
}

struct SnapshotTesterImpl {
    nome: &'static str,
    processador_base: Box<dyn Fn(&str) -> String>,
    rules_pad: Option<Rc<Padronizador>>,
}

impl SnapshotTester for SnapshotTesterImpl {
    fn salvar_snapshot(&self, base_path: &str) -> Result<String, String> {
        let valores_brutos =
            SerializadorString::default().carregar(base_path, self.nome, "bruto")?;
        let valores_processados: Vec<String> = valores_brutos
            .iter()
            .map(|x| (self.processador_base)(x))
            .collect();

        SerializadorString::default().salvar(base_path, self.nome, "snapshot", valores_processados)
    }

    fn comparar_snapshot(&self, base_path: &str) -> Result<(), String> {
        let valores_brutos =
            SerializadorString::default().carregar(base_path, self.nome, "bruto")?;
        let valores_snapshot =
            SerializadorString::default().carregar(base_path, self.nome, "snapshot")?;

        assert_eq!(
            valores_snapshot.len(),
            valores_brutos.len(),
            "Os arquivos com os dados brutos e de snapshot têm tamanhos diferentes."
        );

        let inicio = Instant::now();
        let mut matched_before = 0usize;
        let mut matched_after = 0usize;
        let mut regressed = 0usize;
        let mut improved = 0usize;
        let mut diffs: Vec<Diff> = Vec::new();
        for (bruto, snap) in valores_brutos.iter().zip(valores_snapshot.iter()) {
            let out_base = (self.processador_base)(bruto);
            let out_rules = match &self.rules_pad {
                Some(p) => p.padronizar(bruto),
                None => out_base.clone(),
            };
            if out_base == *snap {
                matched_before += 1;
            }
            if out_rules == *snap {
                matched_after += 1;
            }
            if out_base == *snap && out_rules != *snap {
                regressed += 1;
            }
            if out_base != *snap && out_rules == *snap {
                improved += 1;
            }
            if out_rules != *snap {
                diffs.push(Diff {
                    original: bruto.clone(),
                    snapshot: snap.clone(),
                    atual: out_rules.clone(),
                });
            }
        }

        let duracao = inicio.elapsed().as_nanos();
        let tempo_por_qtd = duracao / valores_snapshot.len() as u128;
        let total = valores_snapshot.len();
        let mb = matched_before as f64 / total as f64;
        let ma = matched_after as f64 / total as f64;

        println!(
            "## Avaliação de {}: \n\n> Processado {} dados em {} ns ({} ns/reg => {:.0} reg/s)",
            self.nome,
            total,
            duracao,
            tempo_por_qtd,
            (1f64 / tempo_por_qtd as f64) * 1_000_000_000f64
        );
        println!(
            "SUMMARY campo={} TOTAL={} MATCH_BEFORE={:.4} MATCH_AFTER={:.4} REGRESSED={} IMPROVED={} CHANGED={}",
            self.nome, total, mb, ma, regressed, improved, diffs.len()
        );

        if !diffs.is_empty() {
            println!("{}", Table::new(diffs).with(Style::markdown()).to_string());
        } else {
            println!("Nenhuma mudança identificada.");
        }

        Ok(())
    }
}

#[derive(Tabled)]
struct Diff {
    original: String,
    snapshot: String,
    atual: String,
}

/////////////////
// Utilitários
////////////////

fn obter_base(tipo: &str) -> Result<Box<dyn Fn(&str) -> String>, String> {
    match tipo {
        "logr" => Ok(Box::new(padronizar_logradouros)),
        "num" => Ok(Box::new(padronizar_numeros)),
        "comp" => Ok(Box::new(padronizar_complementos)),
        "loc" => Ok(Box::new(padronizar_bairros)),
        "mun" => Ok(Box::new(padronizar_municipios)),
        "uf" => Ok(Box::new(|x: &str| {
            padronizar_estados_para_nome(x).to_string()
        })),
        _ => Err(format!("Nenhum teste encontrado para '{}'", tipo)),
    }
}

fn obter_snapshot_tester_dyn(
    tipo: &str,
    rules_pad: Option<Rc<Padronizador>>,
) -> Result<Box<dyn SnapshotTester>, String> {
    let nome = match tipo {
        "logr" => "logradouro",
        "num" => "numero",
        "comp" => "complemento",
        "loc" => "localidade",
        "mun" => "municipio",
        "uf" => "uf",
        _ => return Err(format!("Nenhum teste encontrado para '{}'", tipo)),
    };
    let base = obter_base(tipo)?;
    Ok(Box::new(SnapshotTesterImpl {
        nome,
        processador_base: base,
        rules_pad,
    }))
}

/// Carrega um `Padronizador` a partir de um TSV de regras, sem tocar no core da lib.
///
/// Colunas: `regex \t subst \t ignorar \t status \t campo \t razao \t autor \t fonte`.
/// Linhas com `status=dobrada` são ignoradas (já estão no core). A ordem do arquivo é a ordem
/// de aplicação; o seed via `--dump-base` preserva a ordem de inserção de `obter_pares()`.
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
/// Lê o campo (logr|comp) do TSV de regras a partir da coluna `campo`, ignorando linhas
/// com status=dobrada e cabeçalhos. Assim qualquer nome de arquivo pode ser usado.
fn campo_do_tsv(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 5 {
            continue;
        }
        let campo = cols[4];
        if campo == "logr" || campo == "comp" {
            return Ok(campo.to_string());
        }
    }
    Err(format!(
        "Não foi possível determinar o campo (logr|comp) no TSV de regras: {}",
        path.display()
    ))
}

/// Escreve o conjunto base de regras (`obter_pares`) em `data/regras/<campo>.tsv`, na ordem de
/// inserção. Espelha o `criar_padronizador_*` vivo (evita deriva).
fn dump_base(campo: &str) -> Result<(), Box<dyn Error>> {
    let (pad, campo_arquivo) = match campo {
        "logr" => (criar_padronizador_logradouros(), "logradouro"),
        "comp" => (criar_padronizador_complemento(), "complemento"),
        _ => {
            return Err(format!(
                "Campo '{}' não suporta --dump-base (use logr ou comp)",
                campo
            )
            .into())
        }
    };
    let pares = pad.obter_pares();
    let out_dir = Path::new("data/regras");
    std::fs::create_dir_all(out_dir)?;
    let out_path = out_dir.join(format!("{}.tsv", campo_arquivo));
    let mut w = BufWriter::new(File::create(&out_path)?);
    for (regex, subst, ignorar) in pares {
        let ignorar_s: &str = ignorar.unwrap_or("");
        writeln!(
            w,
            "{}\t{}\t{}\tbase\t{}\t\t\t",
            regex, subst, ignorar_s, campo
        )?;
    }
    println!("Base seed escrita em {}", out_path.display());
    Ok(())
}

/// Compara um arquivo de exemplos (`bruto \t esperado`) entre o processador base e o das regras.
fn avaliar_exemplos(
    path: &Path,
    base: &dyn Fn(&str) -> String,
    rules: Option<&Padronizador>,
) -> Result<(), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut total = 0usize;
    let mut fixed = 0usize;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }
        let bruto = cols[0];
        let esperado = cols[1];
        let out_base = base(bruto);
        let out_rules = match rules {
            Some(p) => p.padronizar(bruto),
            None => out_base.clone(),
        };
        total += 1;
        if out_base != esperado && out_rules == esperado {
            fixed += 1;
        }
    }
    println!("EXEMPLOS_TOTAL={} EXEMPLOS_FIXED={}", total, fixed);
    Ok(())
}

////////////////

/// Utilitário que serve para comparar o resultado desta lib com valores
/// previamente salvos.
#[derive(Parser)]
#[clap(author, version)]
struct Args {
    /// Caminho Base
    caminho: Option<String>,

    /// Testes a serem realizados
    tipo_teste: Vec<String>,

    /// Salvar snapshot
    #[arg(short('s'), long, default_value = "false")]
    salvar: bool,

    /// Semeia `data/regras/<campo>.tsv` com as regras base (logr|comp) e sai.
    #[arg(long)]
    dump_base: Option<String>,

    /// Aplica regras extras de um TSV (`data/regras/<campo>.tsv`) por cima das base.
    #[arg(long)]
    regras: Option<String>,

    /// Compara exemplos (`bruto \t esperado`) entre base e regras.
    #[arg(long)]
    exemplos: Option<String>,
}

fn main() -> Result<(), String> {
    let args = Args::parse();

    if let Some(campo) = &args.dump_base {
        return dump_base(campo).map_err(|e| e.to_string());
    }

    // Quando --regras é informado, o campo vem da própria coluna `campo` do TSV (logr|comp).
    let tipos_testes: Vec<String> = if let Some(regras) = &args.regras {
        vec![campo_do_tsv(Path::new(regras))?]
    } else if !args.tipo_teste.is_empty() {
        args.tipo_teste.clone()
    } else {
        ["logr", "num", "comp", "loc", "mun", "uf"]
            .iter()
            .map(|x| x.to_string())
            .collect()
    };

    let rules_pad: Option<Rc<Padronizador>> = match &args.regras {
        Some(p) => Some(Rc::new(
            carregar_padronizador_tsv(Path::new(p)).map_err(|e| e.to_string())?,
        )),
        None => None,
    };

    let base = args
        .caminho
        .clone()
        .ok_or_else(|| "Caminho base é obrigatório".to_string())?;

    for tipo_teste in &tipos_testes {
        let tester = obter_snapshot_tester_dyn(tipo_teste, rules_pad.clone())?;
        if args.salvar {
            println!("Salvando snapshot para {}", tipo_teste);
            let arq = tester.salvar_snapshot(&base)?;
            println!("Snapshot salvo em {}", arq);
        } else {
            tester.comparar_snapshot(&base)?;
            println!();
        }

        if let Some(ex) = &args.exemplos {
            let base_fn = obter_base(tipo_teste)?;
            avaliar_exemplos(Path::new(ex), &*base_fn, rules_pad.as_deref())?;
        }
    }

    Ok(())
}
