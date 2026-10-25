//! Thin Tauri adapters over the core facade: catálogo and model files and downloads, owner data
//! file, poster cache.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cinelens_core::{Core, Decay, DecayStatus, ExplorarPage, ImportSummary, MoviePage, CATALOG_FORMAT_VERSION, MODEL_FORMAT_VERSION};
use tauri::http::Response;
use tauri::{AppHandle, Emitter, Manager};

const POSTER_SOURCE: &str = "https://image.tmdb.org/t/p/w342/";
const READ_BUFFER: usize = 256 * 1024;

fn catalog_url() -> String {
    format!("https://github.com/LucasJLBraz/cinelens/releases/download/catalog-v{CATALOG_FORMAT_VERSION}/catalog.json.gz")
}

fn model_url() -> String {
    format!("https://github.com/LucasJLBraz/cinelens/releases/download/model-v{MODEL_FORMAT_VERSION}/model.json.gz")
}

fn catalog_file(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("catalog.json")
}

fn model_file(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("model.json")
}

fn owner_file(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("owner.json")
}

fn save_owner(app: &AppHandle, core: &Core) -> Result<(), String> {
    write_atomically(&owner_file(app), &core.owner_data()).map_err(|e| e.to_string())
}

/// `files` are the texts of the export files the owner picked, in any order.
#[tauri::command]
fn import_movielens(
    app: AppHandle,
    core: tauri::State<RwLock<Core>>,
    files: Vec<String>,
) -> Result<ImportSummary, String> {
    let mut core = core.write().unwrap();
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    let summary = core.import_movielens(&files).map_err(|e| e.to_string())?;
    save_owner(&app, &core)?;
    Ok(summary)
}

#[tauri::command]
fn movie(core: tauri::State<RwLock<Core>>, tmdb_id: u32) -> Option<MoviePage> {
    core.read().unwrap().movie(tmdb_id)
}

#[tauri::command]
fn rate(
    app: AppHandle,
    core: tauri::State<RwLock<Core>>,
    tmdb_id: u32,
    nota: f32,
) -> Result<Option<MoviePage>, String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let mut core = core.write().unwrap();
    core.rate(tmdb_id, nota, now).map_err(|e| e.to_string())?;
    save_owner(&app, &core)?;
    Ok(core.movie(tmdb_id))
}

#[tauri::command]
fn decay(core: tauri::State<RwLock<Core>>) -> DecayStatus {
    core.read().unwrap().decay()
}

#[tauri::command]
fn set_decay(app: AppHandle, core: tauri::State<RwLock<Core>>, decay: Decay) -> Result<DecayStatus, String> {
    let mut core = core.write().unwrap();
    core.set_decay(decay);
    save_owner(&app, &core)?;
    Ok(core.decay())
}

#[tauri::command]
fn explorar(core: tauri::State<RwLock<Core>>, page: usize) -> ExplorarPage {
    core.read().unwrap().explorar(page)
}

/// The data versions of the local catálogo and model; `None` before their first download.
#[tauri::command]
fn status(core: tauri::State<RwLock<Core>>) -> (Option<String>, Option<String>) {
    let core = core.read().unwrap();
    (core.data_version().map(str::to_string), core.model_version().map(str::to_string))
}

