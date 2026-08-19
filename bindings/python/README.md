## Fluxo de uso

> PS: Comandos no Makefile também.

```bash
uv sync # Sincronizar as dependências uv -> venv
uv run maturin develop # Compilar projeto
uv run pytest --cov # Rodar testes
uv run --extra doc mkdocs serve # Opcional: avaliar documentação.
```

## Atualizando o maturin e seu CI do Github Actions

```bash
uv tool update maturin

# Criando/atualizando o github actions
cd ../.. && maturin generate-ci -o .github/workflows/maturin.yml -m bindings/python/Cargo.toml github # Tem que rodar na raiz do projeto
```

> PS: Tem que mudar tanto o nome da action quanto o trecho `on` do início do `.github/workflows/maturin.yml` para manter o padrão do projeto. Basta desfazer a mudança nessas linhas via git.
