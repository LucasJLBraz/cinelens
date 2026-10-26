# Letterboxd in training only if it does not worsen the MAE

The monthly model can also train on the Letterboxd April 2026 sample (CC BY-NC-SA 4.0), which covers 2023 to April 2026, the years `ml-latest` lacks. It enters only while it does not raise the MAE on the owner's most recent notas over the MovieLens-only model, measured locally with `cinelens-accuracy` (`docs/guides/accuracy.md`). The switch is the Model workflow's `letterboxd` input: scheduled runs train on MovieLens alone. Letterboxd users get a bias term against MovieLens's.

The published `letterboxd_interactions.csv` has no IMDb id, only `username, film_slug, lb_film_id, film_name, film_year, rating, liked`. Its movies are joined to TMDB ids through Wikidata's items with both a Letterboxd film ID (P6127, the slug) and a TMDB movie ID (P4947), fetched with one SPARQL query per run. Slugs Wikidata lacks fall back to the normalized title and year against the catálogo's `original_title`, or else its `title`, kept only when exactly one movie matches.

## Measurements

Owner's export, 289 notas; the most recent are hidden from the fold-in. "MovieLens" is MovieLens's own error on the same notas, from the predictions it logged when each nota was given. Both 2026-10-26 models come from the Model workflow with `publish` off (same catálogo, genome and trainer).

| Model | Held out | In the model | MAE | MovieLens |
|---|---|---|---|---|
| ml-latest only (2026-10-25) | 10% (29) | 19 | 0.375 | 0.454 |
| ml-latest only (2026-10-25) | 20% (58) | 38 | 0.428 | 0.483 |
| ml-latest only (2026-10-25) | 30% (87) | 65 | 0.493 | 0.509 |
| ml-latest only (2026-10-26) | 10% (29) | 19 | 0.375 | 0.454 |
| ml-latest only (2026-10-26) | 20% (58) | 38 | 0.428 | 0.483 |
| ml-latest only (2026-10-26) | 30% (87) | 65 | 0.493 | 0.509 |
| ml-latest + Letterboxd (2026-10-26) | 10% (29) | 23 | 0.666 | 0.575 |
| ml-latest + Letterboxd (2026-10-26) | 20% (58) | 51 | 0.516 | 0.536 |
| ml-latest + Letterboxd (2026-10-26) | 30% (87) | 80 | 0.523 | 0.549 |

The join on 14,673,076 rated Letterboxd rows: 92.2% by Wikidata slug, 1.2% by title and year, 6.5% dropped.

With Letterboxd, 13 more of the 58 held-out notas at 20% are in the model, and those are harder notas: MovieLens's own error rises from 0.483 to 0.536 on the larger set. But the MAE gets worse at every split, most at 10%, where the two models share 19 of 23 notas. So Letterboxd does not only add harder movies; it also shifts the factors of the ones MovieLens already had.

## Decision (2026-10-26)

Off. At 20% held out the Letterboxd model's MAE (0.516) is above the MovieLens-only one (0.428), and it is worse at 10% and 30% too. Scheduled runs keep training on MovieLens alone. The join stays in the trainer and the workflow, so a manual run with `letterboxd` and `publish` off can measure again, for example after #38 changes the trainer.
