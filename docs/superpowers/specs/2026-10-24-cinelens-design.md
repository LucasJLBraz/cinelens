# Cinelens — design

Data: 2026-10-24. Termos em **negrito** seguem o [GLOSSARY.md](../../../GLOSSARY.md). Decisão de arquitetura: [ADR 0001](../../adr/0001-local-first-split-model.md).

## 1. Intenção

Substituto pessoal do MovieLens para um único usuário (o dono). O dono usa o MovieLens hoje para escolher o que assistir pela **nota prevista**, avaliar o que viu e, com muita frequência, buscar **parecidos** (422 buscas de "similar" nos logs de 2 anos). O problema central do MovieLens é **lentidão**: tudo depende do servidor deles.

Critério absoluto: **velocidade**. Precisão maior é bônus; a exigência é não ficar pior que o MovieLens.

Por quê: o dono quer que o app reflita seu gosto atual, sempre com lançamentos, em todos os seus aparelhos Android, sem esperar nada carregar.

## 2. Objetivos, não-objetivos, anti-objetivos

**Objetivos (v1)**
- Toda interação local e imediata (metas na §8).
- Nota prevista com erro médio absoluto (MAE) ≤ 0,65 nas notas do dono — o erro medido do próprio MovieLens sobre ele (§6.6).
- Catálogo atualizado com lançamentos sem ação manual.
- Progresso salvo na nuvem e **sync** entre aparelhos Android.
- Filtros de gênero e recomendação mais intuitivos que os do MovieLens.
- Estatísticas completas do perfil.

**Não-objetivos**
- Multiusuário, contas, comunidade, social (o Letterboxd é referência visual, não de funcionalidade).
- Tags próprias do dono.
- Servidor próprio ou custo recorrente.
- Sincronização contínua com o MovieLens (só **importação** única).
- Avaliação por temporada (séries, v2).

**Anti-objetivos (falha mesmo que funcione)**
- Qualquer tela com spinner perceptível no uso normal.
- Precisar ir atrás de atualização manualmente (do app ou do modelo). O dono declarou: se for manual, não usa.
- Duas telas com o mesmo papel.
- Previsão que ancora a nota do dono no momento de avaliar.

## 3. Restrições

- Usuário único, dono não é desenvolvedor: todo passo de configuração precisa de guia passo a passo.
- Plataforma: **Tauri 2** com núcleo em **Rust**. Android primeiro; Linux desktop depois, mesmas telas, sem funções exclusivas.
- Distribuição: GitHub Releases do repo **público** `LucasJLBraz/cinelens`; atualização no Android via **Obtainium** (checa em segundo plano e notifica).
- Nenhum dado pessoal nem chave de API no repositório. Chaves (TMDB, OMDb, cliente OAuth Google) ficam fora do git.
- Licenças de dados:
  - MovieLens `ml-latest` (GroupLens): pesquisa/não comercial, redistribuição de transformações sob os mesmos termos, citação obrigatória.
  - Letterboxd sample abr/2026 (Kaggle `tliocastro/letterboxd-sample-apr-2026`): CC BY-NC-SA 4.0.
  - TMDB: atribuição obrigatória; dados de onde assistir exigem crédito ao **JustWatch**.
  - O modelo publicado herda NC/SA; o projeto é não comercial.
- Google Drive: escopo `drive.appdata` (não sensível). O projeto OAuth precisa estar **"Em produção"** (em "Testing" o refresh token expira em 7 dias). Uso pessoal < 100 usuários dispensa verificação; o dono clica uma vez no aviso de app não verificado.

## 4. Arquitetura (decidida)

Local-first, sem backend (ADR 0001).

- **No aparelho:** catálogo, notas e marcações, modelo, cache de pôsteres. Toda leitura e filtro acontece localmente.
- **Sync:** arquivo(s) na `appDataFolder` do Google Drive do dono. Cada aparelho envia e recebe mudanças. Conflito: por item, vence a mudança mais recente. Vale para notas, **já vi**, **não me interessa**, **quero assistir**, **sou fã**, **o que pesou**, configurações.
- **Treino do modelo:** job agendado mensal no GitHub Actions. Publica os artefatos (fatores de filmes etc.) como asset de release. O app baixa sozinho quando há versão nova.
- **Perfil do dono:** recalculado no aparelho a cada nota (fold-in sobre fatores fixos dos filmes).
- **Catálogo (escolha minha):** montado no CI, não no aparelho. Um job diário do GitHub Actions, com as chaves TMDB e OMDb como secrets do repositório, gera o catálogo (metadados pt-BR, onde assistir BR, nota da crítica) e publica como asset de release: um arquivo completo + deltas. O aparelho só baixa arquivos, em segundo plano. Consequências: nenhuma chave de API dentro do APK público; limite de 1000/dia da OMDb gasto uma vez, não por aparelho; primeiro uso = um download. Pôsteres e trailers vêm direto das URLs públicas de imagem/vídeo (não exigem chave).
- **As notas do dono nunca saem do aparelho e do Drive dele (escolha minha).** O treino mensal no CI usa só bases públicas; não precisa das notas do dono, porque o fold-in no aparelho personaliza. Tudo que depende das notas do dono roda no aparelho: portão de precisão (§6.6), calibração do decaimento (§6.2).

