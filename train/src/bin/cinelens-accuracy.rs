//! `cinelens-accuracy --model model.json.gz [--catalog catalog.json.gz] [--held-out 0.2] movielens-*.csv`
//! `cinelens-accuracy --model model.json.gz --catalog catalog.json.gz --series series.json.gz --owner owner.json [--held-out 0.2]`
//!
//! The accuracy harness on the owner's real MovieLens export, run locally only (the export never
//! reaches CI): imports it into a core, hides the most recent notas from the fold-in and prints
//! the MAE of the nota prevista on them, next to MovieLens's own on the same notas. With the
//! catálogo, held-out movies outside the model are also predicted from their content, measured
//! apart. With the séries and the app's owner data, it instead measures the notas on séries:
//! the error for each weight of a nota on a série, and above and below the floor.

/// MovieLens's MAE on all the owner's notas, from the predictions in the logs (design §6.6).
const MOVIELENS_BASELINE: f32 = 0.654;
/// Weights of a nota on a série to compare.
const SERIE_WEIGHTS: [f32; 6] = [1.0, 2.0, 3.0, 5.0, 8.0, 13.0];

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let model = flag("--model").ok_or("missing --model")?;
    let fraction: f32 = flag("--held-out").map_or(Ok(0.2), |f| f.parse()).map_err(|_| "bad --held-out")?;
    let files: Vec<String> = args
        .iter()
        .enumerate()
        .filter(|&(i, a)| !a.starts_with("--") && (i == 0 || !args[i - 1].starts_with("--")))
        .map(|(_, path)| std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}")))
        .collect::<Result<_, _>>()?;

    let mut core = match flag("--catalog") {
        Some(catalog) => cinelens_core::Core::open(&cinelens_train::read_maybe_gzipped(catalog)?).map_err(|e| e.to_string())?,
        None => cinelens_core::Core::default(),
    };
    core.load_model(&cinelens_train::read_maybe_gzipped(model)?).map_err(|e| e.to_string())?;
    if let Some(series) = flag("--series") {
        core.load_series(&cinelens_train::read_maybe_gzipped(series)?).map_err(|e| e.to_string())?;
    }
    if let Some(owner) = flag("--owner") {
        let owner = std::fs::read(owner).map_err(|e| format!("{owner}: {e}"))?;
        core.load_owner_data(&owner).map_err(|e| e.to_string())?;
        return series(&core, fraction);
    }
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    core.import_movielens(&files).map_err(|e| e.to_string())?;
    let accuracy = core.accuracy(fraction).ok_or("no held-out nota is in the model")?;

    println!("model {}", core.model_version().unwrap_or("?"));
    println!("held out: {} most recent notas, {} in the model", accuracy.held_out, accuracy.notas);
    println!("MAE cinelens: {:.3}", accuracy.mae);
    match accuracy.movielens_mae {
        Some(mae) => println!("MAE MovieLens on the same notas (where it logged a prediction): {mae:.3}"),
        None => println!("MAE MovieLens on the same notas: (no logged predictions)"),
    }
    println!("MovieLens baseline on all notas: {MOVIELENS_BASELINE}");
    if let Some(mae) = accuracy.content_mae {
        println!("MAE cinelens on {} held-out notas outside the model, from their content: {mae:.3}", accuracy.content_notas);
    }
    Ok(())
}

fn series(core: &cinelens_core::Core, fraction: f32) -> Result<(), String> {
    println!("model {}", core.model_version().unwrap_or("?"));
    for peso in SERIE_WEIGHTS {
        let a = core.accuracy_series(fraction, peso).ok_or("no held-out nota on a série can be predicted")?;
        println!("weight {peso}: MAE {:.3} on {} notas", a.geral.mae, a.geral.notas);
    }
    let a = core.accuracy_series(fraction, cinelens_core::SERIE_WEIGHT).ok_or("no held-out nota on a série can be predicted")?;
    for (nome, grupo) in [("above the floor", a.acima), ("below the floor", a.abaixo)] {
        match grupo {
            Some(g) => println!("{nome}: MAE {:.3}, mean nota {:.2} on {} notas", g.mae, g.media, g.notas),
            None => println!("{nome}: no notas"),
        }
    }
    Ok(())
}
