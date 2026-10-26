# Cinelens

Um substituto pessoal do MovieLens para Android: escolher o que assistir pela nota prevista, dar notas, achar parecidos. Tudo roda no aparelho, sem servidor e sem esperar nada carregar.

## Instalar

Pelo [Obtainium](https://github.com/ImranR98/Obtainium): **Adicionar app** → `https://github.com/LucasJLBraz/cinelens`. Ele avisa e instala cada versão nova. O passo a passo está em [docs/guides/release.md](docs/guides/release.md#4-instalar-e-atualizar-pelo-obtainium).

Na primeira abertura o app baixa o catálogo, uma vez só. Dá para importar o histórico do MovieLens e entrar com o Google para sincronizar as notas entre aparelhos.

## Como funciona

- **Local-first** ([ADR 0001](docs/adr/0001-local-first-split-model.md)): catálogo, notas e modelo ficam no aparelho; o sync passa só pelo Google Drive do dono.
- **Catálogo:** montado todo dia no GitHub Actions a partir do TMDB e do OMDb e publicado como release (`catalog-v*`). O app baixa sozinho, em deltas.
- **Modelo:** fatores de filmes treinados no GitHub Actions com o MovieLens `ml-latest` e publicados como release (`model-v*`). O perfil do dono é recalculado no aparelho a cada nota, e as notas nunca saem do aparelho e do Drive dele.
- **Precisão:** a nota prevista precisa errar no máximo tanto quanto o MovieLens erra sobre o dono. As medições estão em [docs/guides/accuracy.md](docs/guides/accuracy.md).

O design completo está em [docs/superpowers/specs/2026-10-24-cinelens-design.md](docs/superpowers/specs/2026-10-24-cinelens-design.md), e os termos em [GLOSSARY.md](GLOSSARY.md).

## Código

| Pasta | O quê |
|---|---|
| `core/` | Núcleo em Rust: recomendação, filtros, listas, sync, importação do MovieLens |
| `src-tauri/` | App Tauri 2: downloads, arquivos, sync com o Drive, login Google |
| `ui/` | A interface, uma página HTML |
| `catalog/` | Job que monta o catálogo |
| `train/` | Job que treina o modelo, e `cinelens-accuracy` para medir a nota prevista |

```bash
cargo test -p cinelens-core -p cinelens-train -p cinelens-catalog
```

```bash
npm ci && npm run tauri -- android dev
```

Guias de configuração (chaves, secrets, publicar versão) em [docs/guides/](docs/guides/).

## Dados e licenças

- [MovieLens](https://grouplens.org/datasets/movielens/) `ml-latest`, GroupLens: F. Maxwell Harper e Joseph A. Konstan, "The MovieLens Datasets: History and Context", ACM TiiS 5(4), 2015. Uso não comercial.
- Metadados e imagens do [TMDB](https://www.themoviedb.org/). Este produto usa a API do TMDB, mas não é endossado nem certificado pelo TMDB.
- Onde assistir: dados do [JustWatch](https://www.justwatch.com/), via TMDB.
- Notas da crítica via [OMDb](https://www.omdbapi.com/).

O modelo publicado herda os termos não comerciais do MovieLens. O projeto é pessoal e não comercial.
