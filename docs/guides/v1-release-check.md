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

| Medida | Meta | Resultado no S20 FE |
|---|---|---|
| Abrir do zero até o Início com pôster | < 1 s | 0,71–0,78 s (3 aberturas, `v0.7.2`) |
| Voltar ao app já aberto | < 1 s | 69–73 ms (3 aberturas, `v0.7.0`, tempo do `am start`) |
| Trocar aba | < 100 ms | 74–121 ms, mediana 87 ms (10 trocas, `v0.7.0`) |
| Abrir página de filme | < 100 ms | conteúdo pronto em 76–106 ms (5 aberturas, `v0.7.0`) |
| Trocar filtro | < 100 ms | 87–114 ms, 3 de 6 acima (`v0.7.0`) |
| Depois de dar nota | < 200 ms | Não medido: o aparelho usa a conta real do dono |

Medido em 2026-11-01 em vídeo (`screenrecord`), do toque ou do início da abertura até a tela mudar. A gravação pode somar tempo. Na abertura do zero, o Início mostrado é o salvo da abertura anterior, e um toque espera a carga do catálogo terminar; esse tempo não foi medido. O dono aceitou os tempos e encerrou as otimizações. Ele também avaliou o uso no próprio celular em 2026-10-31: sem latência perceptível.

Atualização do catálogo em 2026-11-01 (`v0.7.2`): baixou o catálogo publicado, cerca de 31 MB, sem crash, com pico de memória amostrado de 490 MB; a abertura seguinte não baixou de novo. A primeira abertura depois da publicação não baixou, e a causa não foi apurada.

Prints: [Início](../prints/inicio.png), [Explorar](../prints/explorar.png), [página de filme](../prints/filme.png).

## 3) CI verde (testes + benchmarks)

- [x] Workflow CI rodando `cargo test -p cinelens-core -p cinelens-catalog -p cinelens-train`.
- [x] Workflow CI rodando `cargo bench -p cinelens-core` com budgets do design.
- Fonte: `.github/workflows/ci.yml`.

## 4) Release publicada (com licença/atribuição) e detectada pelo Obtainium

- [x] Workflow `Release` gera a release do app pelo tag `v*`.
- [x] Notas da release incluem licença/atribuição (MovieLens, TMDB, OMDb, JustWatch).
- [x] Repo público `https://github.com/LucasJLBraz/cinelens` usado no Obtainium.
- [ ] Obtainium detecta a versão publicada (confirmação do dono no aparelho).
- Fontes: `.github/workflows/release.yml`, `README.md`, `docs/guides/release.md`.

## 5) Guias de setup completos e linkados no README

- [x] README com links diretos para TMDB, OMDb, Google, Kaggle, release e precisão.
- [x] Fluxo de publicar/instalar por Obtainium documentado.

## 6) Decisão Letterboxd (migrada da #6)

- [x] Baseline MovieLens-only registrado no ADR (`MAE 0,428` no corte de 20%).
- [x] Modelo com Letterboxd treinado e medido com `cinelens-accuracy`.
- [x] Última linha do ADR preenchida com a decisão.
- [x] Letterboxd desligado: o secret `KAGGLE_API_TOKEN` existe, mas só uma execução manual do workflow **Model** usa a base.
- Fontes: `docs/adr/0002-letterboxd-in-training.md`, `docs/guides/accuracy.md`, `.github/workflows/model.yml`.

## 7) Download do ml-latest pela cópia pinada do Internet Archive (com SHA-256)

- [x] Workflow `Catalog` baixa `ml-latest.zip` da URL pinada do Internet Archive e valida SHA-256.
- [x] Workflow `Model` faz o mesmo.
- [x] Comentário explicando o motivo (`emSign ECC Root CA - G3`) e quando reverter.
- Fontes: `.github/workflows/catalog.yml`, `.github/workflows/model.yml`.
