# Checklist final da v1

Issue: #23 (`v1 release check`), filho de #1.

Use este roteiro para fechar a v1 sem esquecer nenhum portão do design.

## 1) Precisão no export do dono (registrada)

- [x] MAE no export do dono ≤ 0,65, com número registrado.
  - Resultado de referência: **0,428** no corte de 20% (58 notas) no modelo `ml-latest` only.
  - Fonte: `docs/adr/0002-letterboxd-in-training.md`, tabela de medições.

## 2) Velocidade manual no celular do dono

Roteiro curto (design §8), medindo com cronômetro no aparelho real:

1. Fechar o app e abrir de novo (app já com catálogo/modelo baixados): medir tempo até a tela pronta.
2. Trocar de aba e abrir uma página de filme.
3. Tocar num filtro da lista Explorar e esperar a grade atualizar.
4. Dar uma nota e esperar a lista recalcular.

Preencher e manter no git:

| Medida | Meta | Resultado |
|---|---|---|
| Abrir app pronto para uso | < 1 s | Aprovado pelo dono, sem cronômetro |
| Trocar aba / abrir página de filme | < 100 ms | Aprovado pelo dono, sem cronômetro |
| Trocar filtro | < 100 ms | Aprovado pelo dono, sem cronômetro |
| Depois de dar nota | < 200 ms | Aprovado pelo dono, sem cronômetro |

Avaliação do dono no próprio celular em 2026-10-31: sem latência perceptível, tudo imediato. Não há tempos medidos.

## 3) CI verde (testes + benchmarks)

- [x] Workflow CI rodando `cargo test -p cinelens-core -p cinelens-catalog -p cinelens-train`.
- [x] Workflow CI rodando `cargo bench -p cinelens-core` com budgets do design.
- Fonte: `.github/workflows/ci.yml`.

## 4) Release publicada (com licença/atribuição) e detectada pelo Obtainium

- [x] Workflow `Release` gera a release do app pelo tag `v*`.
- [x] Notas da release incluem licença/atribuição (MovieLens, TMDB, OMDb, JustWatch).
- [x] Repo público `https://github.com/LucasJLBraz/cinelens` usado no Obtainium.
- Fontes: `.github/workflows/release.yml`, `README.md`, `docs/guides/release.md`.

## 5) Guias de setup completos e linkados no README

- [x] README com links diretos para TMDB, OMDb, Google, Kaggle, release e precisão.
- [x] Fluxo de publicar/instalar por Obtainium documentado.

## 6) Decisão Letterboxd (migrada da #6)

- [x] Baseline MovieLens-only registrado no ADR (`MAE 0,428` no corte de 20%).
- [x] Modelo com Letterboxd treinado e medido com `cinelens-accuracy`.
- [x] Última linha do ADR preenchida com a decisão.
- [x] `KAGGLE_API_TOKEN` só entra no repositório se o MAE com Letterboxd não passar da baseline.
- Fontes: `docs/adr/0002-letterboxd-in-training.md`, `docs/guides/accuracy.md`, `.github/workflows/model.yml`.

## 7) Download do ml-latest pela cópia pinada do Internet Archive (com SHA-256)

- [x] Workflow `Catalog` baixa `ml-latest.zip` da URL pinada do Internet Archive e valida SHA-256.
- [x] Workflow `Model` faz o mesmo.
- [x] Comentário explicando o motivo (`emSign ECC Root CA - G3`) e quando reverter.
- Fontes: `.github/workflows/catalog.yml`, `.github/workflows/model.yml`.
