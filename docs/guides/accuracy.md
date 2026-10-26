# Medir o erro da nota prevista nas suas notas (local)

A nota prevista só vale se errar no máximo tanto quanto o MovieLens erra sobre você: **MAE 0,654** (calculado do campo `pred` dos logs do seu export; design §6.6). A medição esconde as suas notas mais recentes (20% por padrão), dobra as outras no modelo, prevê as escondidas e compara.

Roda só no seu computador: o export do MovieLens nunca vai para o CI nem para o git (`movielens-*.csv` está no `.gitignore`). No CI o mesmo cálculo roda em dados de fixture (`core/tests/accuracy.rs`).

## Pré-requisitos

- Os quatro CSVs do export na raiz do repositório (`movielens-ratings.csv`, `movielens-logs.csv`, `movielens-wishlist.csv`, `movielens-tags.csv`).
- Rust instalado (`cargo`).
- Para comparar com a base Letterboxd: o token do Kaggle em `~/.kaggle/access_token` (`docs/guides/kaggle.md`, passo 5), o CLI do Kaggle (`pip install kaggle`) e o catálogo publicado (`docs/guides/tmdb.md`).

## 1. Baixar os dados públicos

```bash
mkdir -p data && cd data
curl -fL -o ml-latest.zip https://web.archive.org/web/20260101230406id_/https://files.grouplens.org/datasets/movielens/ml-latest.zip
echo "66a9e518c747d76b241d9a859b001a2619d3ed1672ceef599eb50daf73a7b4a3  ml-latest.zip" | sha256sum -c -
unzip -o ml-latest.zip ml-latest/ratings.csv ml-latest/links.csv
kaggle datasets download tliocastro/letterboxd-sample-apr-2026 -f letterboxd_interactions.csv --unzip
curl -fsS -o letterboxd_slugs.csv -H "Accept: text/csv" --data-urlencode 'query=SELECT ?slug ?tmdb WHERE { ?item wdt:P6127 ?slug; wdt:P4947 ?tmdb }' https://query.wikidata.org/sparql
curl -fLO https://github.com/LucasJLBraz/cinelens/releases/download/catalog-v1/catalog.json.gz
cd ..
```

A pasta `data/` fica fora do git (`.gitignore`). O `ml-latest` vem da cópia do Internet Archive, conferida pelo hash: o certificado do site do GroupLens deixou de ser aceito em outubro de 2026 (raiz `emSign ECC Root CA - G3` removida dos navegadores e sistemas).

## 2. Treinar os dois modelos

Só MovieLens (a linha de base) e MovieLens + Letterboxd:

```bash
cargo run --release -p cinelens-train -- --ratings data/ml-latest/ratings.csv --links data/ml-latest/links.csv --out data/model-movielens.json.gz --data-version local-movielens
```

```bash
cargo run --release -p cinelens-train -- --ratings data/ml-latest/ratings.csv --links data/ml-latest/links.csv --letterboxd data/letterboxd_interactions.csv --letterboxd-slugs data/letterboxd_slugs.csv --catalog data/catalog.json.gz --out data/model-letterboxd.json.gz --data-version local-letterboxd
```

Sem o token do Kaggle no computador, o workflow **Model** treina os dois no GitHub sem publicar: **Run workflow** com `publish` desligado (e `letterboxd` ligado para o segundo), depois `gh run download <id> -n model -D data/<nome>`.

## 3. Medir

```bash
cargo run --release -p cinelens-train --bin cinelens-accuracy -- --model data/model-movielens.json.gz movielens-*.csv
```

```bash
cargo run --release -p cinelens-train --bin cinelens-accuracy -- --model data/model-letterboxd.json.gz movielens-*.csv
```

Cada uma imprime o MAE do Cinelens nas notas escondidas, o MAE do MovieLens nas mesmas notas (das previsões que ele registrou quando você deu cada nota) e a linha de base 0,654. `--held-out 0.1` muda a fração escondida.

## Como decidir

- A base Letterboxd fica só se o modelo com ela não tiver MAE maior que o modelo só MovieLens.
- O v1 só sai se o MAE do modelo publicado for ≤ 0,65.
- Registre os números e a decisão em `docs/adr/0002-letterboxd-in-training.md`.
