# Movie recommender

A personal replacement for MovieLens: rate movies, get predictions and recommendations, kept in sync across the owner's devices. The **owner** is whoever uses an installation; two people mean two owners, each with their own notas and Drive, sharing nothing.

## Language

**Obra** (Work):
Anything the app can recommend: a filme, a série or an anime.
_Avoid_: título, item, conteúdo

**Anime**:
A serialized animation in the anime style, from any country (Cyberpunk: Edgerunners, Link Click, Castlevania; not Scooby-Doo or Ben 10), rated as a whole or by temporada, and recommended as a whole. An animated feature is a filme, not an anime; hentai is left out.
_Avoid_: desenho, animação

**Temporada** (Season):
One season of an anime as TMDB lists it. A nota on a temporada wins over the nota on the whole anime for that temporada. Séries have no notas by temporada.
_Avoid_: parte, cour

**Próxima temporada** (Next season):
A row on Início with the temporada after what the owner has seen of an anime they rated well or are Assistindo; a plain reminder, with no nota prevista or notification.
_Avoid_: continuação, nova temporada

**Série** (Series):
A scripted series, miniseries or documentary series, rated as a whole; not reality, talk shows, news, daily telenovelas or anime.
_Avoid_: show, programa, seriado

**Nota** (Rating):
A score the owner gives an obra, from 0.5 to 5 in half-star steps.
_Avoid_: avaliação, score, estrela

**Reação** (Reaction):
A quick verdict on a whole série or anime, or on one temporada of an anime, up to where the owner has seen it: Não gostei, Gostei or Amei; it counts as a fixed nota. The owner may give it a nota in stars instead.
_Avoid_: joinha, like, curtida

**Assistindo** (Watching):
A mark that the owner is partway through a série or anime; it takes it out of recomendações and teaches nothing about taste.
_Avoid_: em andamento (that is the série's own situação), progresso

**Nota prevista** (Predicted rating):
The score the recommender expects the owner would give an obra not yet rated; a continuous number, not restricted to half stars. For an obra already rated, the same score with its own nota left out.
_Avoid_: previsão, match, recomendação

**Nota do público** (Public score):
The average of other people's votes on an obra, shown as it is and never mixed into the nota prevista. Séries in the lowest quarter of it, among those with at least 50 votes, are below the **piso** and do not become recomendações; animes face the same value.
_Avoid_: nota (alone), nota da crítica

**Confiança** (Confidence):
How certain the recommender is about a nota prevista, shown alongside it as a range or level.
_Avoid_: certeza, precisão

**Faixa** (Prediction band):
A nota prevista shown as one of four ordinal bands (Aposta forte, Boa chance, Arriscado, Provavelmente não), set by where it falls among the owner's own predictions of that type, with a floor so that a high nota never reads as a verdict against the obra: from 4.0 at worst Boa chance, from 3.0 at worst Arriscado. The default way of showing a nota prevista.
_Avoid_: categoria, nível

**Recomendação** (Recommendation):
An obra surfaced to the owner because its nota prevista is high and it passes the active filters.
_Avoid_: top pick, sugestão

**Catálogo** (Catalog):
The set of all obras the app knows about, including recent releases.
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
Obras close to a given obra and of the same type, shown from its page: for a filme, those that people who liked it also liked; for a série, those with the closest content; for an anime, those closest in its factors or, without them, its content.
_Avoid_: relacionados, mais como este

**Populares ↔ Garimpo** (Popularity bias):
The owner's setting for how much popularity weighs in ranking recomendações, from well-known movies to obscure ones.
_Avoid_: algoritmo, modo

**Rodada rápida** (Quick rating session):
A sequence of obras of one type the owner has likely seen, chosen to teach the recommender the most, each one answered (a nota on a filme, a Reação on a série or a whole anime) or skipped as "não vi".
_Avoid_: avaliação rápida, onboarding, calibração

**Já vi** (Seen):
A mark that the owner has watched a movie without giving it a nota; it removes the movie from recomendações but teaches nothing about taste.
_Avoid_: assistido, visto

**Não me interessa** (Dismissed):
A permanent mark hiding a movie from recomendações, counted as a weak negative signal.
_Avoid_: ocultar, agora não

**Destaque** (Featured pick):
The obra shown at the top of Início, one at a time: the best recomendação not set aside with agora não, swiping on to the next few.
_Avoid_: top pick, hero

**Agora não** (Not now):
A mark that keeps an obra out of the destaque for a few weeks; it hides it nowhere else and teaches nothing about taste.
_Avoid_: adiar, não me interessa

**Para rever** (Rewatch list):
A lista pronta of obras marked já vi whose nota prevista is high.
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

**Minhas notas** (My ratings):
The list of every movie the owner rated, the most recent nota first, opened from Perfil.
_Avoid_: histórico (that is one movie's past notas)
