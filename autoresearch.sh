#!/usr/bin/env bash
# autoresearch.sh — benchmark do modo autoresearch para o enderecobr_rs.
# Fase 1 (harness): roda o pareamento CNEFE e emite a métrica primária no formato
#   METRIC PAREAMENTO=<n>
# mais metadados livres em
#   ASI chave=valor
# Sai 0 no sucesso/improvement, !=0 em erro/regressão/sem-ganho (o loop descarta o run).
#
# Guardrail de regressão: compara o conjunto de linhas NÃO pareadas da rodada atual contra
# um baseline estável (1ª rodada, ou --reset-baseline). Se o candidato faz ALGUMA linha que
# o baseline pareava deixar de parear (regressão), o run é rejeitado (exit 1) — nunca se
# perde um pareamento já conquistado.
set -uo pipefail
export LC_ALL=C

CANONICOS=datasets/dados/cnefe/logradouro_canonicos.tsv
AMOSTRA=datasets/dados/cnefe/amostra_pareamento.tsv
REGRAS=data/regras/logradouro.tsv
BASE_DUMP=/tmp/ar_baseline_naopareados.tsv
BASE_PAREA=/tmp/ar_baseline_pareamento.txt
CAND_DUMP=/tmp/ar_cand_naopareados.tsv
REG_FILE=/tmp/ar_regressions.txt

# --reset-baseline: força re-estabelecer o baseline na próxima rodada
if [[ "${1:-}" == "--reset-baseline" ]]; then
  rm -f "$BASE_DUMP" "$BASE_PAREA"
fi

# Roda o benchmark (REGRAS = TSV atual, que inclui eventuais linhas candidata)
OUT=$(cargo run -q --release --bin avaliar-cnefe -- "$CANONICOS" "$AMOSTRA" "$REGRAS" --dump "$CAND_DUMP" 2>/dev/null)
PAREA=$(printf '%s\n' "$OUT" | grep -oE 'PAREAMENTO_AFTER=[0-9]+' | head -1 | cut -d= -f2)
TOTAL=$(printf '%s\n' "$OUT" | grep -oE 'TOTAL_AMOSTRA=[0-9]+' | head -1 | cut -d= -f2)

if [[ -z "$PAREA" || -z "$TOTAL" ]]; then
  echo "METRIC PAREAMENTO=0"
  echo "ASI error=benchmark_failed"
  exit 2
fi

# chave da linha não pareada = (uf|mun|logr_cru), colunas 1-3 do dump
keys_of() {
  awk -F'\t' 'NF>=3 {print $1"|"$2"|"$3}' "$1" | sort -u
}

if [[ ! -f "$BASE_DUMP" ]]; then
  cp "$CAND_DUMP" "$BASE_DUMP"
  printf '%s\n' "$PAREA" > "$BASE_PAREA"
  echo "METRIC PAREAMENTO=$PAREA"
  echo "ASI phase=baseline total=$TOTAL"
  exit 0
fi

BASE_PAREA_VAL=$(cat "$BASE_PAREA")
# regressão = linhas que o candidato NÃO pareia, mas o baseline pareava
comm -23 <(keys_of "$CAND_DUMP") <(keys_of "$BASE_DUMP") > "$REG_FILE"
REGRESSIONS=$(wc -l < "$REG_FILE")

echo "METRIC PAREAMENTO=$PAREA"
echo "ASI regressed=$REGRESSIONS baseline=$BASE_PAREA_VAL total=$TOTAL"

if [[ "$REGRESSIONS" -gt 0 ]]; then
  echo "ASI verdict=regression"
  exit 1
fi
if [[ "$PAREA" -le "$BASE_PAREA_VAL" ]]; then
  echo "ASI verdict=no_improvement"
  exit 1
fi
echo "ASI verdict=improvement"
exit 0