## 5. Dados

### 5.1 Importação (única)
Arquivos do export do MovieLens, já presentes na raiz do repo local (não commitar — dados pessoais):
- `movielens-ratings.csv` — `movie_id, imdb_id, tmdb_id, rating, average_rating, title`. 289 notas, **sem data**.
- `movielens-logs.csv` — `datetime, login_id, action_type, log_json`. Eventos `rating` trazem `movieId`, `rating`, `action` e `pred` (previsão do MovieLens no momento). Cobre as 289 notas (set/2024 a out/2026). Usar para data de cada nota e para o histórico.
- `movielens-wishlist.csv` — 5 filmes → **quero assistir**.
- `movielens-tags.csv` — vazio; ignorar.
Reimportação manual possível.

### 5.2 Fontes externas
| Fonte | Uso |
|---|---|
| MovieLens `ml-latest` (33,8M notas, até jul/2023; `links.csv` → tmdbId/imdbId; tag genome) | Treino colaborativo; **tag genome** para **o que pesou** e **parecidos, mas…** |
| Letterboxd sample abr/2026 (14,7M notas, escala 0,5–5, IMDb `tconst` com coluna de confiança) | Treino colaborativo cobrindo 2023–abr/2026. Média 3,35 (mais baixa): modelar viés por fonte. Juntar via IMDb id; filtrar por confiança do match |
| TMDB API | Catálogo, metadados pt-BR, pôsteres, elenco, trailers, gêneros, onde assistir (região BR), em alta |
| OMDb API (chave gratuita, 1000/dia) | Nota da crítica (Metacritic/Rotten Tomatoes) para "Aclamados pela crítica" |

### 5.3 Catálogo (escolha minha)
Longas-metragens com mínimo de votos no TMDB + todos os lançamentos recentes + todo filme presente nas bases de treino ou nas notas do dono. Exclui curtas e cauda obscura para download inicial pequeno. Limiar exato: builder.

### 5.4 Histórico
Reavaliar troca a nota vigente; a anterior fica no histórico (alimenta "como seu gosto mudou").

## 6. Motor de recomendação

### 6.1 Colaborativo
Fatoração de matrizes treinada em `ml-latest` + Letterboxd 2026, com termo de viés por fonte. Fold-in do dono no aparelho.

### 6.2 Peso no tempo
Notas antigas pesam menos, decaimento suave. Velocidade calibrada automaticamente nos dados do dono (escolher a que melhor prevê as notas mais recentes). Configurável, incluindo "sem decaimento".

### 6.3 Filmes sem dados colaborativos
Filmes após abr/2026 (ou sem notas nas bases): mapear conteúdo (gêneros, diretor, elenco, palavras-chave, sinopse do TMDB) para o espaço de fatores, aprendido nos filmes que têm os dois. Aparecem com **confiança** menor.

### 6.4 Viés do dono
O dono só assiste o que espera gostar, então suas notas são altas. O modelo trabalha relativo à média do próprio dono (viés de usuário), nunca lê isso como "generoso". **Rodada rápida** e **não me interessa** trazem sinal fora da zona de conforto.

### 6.5 Controles e ajustes
- **Populares ↔ Garimpo:** peso da popularidade no ranking.
- **Parecidos:** item-item ("quem gostou deste também gostou").
- **Parecidos, mas…:** refinar parecidos numa direção (mais leve, mais curto, mais recente, melhor fotografia…), via tag genome. Técnica = "movie tuner" do MovieLens.
- **Sou fã** (diretor, ator, compositor): filmes da pessoa sobem no ranking.
- **O que pesou** (opcional, após dar nota): aspectos (fotografia, atuações, roteiro, trilha, ritmo, diálogos…) puxam recomendações para filmes fortes neles, via tag genome. Efeito mais fraco em filmes pós-2023 (genome parado). Escondível; nunca atrasa o ato de dar nota.
- **Não me interessa:** sinal negativo leve. **Já vi:** sem sinal de gosto.

