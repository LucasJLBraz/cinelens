<div align="center">

# 🎬 Cinelens

**Pare de rolar catálogo. Descubra o que você vai gostar de assistir hoje.**

Recomendações de filmes feitas para o seu gosto, que rodam no seu Android, sem servidor e sem anúncios.

[**Baixar o APK**](https://github.com/LucasJLBraz/cinelens/releases/latest) · [Instalar pelo Obtainium](#instalar) · [Como funciona](#como-funciona)

<table>
  <tr>
    <td align="center"><img src="docs/prints/inicio.png" alt="Início com destaques e listas prontas" width="240"></td>
    <td align="center"><img src="docs/prints/explorar.png" alt="Explorar com busca, filtros e o controle Populares–Garimpo" width="240"></td>
    <td align="center"><img src="docs/prints/filme.png" alt="Página de filme com o motivo da recomendação e onde assistir" width="240"></td>
  </tr>
  <tr>
    <td align="center"><b>Início</b><br>O que ver hoje, já escolhido</td>
    <td align="center"><b>Explorar</b><br>Do popular ao garimpo</td>
    <td align="center"><b>Filme</b><br>Por que ver e onde ver</td>
  </tr>
</table>

</div>

## Por que o Cinelens

- **Entende o seu gosto.** Cada filme ganha uma nota prevista só para você, que se ajusta a cada nota que você dá. Medido nas notas mais recentes do dono do projeto, 38 filmes que o modelo conhece, o erro médio foi de 0,43 estrela ([medições](docs/adr/0002-letterboxd-in-training.md)).
- **Explica a recomendação.** "Porque você amou Monty Python em Busca do Cálice Sagrado": você sabe de onde veio cada sugestão.
- **Seus dados são seus.** As notas ficam no aparelho e, se você quiser, no seu próprio Google Drive. Não há servidor nem anúncios.
- **Abre rápido.** Início na tela em menos de 1 s num Galaxy S20 FE, e o dia a dia roda no próprio aparelho.

## O que dá para fazer

| | |
|---|---|
| 🎯 **Início** | Destaques do dia, listas prontas e novas sugestões ao puxar a tela |
| 🔎 **Explorar** | Mais de 91 mil filmes, com filtros e busca por título, direção ou elenco, que perdoa erro de digitação |
| 🎚️ **Populares ↔ Garimpo** | Um controle vai dos filmes que todo mundo viu às joias escondidas |
| ⚡ **Rodada rápida** | Dê notas em sequência e ensine seu gosto ao app em minutos |
| 📺 **Onde assistir** | Assinatura, aluguel ou compra no Brasil, e um filtro para o que está nos seus streamings |
| 🙈 **Sem spoiler de nota** | As notas ficam escondidas até você tocar, para não influenciar sua escolha |
| 🧭 **Parecidos, mas…** | Parte de um filme que você gostou e pede algo mais leve, mais curto ou mais recente |
| 📊 **Estatísticas** | O que suas notas dizem sobre você |
| 🔔 **Aviso semanal** | Avisa quando sai um lançamento com cara de Aposta forte para você |
| 🔁 **Sync e importação** | Traga seu histórico do MovieLens e sincronize aparelhos pelo Google Drive |
| 📤 **Compartilhar** | Envie um filme como card pelo Android |

## Instalar

1. Instale o [Obtainium](https://github.com/ImranR98/Obtainium).
2. Toque em **Adicionar app** e cole `https://github.com/LucasJLBraz/cinelens`.
3. Pronto: ele avisa e instala cada versão nova.

Prefere sem Obtainium? Baixe o APK em [Releases](https://github.com/LucasJLBraz/cinelens/releases/latest). O passo a passo está em [docs/guides/release.md](docs/guides/release.md#4-instalar-e-atualizar-pelo-obtainium).

Na primeira abertura o app baixa o catálogo, uma vez só. Depois, é só dar notas. Conta Google é opcional.

## Como funciona

- **Local-first** ([ADR 0001](docs/adr/0001-local-first-split-model.md)): catálogo, notas e modelo ficam no aparelho; o sync passa só pelo Google Drive do dono.
- **Catálogo:** montado todo dia no GitHub Actions a partir do TMDB e do OMDb e publicado como release (`catalog-v*`). O app baixa sozinho.
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

- [Checklist final da v1](docs/guides/v1-release-check.md)
- [TMDB](docs/guides/tmdb.md)
- [OMDb](docs/guides/omdb.md)
- [Google Drive sync](docs/guides/google.md)
- [Kaggle / Letterboxd](docs/guides/kaggle.md)
- [Publicar versão + Obtainium](docs/guides/release.md)
- [Medição de precisão](docs/guides/accuracy.md)

## Dados e licenças

- [MovieLens](https://grouplens.org/datasets/movielens/) `ml-latest`, GroupLens: F. Maxwell Harper e Joseph A. Konstan, "The MovieLens Datasets: History and Context", ACM TiiS 5(4), 2015. Uso não comercial.
- Metadados e imagens do [TMDB](https://www.themoviedb.org/). Este produto usa a API do TMDB, mas não é endossado nem certificado pelo TMDB.
- Onde assistir: dados do [JustWatch](https://www.justwatch.com/), via TMDB.
- Notas da crítica via [OMDb](https://www.omdbapi.com/).

O modelo publicado herda os termos não comerciais do MovieLens. O projeto é pessoal e não comercial.
