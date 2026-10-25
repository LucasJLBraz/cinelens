# Letterboxd in training only if it does not worsen the MAE

The monthly model can also train on the Letterboxd April 2026 sample (CC BY-NC-SA 4.0), which covers 2023 to April 2026, the years `ml-latest` lacks. It enters only while it does not raise the MAE on the owner's most recent notas over the MovieLens-only model, measured locally with `cinelens-accuracy` (`docs/guides/accuracy.md`). The switch is the `KAGGLE_API_TOKEN` repository secret: without it the Model workflow trains on MovieLens alone. Letterboxd users get a bias term against MovieLens's, and Letterboxd rows with an IMDb match confidence under 0.9 are dropped.

## Measurements

Owner's export, 289 notas; the most recent are hidden from the fold-in. "MovieLens" is MovieLens's own error on the same notas, from the predictions it logged when each nota was given.

| Model | Held out | In the model | MAE | MovieLens |
|---|---|---|---|---|
| ml-latest only (2026-10-25) | 10% (29) | 19 | 0.375 | 0.454 |
| ml-latest only (2026-10-25) | 20% (58) | 38 | 0.428 | 0.483 |
| ml-latest only (2026-10-25) | 30% (87) | 65 | 0.493 | 0.509 |
| ml-latest + Letterboxd | — | — | not measured yet | — |

The MovieLens-only model already meets the v1 bar (MAE ≤ 0.65; MovieLens on all the owner's notas: 0.654). A third of the held-out notas are of movies outside `ml-latest`, which is what Letterboxd would add.

## Decision

Off until measured: the owner has not created the Kaggle token yet. When it exists, train both models, fill the last row, and set the secret only if the Letterboxd model's MAE at 20% is not above 0.428.