### 6.6 Critério de precisão
Teste: esconder as notas mais recentes do dono, prever, comparar. Limite MAE ≤ 0,65 (MAE do MovieLens sobre o dono, calculado do campo `pred` dos logs: 0,654, RMSE 0,875). Bloqueia por ser pior, não exige ser melhor.

Onde roda (escolha minha):
- **No aparelho, a cada modelo mensal novo:** o app avalia o modelo baixado nas notas do dono antes de trocar. Se o MAE piorar em relação ao modelo atual ou passar de 0,65, mantém o modelo atual e registra o motivo (visível no Perfil). Nenhuma nota sai do aparelho.
- **Durante o desenvolvimento, localmente:** com os CSVs do export na máquina do dono, para decidir se a base Letterboxd fica e para validar a v1 antes do primeiro release.

## 7. Experiência

Tema escuro por padrão (escolha minha). Referências visuais: Nuvio (vitrine) e Letterboxd (grade), sem comunidade. Interface em português; títulos em português com o original abaixo; busca pelos dois.

### 7.1 Abas: Início · Explorar · Rodada · Perfil
- **Início** — vitrine, sem filtros. Topo: destaque deslizável, um filme por vez, com o porquê ("porque você amou X") e ações (detalhes, quero assistir, parecidos, **agora não**). Chips de atalho para listas prontas. Fileiras: "Em alta, do seu jeito" (em alta no TMDB reordenado pelo gosto do dono), Lançamentos para você, Joias escondidas etc. Todo "ver tudo ›" e todo chip abre o **Explorar** com aquele filtro marcado.
- **Explorar** — a única grade do catálogo. Abre sem filtro, ordenada por "Para você". Busca no topo. Três filtros visíveis (gênero, onde assistir, duração); resto em "+ filtros". Controle Populares ↔ Garimpo. Resultados em páginas curtas.
- **Rodada** — **rodada rápida**: pôster, nota ou "não vi", próximo já carregado. Ordem escolhida pelos filmes que mais informam o modelo.
- **Perfil** — estatísticas, histórico, configurações.

Protótipos aprovados (abrir com o servidor de brainstorm; fora do git): `.superpowers/brainstorm/*/content/home-v2-inicio-explorar.html`, `detalhe-filme.html`.

### 7.2 Filtros (Explorar)
- Gênero tri-estado: toque inclui, segundo toque exclui, terceiro limpa. Combinação dos incluídos: **E** por padrão, chave para **OU**. Gêneros do TMDB em português.
- Ano/década, duração, idioma original, país.
- Onde assistir no Brasil, só nos serviços que o dono assina (assinatura configurada no Perfil).
- Nota prevista mínima, confiança mínima.
- Esconder avaliados, já vi, quero assistir (e mostrar só estes, como filtro — não há telas separadas para essas listas).
- Atualização instantânea a cada toque.

### 7.3 Listas prontas (definidas pelo app)
| Lista | Regra |
|---|---|
| Sessão da Tarde | aventura/comédia/família, classificação livre ou 12, 1980–2010, populares |
| Aclamados pela crítica | nota alta de crítica (OMDb); sem OMDb vira "Aclamados pelo público" |
| Joias escondidas | previsão alta, pouco vistos |
| Cabe numa noite | < 1h40 |
| Lançamentos para você | últimos 6 meses, previsão alta |
| Clássicos que você não viu | antes de 1980, muito bem avaliados pelo público |
| No seu streaming hoje | só nos serviços do dono |
| Para rever | já vi + previsão alta |
Mais listas podem vir depois.

### 7.4 Previsão: exibição
Configuração "Mostrar previsão como":
- **Estrelas** — decimal (4.2★) + meia estrela mais provável + confiança.
- **Faixa** (padrão) — rótulos ordinais relativos ao gosto do dono: Aposta forte (top ~10% para ele), Boa chance, Arriscado, Provavelmente não. Sem número 0–5.
- **Oculto** — só a ordem.
Na tela de avaliar a previsão fica sempre escondida (Cosley et al., CHI 2003: a previsão exibida puxa a nota). Após avaliar, revelação no modo escolhido ("o app achava Aposta forte, 4.2"), registrada para a estatística de acerto.
**Marcado: precisa de refinamento.** O dono aceitou como caminho inicial e espera algo melhor; tratar como ponto de iteração.

