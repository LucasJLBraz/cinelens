# Movie recommender

A personal, single-user replacement for MovieLens: rate movies, get predictions and recommendations, kept in sync across the owner's devices.

## Language

**Nota** (Rating):
A score the owner gives a movie, from 0.5 to 5 in half-star steps.
_Avoid_: avaliação, score, estrela

**Nota prevista** (Predicted rating):
The score the recommender expects the owner would give a movie not yet rated; a continuous number, not restricted to half stars.
_Avoid_: previsão, match, recomendação

**Confiança** (Confidence):
How certain the recommender is about a nota prevista, shown alongside it as a range or level.
_Avoid_: certeza, precisão

**Faixa** (Prediction band):
A nota prevista shown as one of four ordinal bands (Aposta forte, Boa chance, Arriscado, Provavelmente não), set by where it falls among the owner's own predictions; the default way of showing a nota prevista.
_Avoid_: categoria, nível

**Recomendação** (Recommendation):
A movie surfaced to the owner because its nota prevista is high and it passes the active filters.
_Avoid_: top pick, sugestão

**Catálogo** (Catalog):
The set of all movies the app knows about, including recent releases.
_Avoid_: base, acervo

**Quero assistir** (Watchlist):
Movies the owner has marked to watch later.
_Avoid_: wishlist, lista de desejos

**Importação** (Import):
The one-time load of the owner's MovieLens rating export into the app; after it, the app is the source of truth.
_Avoid_: migração, sync com MovieLens

**Sync**:
Exchange of changes between the owner's devices through cloud storage; on conflicting notas for the same movie, the most recent wins.
_Avoid_: backup, upload

**Lista pronta** (Preset list):
A named, app-defined combination of filters and ranking, such as "Sessão da Tarde" or "Joias escondidas".
_Avoid_: filtro salvo, coleção, playlist

**Parecidos** (Similar titles):
Movies that people who liked a given movie also liked, shown from that movie's page.
_Avoid_: relacionados, mais como este

**Populares ↔ Garimpo** (Popularity bias):
The owner's setting for how much popularity weighs in ranking recomendações, from well-known movies to obscure ones.
_Avoid_: algoritmo, modo

**Rodada rápida** (Quick rating session):
A sequence of movies chosen to teach the recommender the most about the owner, each one rated or skipped as "não vi".
_Avoid_: avaliação rápida, onboarding, calibração

**Já vi** (Seen):
A mark that the owner has watched a movie without giving it a nota; it removes the movie from recomendações but teaches nothing about taste.
_Avoid_: assistido, visto

**Não me interessa** (Dismissed):
A permanent mark hiding a movie from recomendações, counted as a weak negative signal.
_Avoid_: ocultar, agora não

**Agora não** (Not now):
A mark that keeps a movie out of the featured pick of Início for a few weeks; it hides it nowhere else and teaches nothing about taste.
_Avoid_: adiar, não me interessa

**Para rever** (Rewatch list):
A lista pronta of movies marked já vi whose nota prevista is high.
_Avoid_: reassistir, revisitar

**Sou fã** (Favorite person):
A mark on a director, actor or composer that raises their movies in the owner's ranking.
_Avoid_: seguir, favoritar

**O que pesou** (Rating aspects):
Optional aspects (fotografia, atuações, roteiro, trilha, ritmo…) the owner tags after giving a nota, steering recomendações toward movies strong in them.
_Avoid_: tags, motivos

**Parecidos, mas…** (Tuned similar):
Parecidos refined in one direction, such as lighter, shorter or more recent.
_Avoid_: movie tuner, filtro de parecidos

## Screens

**Início** (Home):
The showcase screen of ready-made entry points (featured pick, preset lists, rows); it holds no filters.
_Avoid_: feed, dashboard, vitrine

**Explorar** (Explore):
The single filterable grid of the catalog, opened unfiltered or with a filter pre-set from Início; search lives here.
_Avoid_: buscar, grade, catálogo (for the screen)
