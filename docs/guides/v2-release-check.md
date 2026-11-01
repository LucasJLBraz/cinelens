# Checklist final da v2

Issue: #54 (`v2 release check`), filho de #40.

Use este roteiro para fechar a v2. A meta é a mesma da v1: com séries no catálogo, o app continua imediato.

## 1) Testes e benchmarks

- [x] `cargo test -p cinelens-core -p cinelens-catalog -p cinelens-train` passa na branch `feat/series-catalogo`, rodado no computador do dono em 2026-11-01.
- [x] `cargo bench -p cinelens-core` fica dentro de todos os limites, na mesma execução, com 60.000 filmes e 15.000 séries sintéticos (`core/benches/budgets.rs`). O catálogo publicado em 2026-11-01 tem cerca de 91.000 filmes e 5.786 séries; nesse tamanho, todas as medidas também ficam dentro do limite, e o Início leva 69 ms:

| Medida | Limite | Mediana |
|---|---|---|
| Abrir: carregar catálogo, séries e modelo | 1 s | 331 ms |
| Nota em filme: recalcular e ordenar | 200 ms | 35 ms |
| Nota em série: recalcular e ordenar | 200 ms | 38 ms |
| Trocar filtro | 100 ms | 48 ms |
| Busca com erro de digitação no catálogo todo | 100 ms | 83 ms |
| Abrir página de filme | 100 ms | 2 ms |
| Abrir página de série | 100 ms | 0,9 ms |
| Início: destaques e todas as listas | 100 ms | 49 ms |
| Rodada rápida: próximas obras depois de uma nota | 100 ms | 24 ms |
| Perfil: todas as estatísticas | 100 ms | 0,3 ms |

- [x] Workflow CI verde no GitHub em `eac41c5`: Início 77 ms, busca 67 ms. A execução anterior, `dcc0e3e`, falhou no Início (107 ms e 134 ms, limite de 100 ms).

## 2) Precisão em filmes

- [x] MAE no export do dono ≤ 0,65: **0,428** nas 58 notas mais recentes, 38 delas no modelo, com `--model data/published/model.json.gz` (modelo de 2026-10-25). O MovieLens errou 0,483 nas mesmas notas. Comando em [accuracy.md](accuracy.md), passo 3.

As notas em séries nunca mudam a nota prevista de um filme, e o modelo não mudou. Por isso o número é igual ao da v1.

## 3) Publicar, nesta ordem

1. Fazer o merge de `feat/series-catalogo` na `main` e o push da `main`.
2. Rodar o workflow **Catalog** na `main` e conferir que a release **Catálogo (formato 1)** tem `series.json.gz`.
3. Publicar a versão, conforme [release.md](release.md), passo 3:

```bash
git tag v2.0.0
```

```bash
git push origin v2.0.0
```

- [x] Merge e push da `main` (`dcc0e3e`).
- [x] `series.json.gz` publicado em 2026-11-01: 5.786 séries, 2,1 MB, 7% do tamanho do catálogo (limite de 25%).
- [x] Release `v2.0.0` publicada, com as notas de licença e atribuição. Instalada por cima da 1.0.0 no S20 FE, mantendo os dados.
- [ ] Obtainium detecta `v2.0.0`. Não confirmado: o app foi instalado por `adb`, e o Obtainium não está no aparelho medido.

## 4) Velocidade manual no celular do dono

Medir no aparelho real, com a `v2.0.0` e as séries já baixadas. O roteiro é o da [v1](v1-release-check.md), mais a página de série.

| Medida | Meta | Resultado no S20 FE |
|---|---|---|
| Abrir do zero até o Início com pôster | < 1 s | 643–829 ms (3 aberturas) |
| Voltar ao app já aberto | < 1 s | 61–74 ms (3 aberturas, tempo do `am start`) |
| Trocar aba | < 100 ms | 33–63 ms, mediana 49 ms (10 trocas) |
| Abrir página de filme | < 100 ms | 36–45 ms (5 aberturas); a primeira levou 494 ms até o último quadro, com o pôster |
| Abrir página de série | < 100 ms | 34–49 ms até o primeiro quadro, 56–102 ms até o último (5 aberturas) |
| Trocar filtro para Séries | < 100 ms | 34–42 ms (3 trocas) |
| Voltar o filtro para Tudo | < 100 ms | **108–112 ms, acima da meta** (3 trocas) |
| Filtro de gênero | < 100 ms | 73–120 ms, **2 de 4 acima** |
| Depois de dar nota em filme | < 200 ms | Não medido: o aparelho usa a conta real do dono |
| Depois de dar nota em série | < 200 ms | Não medido, pelo mesmo motivo |

Medido em 2026-11-01 na `v2.0.0`, com séries baixadas, sem crash. Toques por `adb`: do fim do toque no log do sistema (`ViewPostIme pointer 1`) ao primeiro quadro do app no SurfaceFlinger, o mesmo relógio. A abertura do zero vem de vídeo (`screenrecord`), da primeira à última mudança grande da tela. Métodos diferentes dos da v1, então os números não são diretamente comparáveis.

- [ ] Medidas fora da meta aceitas pelo dono, como na v1 (87–114 ms na troca de filtro).
- [ ] Nota medida, numa conta de teste.

## 5) README e guias

- [x] README fala de séries: o que o app faz, como o catálogo é montado e de onde vem a nota prevista.
- [x] Guias: [tmdb.md](tmdb.md) cobre a publicação de séries, e [accuracy.md](accuracy.md) cobre a medição.
