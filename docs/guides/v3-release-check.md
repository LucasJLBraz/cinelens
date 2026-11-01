# Checklist final da v3

Issue: #68 (`v3 release check`), filho de #58.

Use este roteiro para fechar a v3. A meta é a mesma das anteriores: com animes no catálogo, o app continua imediato, e nada do que o dono já tem muda.

## 1) Testes e benchmarks

- [x] `cargo test -p cinelens-core -p cinelens-catalog -p cinelens-train` passa na branch `feat/series-catalogo`, rodado no computador do dono em 2026-11-01.
- [x] `cargo bench -p cinelens-core` fica dentro dos limites, com 91.000 filmes, 5.800 séries e 2.900 animes sintéticos, o tamanho publicado em 2026-11-01 (`core/benches/budgets.rs`). A tabela abaixo é anterior às otimizações e a tamanhos antigos; depois delas, a busca mede 79 ms e o Início 59 ms.

| Medida | Limite | Mediana |
|---|---|---|
| Abrir: carregar catálogo, séries, animes e modelos | 1 s | 382 ms |
| Nota em filme: recalcular e ordenar | 200 ms | 46 ms |
| Nota em série: recalcular e ordenar | 200 ms | 49 ms |
| Nota em anime: recalcular e ordenar | 200 ms | 46 ms |
| Trocar filtro | 100 ms | 68 ms |
| Busca com erro de digitação no catálogo todo | 100 ms | 98 ms |
| Abrir página de filme | 100 ms | 2 ms |
| Abrir página de série | 100 ms | 1 ms |
| Abrir página de anime | 100 ms | 1,2 ms |
| Início: destaques e todas as listas | 100 ms | 81 ms |
| Rodada rápida: próximas obras depois de uma nota | 100 ms | 28 ms |
| Perfil: todas as estatísticas | 100 ms | 0,6 ms |

- [x] Workflow CI verde no GitHub: busca 70 ms, Início 74 ms. Antes das otimizações, falhou nas duas medidas.

## 2) Dados do dono

- [x] Teste automático: com os animes e o modelo deles carregados, os dados do dono ficam idênticos, e uma série que vira anime mantém nota, Reação e marca (`core/tests/animes.rs`, módulo `atualizar`).
- [ ] No aparelho: as notas e marcas continuam iguais depois de atualizar da v2.

## 3) Publicar, nesta ordem

1. Fazer o merge de `feat/series-catalogo` na `main` e o push da `main`.
2. Rodar o workflow **Anime model** e conferir a release `anime-model-v1`.
3. Rodar o workflow **Catalog** na `main` e conferir `animes.json.gz` na release **Catálogo (formato 1)**. Ele precisa ter até 25% do tamanho do catálogo, e as temporadas precisam ter `estreia`: sem ela, a fileira Próxima temporada fica vazia.
4. Publicar a versão, conforme [release.md](release.md), passo 3:

```bash
git tag v3.0.0
```

```bash
git push origin v3.0.0
```

- [x] Merge e push da `main`.
- [x] `anime-model-v1` publicado: 986 KB.
- [x] `animes.json.gz` publicado: 2.867 animes, 1,3 MB, 4% do catálogo (limite de 25%); 2.852 têm `estreia` em alguma temporada.
- [ ] Release `v3.0.0` publicada e detectada pelo Obtainium.

## 4) Velocidade manual no celular do dono

Medir no aparelho real, com a `v3.0.0` e os animes já baixados. O roteiro é o da [v2](v2-release-check.md), mais a página de anime.

| Medida | Meta | Resultado |
|---|---|---|
| Abrir do zero até o Início com pôster | < 1 s | |
| Voltar ao app já aberto | < 1 s | |
| Trocar aba | < 100 ms | |
| Abrir página de anime | < 100 ms | |
| Busca com erro de digitação | < 100 ms | |

## 5) Atribuição

- [x] O README e o rodapé do app creditam o MyAnimeList.