/// Downloads the full catálogo, emitting `download-progress`. The local catálogo is only
/// replaced once the core accepts the new one.
#[tauri::command]
async fn download_catalog(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let json = download(&app, &catalog_url())?;
        // Validated before it touches disk, loaded only once it is saved; parsed twice so the
        // lock is never held during a download.
        Core::open(&json).map_err(|e| e.to_string())?;
        write_atomically(&catalog_file(&app), &json).map_err(|e| e.to_string())?;
        app.state::<RwLock<Core>>().write().unwrap().load_catalog(&json).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Downloads the model, emitting `download-progress`; same guarantees as the catálogo.
#[tauri::command]
async fn download_model(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let json = download(&app, &model_url())?;
        Core::default().load_model(&json).map_err(|e| e.to_string())?;
        write_atomically(&model_file(&app), &json).map_err(|e| e.to_string())?;
        app.state::<RwLock<Core>>().write().unwrap().load_model(&json).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Downloads and gunzips `url`, emitting `download-progress` as `[received, total]` bytes
/// (`total` is null when unknown).
fn download(app: &AppHandle, url: &str) -> Result<Vec<u8>, String> {
    let mut response = ureq::get(url)
        .config()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_recv_response(Some(Duration::from_secs(30)))
        .build()
        .call()
        .map_err(|e| e.to_string())?;
    let total: Option<usize> = response
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok()?.parse().ok());
    let mut body = response.body_mut().with_config().limit(u64::MAX).reader();
    let mut gz = Vec::with_capacity(total.unwrap_or(0));
    let mut chunk = vec![0; READ_BUFFER];
    loop {
        let n = body.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        gz.extend_from_slice(&chunk[..n]);
        let _ = app.emit("download-progress", (gz.len(), total));
    }

    let mut json = Vec::new();
    flate2::read::GzDecoder::new(&gz[..]).read_to_end(&mut json).map_err(|e| e.to_string())?;
    Ok(json)
}

fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // Unique per write: two requests for the same poster must not share a temp file.
    static NEXT: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension(format!("{}.tmp", NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, path)
}

/// TMDB poster file names look like `mCICnh7QBH0gzYaTQChBDDVIKdm.jpg`; nothing else is served.
fn is_poster_name(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && stem.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            && matches!(ext, "jpg" | "png")
    })
}

/// Serves `poster://localhost/<name>` from the on-device cache, fetching from TMDB on a miss.
fn poster(app: &AppHandle, name: &str) -> Response<Vec<u8>> {
    let respond = |status: u16, bytes: Vec<u8>| {
        Response::builder()
            .status(status)
            .header("Content-Type", if name.ends_with(".png") { "image/png" } else { "image/jpeg" })
            .header("Cache-Control", "max-age=31536000, immutable")
            .body(bytes)
            .unwrap()
    };
    if !is_poster_name(name) {
        return respond(404, Vec::new());
    }
    let path = app.path().app_cache_dir().expect("app cache dir").join("posters").join(name);
    if let Ok(bytes) = std::fs::read(&path) {
        return respond(200, bytes);
    }
    let fetched = ureq::get(format!("{POSTER_SOURCE}{name}"))
        .config()
        .timeout_global(Some(Duration::from_secs(20)))
        .build()
        .call()
        .and_then(|mut r| r.body_mut().read_to_vec());
    match fetched {
        Ok(bytes) => {
            // A failed cache write only costs a re-download next time.
            let _ = write_atomically(&path, &bytes);
            respond(200, bytes)
        }
        Err(_) => respond(502, Vec::new()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let mut core = Core::default();
            // A missing or unreadable catálogo means the UI downloads it again.
            if let Ok(json) = std::fs::read(catalog_file(app.handle())) {
                let _ = core.load_catalog(&json);
            }
            // Same for the model.
            if let Ok(json) = std::fs::read(model_file(app.handle())) {
                let _ = core.load_model(&json);
            }
            // Unlike the catálogo, owner data cannot be downloaded again: refuse to start over it.
            match std::fs::read(owner_file(app.handle())) {
                Ok(bytes) => core.load_owner_data(&bytes)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            app.manage(RwLock::new(core));
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("poster", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let name = request.uri().path().trim_start_matches('/').to_string();
            tauri::async_runtime::spawn_blocking(move || responder.respond(poster(&app, &name)));
        })
        .invoke_handler(tauri::generate_handler![
            explorar,
            status,
            download_catalog,
            download_model,
            import_movielens,
            movie,
            rate,
            decay,
            set_decay
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
