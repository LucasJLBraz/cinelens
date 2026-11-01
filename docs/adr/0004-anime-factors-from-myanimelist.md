# Anime factors are trained on MyAnimeList ratings

Animes get collaborative factors from a frozen MyAnimeList dump, the Gatchalian November 2023 Kaggle sample (declared MIT), one row per MAL entry. The training runs in a manual GitHub workflow, downloading the dump from Kaggle, and only the factors are published, in a public release; ratings and usernames are never committed or published. Animes without factors, including everything released after the dump, are predicted through content.

MyAnimeList's terms bar copying and aggregating its content, whatever license a Kaggle upload declares, and no later or cleaner dump was found. We accepted this: the project is personal and non-commercial, and factors hold no rating, user or text from the site. If asked to stop, we remove the release and animes fall back to content, with nothing else depending on the factors.

## Measurements (2026-11-01)

Same ALS as `train` (32 factors, 10 iterations), run locally. 3,000 users of each dump were held out; for each one, 10 notas on serialized entries were hidden and the model was folded in on the notas below. MAE on the app's 0.5 to 5 scale. "Reação" means fold-in notas reduced to 2.5, 4 and 5; hidden notas keep their real value.

Hernan4444 (2020, 57M notas, 310k users), its own users:

| Fold-in | Collaborative | Content | Public + owner's bias | Public mean |
|---|---|---|---|---|
| 6 filmes | 0.548 | 0.608 | 0.530 | 0.573 |
| 10 animes | 0.480 | 0.578 | 0.491 | 0.578 |
| 30 animes | 0.446 | 0.561 | 0.476 | 0.578 |
| 10 animes, Reação | 0.517 | 0.572 | 0.514 | 0.578 |
| 30 animes, Reação | 0.477 | 0.550 | 0.495 | 0.578 |

Collaborative MAE of each dump on each population:

| Fold-in | Hernan model on Hernan users | Gatchalian model on Hernan users | Hernan model on Gatchalian users | Gatchalian model on Gatchalian users |
|---|---|---|---|---|
| 6 filmes | 0.548 | 0.574 | 0.691 | 0.665 |
| 30 animes | 0.446 | 0.489 | 0.627 | 0.579 |
| 30 animes, Reação | 0.477 | 0.527 | 0.628 | 0.602 |

## Decision

Collaborative beats content in every cut, by 0.06 from 6 filmes and more with notas on animes, so it stays. Each dump wins on its own users and they tie on average, and the tie goes to Gatchalian: it is three years newer (MAL ids up to 57037 against 48456) and declares a license.

Public mean plus the owner's bias beats collaborative below about 10 notas, but it stays out of the nota prevista: the same score for everyone does not personalize a ranking, and this measurement looks at error only, not ranking.

These users are heavy MAL users, and the owner's six filmes in common are nearly all 5.0, so real results from the bridge may be worse. The owner's own check comes from the informational MAE after 30 notas on animes.
