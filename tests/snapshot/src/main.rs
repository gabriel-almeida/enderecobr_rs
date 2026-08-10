use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::hash::Hash;
use std::time::Instant;

use clap::Parser;
use enderecobr_rs::{
    padronizar_bairros, padronizar_complementos, padronizar_estados_para_nome,
    padronizar_logradouros, padronizar_municipios, padronizar_numeros,
};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
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
//////////////////

trait SnapshotTester<T> {
    fn salvar_snapshot(&self, base_path: &str) -> Result<String, String>;
    fn comparar_snapshot(&self, base_path: &str) -> Result<(), String>;
    fn comparar_pareamentos(
        &self,
        base_path: &str,
        bruto: &[T],
        snapshot: &[T],
    ) -> Result<(), String>;
}

struct SnapshotTesterImpl<T, I, O>
where
    I: SerializadorSnapshot<T>,
    O: SerializadorSnapshot<T>,
{
    nome: &'static str,
    serializador_entrada: I,
    serializador_saida: O,
    processador: fn(&T) -> T,
}

impl<T, I, O> SnapshotTester<T> for SnapshotTesterImpl<T, I, O>
where
    I: SerializadorSnapshot<T>,
    O: SerializadorSnapshot<T>,
    T: PartialEq + Display + Eq + Hash + Clone,
{
    fn salvar_snapshot(&self, base_path: &str) -> Result<String, String> {
        let valores_brutos = self
            .serializador_entrada
            .carregar(base_path, self.nome, "bruto")?;
        let valores_processados: Vec<T> = valores_brutos
            .iter()
            .map(|x| (self.processador)(x))
            .collect();

        self.serializador_saida
            .salvar(base_path, self.nome, "snapshot", valores_processados)
    }

    fn comparar_snapshot(&self, base_path: &str) -> Result<(), String> {
        let valores_brutos = self
            .serializador_entrada
            .carregar(base_path, self.nome, "bruto")?;
        let valores_snapshot = self
            .serializador_saida
            .carregar(base_path, self.nome, "snapshot")?;

        assert_eq!(
            valores_snapshot.len(),
            valores_brutos.len(),
            "Os arquivos com os dados brutos e de snapshot têm tamanhos diferentes."
        );

        // Entrada e snapshot ambos vazios (condição já garantida pelo assert_eq
        // de tamanho acima): nada a comparar. Retornar aqui evita ainda a
        // divisão por zero em `duracao / valores_snapshot.len()` abaixo.
        if valores_snapshot.is_empty() {
            println!("Entrada e snapshot vazios; nada a comparar.");
            return Ok(());
        }

        let inicio = Instant::now();
        let res: Vec<_> = valores_brutos
            .iter()
            .zip(valores_snapshot.iter())
            .filter_map(|(bruto, snap)| {
                let atual = (self.processador)(bruto);
                if atual != *snap {
                    Some(Diff {
                        original: bruto.to_string(),
                        snapshot: snap.to_string(),
                        atual: atual.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect();

        let duracao = inicio.elapsed().as_nanos();
        let tempo_por_qtd = duracao / valores_snapshot.len() as u128;

        println!(
            "## Avaliação de {}: \n\n> Processado {} dados em {} ns ({} ns/reg => {:.0} reg/s)",
            self.nome,
            valores_snapshot.len(),
            duracao,
            tempo_por_qtd,
            (1f64 / tempo_por_qtd as f64) * 1_000_000_000f64
        );
        println!();

        if !res.is_empty() {
            println!("### Mudanças encontradas:");
            println!();
            println!("{:}", Table::new(res).with(Style::markdown()));
        } else {
            println!("Nenhuma mudança identificada.");
        }
        println!();

        self.comparar_pareamentos(base_path, &valores_brutos, &valores_snapshot)?;

        Ok(())
    }

    fn comparar_pareamentos(
        &self,
        base_path: &str,
        brutos: &[T],
        snapshot: &[T],
    ) -> Result<(), String> {
        let referencia_res = self
            .serializador_entrada
            .carregar(base_path, self.nome, "referencia");

        if referencia_res.is_err() {
            // Não existir referência é uma situação legítima
            return Ok(());
        }

        // Valores de Referência tratados com a versão atual e a anterior do pacote.
        let mut idx_ref_novas = HashMap::<String, HashSet<Comparacao>>::new();
        let mut idx_ref_antigas = HashMap::<String, HashSet<Comparacao>>::new();
        for r in referencia_res.unwrap() {
            let comparacao = Comparacao {
                versao_antiga: r.to_string(),
                versao_nova: (self.processador)(&r).to_string(),
            };
            idx_ref_novas
                .entry(comparacao.versao_nova.clone())
                .or_default()
                .insert(comparacao.clone());

            idx_ref_antigas
                .entry(comparacao.versao_antiga.clone())
                .or_default()
                .insert(comparacao.clone());
        }

        ////////

        // Valores processados com a versão antiga que batem com os valores de referência processados também com a versão antiga
        let pareados_antiga: HashSet<_> = snapshot
            .iter()
            .filter_map(|antiga| idx_ref_antigas.get(&antiga.to_string()))
            .flatten()
            .cloned()
            .collect();

        // Valores processados com a versão atual que batem com os valores de referência tratados
        // com a versão atual.
        let pareados_novos: HashSet<_> = brutos
            .iter()
            .filter_map(|bruto| {
                let processado = (self.processador)(bruto);
                idx_ref_novas.get(&processado.to_string())
            })
            .flatten()
            .cloned()
            .collect();

        ////////////

        // quem está pareado na versão antiga que não está pareado na versão nova
        let regressoes: Vec<_> = pareados_antiga
            .iter()
            .filter(|x| !pareados_novos.contains(*x))
            .collect();

        // quem está pareado na versão nova mas não está na versão antiga
        let melhorias = pareados_novos
            .iter()
            .filter(|x| !pareados_antiga.contains(*x))
            .count();

        ////////////

        println!("### Avaliação de pareamento:");
        println!(
            "> Pareamento de {} com base de referência: {}/{} total ({:.2}%)",
            self.nome,
            pareados_novos.len(),
            brutos.len(),
            pareados_novos.len() as f64 * 100f64 / brutos.len() as f64
        );

        println!(
            "> Foram pareados {} novos casos em relação a versão anterior ({}  na versão nova vs {} na versão antiga => aumento de {:.2}% absoluto)",
            melhorias,
            pareados_novos.len(),
            pareados_antiga.len(),
            (pareados_novos.len() as f64 - pareados_antiga.len() as f64) * 100f64 / brutos.len() as f64,
        );

        if !regressoes.is_empty() {
            println!();
            println!(
                "## Regressões do pareamento de {} da versão antiga (snapshot) em relação à versão atual ({}/{} casos => {:.2}%):",
                self.nome,
                regressoes.len(),
                brutos.len(),
                regressoes.len() as f64 * 100f64 / brutos.len() as f64
            );
            println!();
            println!(
                "{:}",
                Table::new(regressoes.iter().take(20)).with(Style::markdown())
            );
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

#[derive(Tabled, Hash, PartialEq, Eq, Clone)]
struct Comparacao {
    versao_antiga: String,
    versao_nova: String,
}
/////////////////
// Utilitários
////////////////

fn obter_snapshot_tester_dyn(nome_teste: &str) -> Box<dyn SnapshotTester<String>> {
    match nome_teste {
        "logr" => Box::new(SnapshotTesterImpl {
            nome: "logradouro",
            serializador_entrada: SerializadorString,
            serializador_saida: SerializadorString,
            processador: |x: &String| padronizar_logradouros(x),
        }),
        "num" => Box::new(SnapshotTesterImpl {
            nome: "numero",
            serializador_entrada: SerializadorString,
            serializador_saida: SerializadorString,
            processador: |x: &String| padronizar_numeros(x),
        }),
        "comp" => Box::new(SnapshotTesterImpl {
            nome: "complemento",
            serializador_entrada: SerializadorString,
            serializador_saida: SerializadorString,
            processador: |x: &String| padronizar_complementos(x),
        }),
        "loc" => Box::new(SnapshotTesterImpl {
            nome: "localidade",
            serializador_entrada: SerializadorString,
            serializador_saida: SerializadorString,
            processador: |x: &String| padronizar_bairros(x),
        }),
        "mun" => Box::new(SnapshotTesterImpl {
            nome: "municipio",
            serializador_entrada: SerializadorString,
            serializador_saida: SerializadorString,
            processador: |x: &String| padronizar_municipios(x),
        }),
        "uf" => Box::new(SnapshotTesterImpl {
            nome: "uf",
            serializador_entrada: SerializadorString,
            serializador_saida: SerializadorString,
            processador: |x: &String| padronizar_estados_para_nome(x).to_string(),
        }),
        _ => panic!("Nenhum teste encontrado"),
    }
}

////////////////

/// Utilitário que serve para comparar o resultado desta lib com valores
/// previamente salvos.
#[derive(Parser)]
#[clap(author, version)]
struct Args {
    /// Caminho Base
    caminho: String,

    /// Testes a serem realizados
    tipo_teste: Vec<String>,

    /// Salvar snapshot
    #[arg(short('s'), long, default_value = "false")]
    salvar: bool,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    let tipos_testes = if !args.tipo_teste.is_empty() {
        args.tipo_teste
    } else {
        ["logr", "num", "comp", "loc", "mun", "uf"]
            .iter()
            .map(|x| x.to_string())
            .collect()
    };

    for tipo_teste in tipos_testes {
        let tester = obter_snapshot_tester_dyn(&tipo_teste);
        if args.salvar {
            println!("Salvando snapshot para {}", tipo_teste);
            let arq = tester.salvar_snapshot(&args.caminho)?;
            println!("Snapshot salvo em {}", arq);
        } else {
            tester.comparar_snapshot(&args.caminho)?;
            println!();
        }
    }

    Ok(())
}