### 7.5 Página do filme
De cima para baixo: fundo e pôster; título pt-BR e original; ano, duração, gêneros, idioma; previsão (modo escolhido) com confiança e porquê; onde assistir (BR, com crédito JustWatch); ações (dar nota, quero assistir, já vi, ⋯ com não me interessa, histórico de notas, abrir no TMDB); sinopse; **Parecidos** + "Parecidos, mas…"; bloco "o que pesou?" após avaliar; direção (toque = filmografia ordenada pelo gosto; marcar sou fã); elenco (idem); trailer.

### 7.6 Ações sobre um filme
| Ação | Efeito |
|---|---|
| Dar nota (0,5–5, meia estrela) | treina o perfil, recalcula na hora |
| Já vi | some das recomendações, sem sinal de gosto; elegível a Para rever |
| Agora não (destaque do Início) | some do destaque por algumas semanas |
| Não me interessa | some para sempre, sinal negativo leve |
| Quero assistir | entra na lista |

### 7.7 Estatísticas (Perfil)
Todas na v1: distribuição das notas; notas por gênero, década e diretor; filmes avaliados por mês; o quanto o dono é mais duro ou generoso que a média; filmes em que mais discorda do público; como o gosto mudou ao longo do tempo; quanto o app acerta o dono (previsão revelada vs nota dada).

### 7.8 Notificações
Semanal no máximo: lançamento com previsão alta ("Saiu X, Aposta forte").

## 8. Velocidade (critério absoluto)

Cada meta vira teste automatizado que reprova a build se regredir:
- Abrir o app pronto para uso: < 1 s.
- Trocar aba, abrir página do filme: < 100 ms.
- Tocar filtro → grade atualizada: < 100 ms.
- Dar nota → recomendações recalculadas: < 200 ms.
- Pôsteres em cache local após a primeira vez; rolar nunca mostra vazio.
Exceção aceita: primeiro uso (download de catálogo e modelo), uma vez.

Como medir (escolha minha):
- **No CI:** benchmarks em Rust das peças que dominam o tempo (consulta de filtro, ranking, fold-in após nota, leitura do catálogo na abertura), com orçamento derivado das metas acima. Regressão reprova a build. Emuladores no CI não são confiáveis para tempo de tela, então não são usados como portão.
- **Antes de cada release:** checagem manual no Android do dono das metas de tela, com um roteiro curto de passos.

## 9. Offline (escolha minha)
Funciona sem internet. Só pôsteres novos, onde assistir atualizado, lançamentos e sync esperam conexão.

## 10. Fases
- **v1** — filmes, Android. Tudo acima.
- **v1.x** — app Linux (mesmas telas).
- **v2** — séries. Sem base pública boa de notas individuais: motor de conteúdo + notas do dono. Série avaliada como um todo. Catálogo preparado desde a v1 para outros tipos de obra.
- **v3** — animes. Colaborativo com base derivada do MyAnimeList (Hernan4444, 2020, ~57M notas; sem licença declarada — reavaliar). Ids MAL → TMDB via listas de mapeamento.
- **Futuro possível** — notas de usuários do Trakt via API (termos não confirmados).

## 11. Passos únicos do dono (com guia)
1. Chave gratuita TMDB.
2. Chave gratuita OMDb.
3. Projeto Google Cloud: tela de consentimento "Em produção", cliente OAuth Android + desktop, escopo `drive.appdata`.
4. Instalar Obtainium e adicionar o repo.
5. Colocar como secrets do repositório GitHub: chaves TMDB e OMDb, token do Kaggle (download da base Letterboxd no CI) e a chave de assinatura do APK. A chave de assinatura é gerada uma vez e nunca muda: o cliente OAuth Android do Google fica preso à impressão digital (SHA-1) dela.
6. Copiar os CSVs do export do MovieLens para o celular na primeira abertura (o app pede o arquivo).

## 12. Deixado ao builder
- Estrutura do código, biblioteca de UI dentro do Tauri, formato de armazenamento local, formato dos arquivos de sync.
- Algoritmo exato de fatoração, dimensões, regularização; método de confiança.
- Plugin/fluxo de login Google (opções conhecidas: `tauri-plugin-google-auth`, pré-1.0; ou Custom Tabs + PKCE + deep link — WebView embutida é bloqueada pelo Google).
- Limiar do catálogo, estratégia de atualização incremental, política de cache de pôsteres.
- Regra exata de seleção da rodada rápida e do destaque do Início.
- Duração de "agora não".
- Como reduzir o tamanho do modelo para o celular.
