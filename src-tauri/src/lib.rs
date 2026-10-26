//! Thin Tauri adapters over the core facade: catálogo and model files, downloads and background
//! updates, owner data file and its sync, poster cache.

mod google;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cinelens_core::{Aspecto, Core, Decay, Direcao, DecayStatus, DisplayMode, Estatisticas, Exibicao, ExplorarPage, Filmografia, Filtro, ImportSummary, Inicio, Opcoes, Marca, MinhaNota, MovieCard, MoviePage, Provedor, Rejeicao, SyncStorage, CATALOG_FORMAT_VERSION, MODEL_FORMAT_VERSION};
use tauri::http::Response;
use tauri::{AppHandle, Emitter, Manager};

const POSTER_SOURCE: &str = "https://image.tmdb.org/t/p/w342/";
const READ_BUFFER: usize = 256 * 1024;

/// How often the app looks for a new catálogo and model while open.
const UPDATE_EVERY: Duration = Duration::from_secs(6 * 3600);
/// How often the app syncs while open, to pick up changes from the owner's other devices.
const SYNC_EVERY: Duration = Duration::from_secs(10 * 60);

fn catalog_release(asset: &str) -> String {
    format!("https://github.com/LucasJLBraz/cinelens/releases/download/catalog-v{CATALOG_FORMAT_VERSION}/{asset}")
}

fn model_release(asset: &str) -> String {
    format!("https://github.com/LucasJLBraz/cinelens/releases/download/model-v{MODEL_FORMAT_VERSION}/{asset}")
}

fn catalog_url() -> String {
    catalog_release("catalog.json.gz")
}

fn model_url() -> String {
    model_release("model.json.gz")
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

/// The last new model kept out by the accuracy gate, and why.
fn rejected_file(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir").join("model-rejected.json")
}

/// Saves the owner data, then shares it with the owner's other devices.
fn save_owner(app: &AppHandle, core: &Core) -> Result<(), String> {
    write_atomically(&owner_file(app), &core.owner_data()).map_err(|e| e.to_string())?;
    schedule_sync(app);
    Ok(())
}

/// What the core reads and writes during a merge, so no network call happens under the lock.
struct Fetched {
    remote: Option<Vec<u8>>,
    merged: Option<Vec<u8>>,
}

impl SyncStorage for Fetched {
    fn get(&mut self) -> Result<Option<Vec<u8>>, String> {
        Ok(self.remote.clone())
    }

    fn put(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.merged = Some(bytes.to_vec());
        Ok(())
    }
}

/// Syncs in the background if the owner signed in; changes made during a sync trigger another.
/// Never asks the owner anything: without a valid grant it does nothing.
fn schedule_sync(app: &AppHandle) {
    static PENDING: AtomicBool = AtomicBool::new(false);
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if !google::signed_in(app) {
        return;
    }
    PENDING.store(true, Ordering::SeqCst);
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        while PENDING.swap(false, Ordering::SeqCst) {
            if let Err(e) = sync_once(&app) {
                eprintln!("sync: {e}");
            }
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
}

fn sync_once(app: &AppHandle) -> Result<(), String> {
    let mut drive = google::Drive::new(google::access_token(app)?);
    let mut fetched = Fetched { remote: drive.get()?, merged: None };
    let changed = {
        let state = app.state::<RwLock<Core>>();
        let mut core = state.write().unwrap();
        let before = core.owner_data();
        core.sync(&mut fetched).map_err(|e| e.to_string())?;
        let after = core.owner_data();
        if after != before {
            write_atomically(&owner_file(app), &after).map_err(|e| e.to_string())?;
        }
        after != before
    };
    if let Some(merged) = fetched.merged {
        drive.put(&merged)?;
    }
    if changed {
        let _ = app.emit("sincronizado", ());
    }
    Ok(())
}

/// Signs in with Google (once per device) and syncs right away.
#[tauri::command]
async fn google_entrar(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        google::sign_in(&app)?;
        schedule_sync(&app);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn google_conectado(app: AppHandle) -> bool {
    google::signed_in(&app)
}

/// The last new model kept out, and why; `None` when the current model is the newest.
#[tauri::command]
fn modelo_rejeitado(app: AppHandle) -> Option<Rejeicao> {
    serde_json::from_slice(&std::fs::read(rejected_file(&app)).ok()?).ok()
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
fn pessoa(core: tauri::State<RwLock<Core>>, person_id: u32) -> Option<Filmografia> {
    core.read().unwrap().pessoa(person_id)
}

/// Marks or unmarks the owner as fã of the person; returns their filmography after.
#[tauri::command]
fn set_fa(app: AppHandle, core: tauri::State<RwLock<Core>>, person_id: u32, on: bool) -> Result<Option<Filmografia>, String> {
    let mut core = core.write().unwrap();
    core.set_fa(person_id, on, now());
    save_owner(&app, &core)?;
    Ok(core.pessoa(person_id))
}

/// The movie page after the nota, and what the app had predicted for it (the reveal).
#[tauri::command]
fn rate(
    app: AppHandle,
    core: tauri::State<RwLock<Core>>,
    tmdb_id: u32,
    nota: f32,
) -> Result<(Option<MoviePage>, Option<Exibicao>), String> {
    let mut core = core.write().unwrap();
    let revealed = core.rate(tmdb_id, nota, now()).map_err(|e| e.to_string())?;
    save_owner(&app, &core)?;
    Ok((core.movie(tmdb_id), revealed))
}

/// The rodada rápida's next `n` movies.
#[tauri::command]
fn rodada(core: tauri::State<RwLock<Core>>, n: usize) -> Vec<MovieCard> {
    core.read().unwrap().rodada(n)
}

#[tauri::command]
fn nao_vi(app: AppHandle, core: tauri::State<RwLock<Core>>, tmdb_id: u32) -> Result<(), String> {
    let mut core = core.write().unwrap();
    core.nao_vi(tmdb_id);
    save_owner(&app, &core)
}

#[tauri::command]
fn parecidos(core: tauri::State<RwLock<Core>>, tmdb_id: u32, desde: usize) -> Vec<MovieCard> {
    core.read().unwrap().parecidos(tmdb_id, desde)
}

#[tauri::command]
fn parecidos_mas(core: tauri::State<RwLock<Core>>, tmdb_id: u32, direcao: Direcao) -> Vec<MovieCard> {
    core.read().unwrap().parecidos_mas(tmdb_id, direcao)
}

#[tauri::command]
fn set_pesou(app: AppHandle, core: tauri::State<RwLock<Core>>, tmdb_id: u32, aspectos: Vec<Aspecto>) -> Result<(), String> {
    let mut core = core.write().unwrap();
    core.set_pesou(tmdb_id, &aspectos, now());
    save_owner(&app, &core)
}

#[tauri::command]
fn pesou_oculto(core: tauri::State<RwLock<Core>>) -> bool {
    core.read().unwrap().pesou_oculto()
}

#[tauri::command]
fn set_pesou_oculto(app: AppHandle, core: tauri::State<RwLock<Core>>, oculto: bool) -> Result<(), String> {
    let mut core = core.write().unwrap();
    core.set_pesou_oculto(oculto, now());
    save_owner(&app, &core)
}

/// Marks or unmarks the movie; returns its page after.
#[tauri::command]
fn marcar(
    app: AppHandle,
    core: tauri::State<RwLock<Core>>,
    tmdb_id: u32,
    marca: Marca,
    on: bool,
) -> Result<Option<MoviePage>, String> {
    let mut core = core.write().unwrap();
    core.marcar(tmdb_id, marca, on, now());
    save_owner(&app, &core)?;
    Ok(core.movie(tmdb_id))
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

#[tauri::command]
fn notificacao(core: tauri::State<RwLock<Core>>) -> bool {
    core.read().unwrap().notificacao()
}

#[tauri::command]
fn set_notificacao(app: AppHandle, core: tauri::State<RwLock<Core>>, on: bool) -> Result<(), String> {
    let mut core = core.write().unwrap();
    core.set_notificacao(on, now());
    save_owner(&app, &core)
}

/// Shows the weekly notification if one is due, asking the owner for the permission the first
/// time one is; it counts as shown only once it is.
// ponytail: checked only while the app runs (after each update), so a week without opening it
// sends nothing; schedule an Android WorkManager job if the owner misses releases.
fn notify(app: &AppHandle) -> Result<(), String> {
    use tauri_plugin_notification::{NotificationExt, PermissionState};
    let Some(due) = app.state::<RwLock<Core>>().read().unwrap().notificacao_devida(now()) else { return Ok(()) };
    let notification = app.notification();
    let mut permission = notification.permission_state().map_err(|e| e.to_string())?;
    if matches!(permission, PermissionState::Prompt | PermissionState::PromptWithRationale) {
        permission = notification.request_permission().map_err(|e| e.to_string())?;
    }
    if permission != PermissionState::Granted {
        return Ok(());
    }
    notification.builder().title("Lançamento para você").body(due.texto).show().map_err(|e| e.to_string())?;
    let state = app.state::<RwLock<Core>>();
    let mut core = state.write().unwrap();
    core.notificado(due.tmdb_id, now());
    save_owner(app, &core)
}

#[tauri::command]
fn display_mode(core: tauri::State<RwLock<Core>>) -> DisplayMode {
    core.read().unwrap().display_mode()
}

#[tauri::command]
fn set_display_mode(app: AppHandle, core: tauri::State<RwLock<Core>>, mode: DisplayMode) -> Result<(), String> {
    let mut core = core.write().unwrap();
    core.set_display_mode(mode, now());
    save_owner(&app, &core)
}

/// Every service in the catálogo, and the ids of those the owner subscribes to.
#[tauri::command]
fn servicos(core: tauri::State<RwLock<Core>>) -> (Vec<Provedor>, Vec<u32>) {
    let core = core.read().unwrap();
    (core.provedores().to_vec(), core.servicos())
}

#[tauri::command]
fn set_servicos(app: AppHandle, core: tauri::State<RwLock<Core>>, ids: Vec<u32>) -> Result<(), String> {
    let mut core = core.write().unwrap();
    core.set_servicos(&ids, now());
    save_owner(&app, &core)
}

#[tauri::command]
fn decay(core: tauri::State<RwLock<Core>>) -> DecayStatus {
    core.read().unwrap().decay()
}

#[tauri::command]
fn set_decay(app: AppHandle, core: tauri::State<RwLock<Core>>, decay: Decay) -> Result<DecayStatus, String> {
    let mut core = core.write().unwrap();
    core.set_decay(decay, now());
    save_owner(&app, &core)?;
    Ok(core.decay())
}

#[tauri::command]
fn explorar(core: tauri::State<RwLock<Core>>, filtro: Filtro, page: usize) -> ExplorarPage {
    core.read().unwrap().explorar(&filtro, page)
}

#[tauri::command]
fn inicio(core: tauri::State<RwLock<Core>>) -> Inicio {
    core.read().unwrap().inicio(now())
}

#[tauri::command]
fn estatisticas(core: tauri::State<RwLock<Core>>) -> Estatisticas {
    core.read().unwrap().estatisticas()
}

#[tauri::command]
fn minhas_notas(core: tauri::State<RwLock<Core>>) -> Vec<MinhaNota> {
    core.read().unwrap().minhas_notas()
}

#[tauri::command]
fn opcoes(core: tauri::State<RwLock<Core>>) -> Opcoes {
    core.read().unwrap().opcoes()
}

/// The data versions of the local catálogo and model; `None` before their first download.
/// Off the main thread: the UI's first call, it waits for the launch to load them.
#[tauri::command(async)]
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

/// Brings the catálogo up to date with the published one through its daily deltas, or the
/// full catálogo if they do not reach (the app was too long without updating). Only once
/// there is a catálogo: the first one is the setup's download, with its progress shown.
fn update_catalog(app: &AppHandle) -> Result<(), String> {
    let Some(mut version) = app.state::<RwLock<Core>>().read().unwrap().data_version().map(str::to_string) else { return Ok(()) };
    let Some(latest) = fetch(&catalog_release("catalog-version.txt"))? else { return Ok(()) };
    let latest = String::from_utf8_lossy(&latest).trim().to_string();
    if version == latest {
        return Ok(());
    }
    let mut json = std::fs::read(catalog_file(app)).map_err(|e| e.to_string())?;
    while version != latest {
        let Some(delta) = fetch(&catalog_release(&format!("catalog-delta-{version}.json.gz")))? else { break };
        json = cinelens_core::apply_catalog_delta(&json, &gunzip(&delta)?).map_err(|e| e.to_string())?;
        // ponytail: a full parse per delta just for its version; fine for the few days an app
        // falls behind, read the version alone if long chains get slow.
        let next = Core::open(&json).map_err(|e| e.to_string())?.data_version().unwrap_or_default().to_string();
        if next == version {
            break; // A delta that goes nowhere: fall back to the full catálogo.
        }
        version = next;
    }
    if version != latest {
        json = download(app, &catalog_url())?;
        Core::open(&json).map_err(|e| e.to_string())?;
    }
    write_atomically(&catalog_file(app), &json).map_err(|e| e.to_string())?;
    app.state::<RwLock<Core>>().write().unwrap().load_catalog(&json).map_err(|e| e.to_string())?;
    let _ = app.emit("atualizado", ());
    Ok(())
}

/// Downloads a newly published model and switches to it only if it predicts the owner's notas
/// no worse (the accuracy gate, in the core); a rejected version is not downloaded again. Only
/// once there is a model: the first one is downloaded by `download_model`.
fn update_model(app: &AppHandle) -> Result<(), String> {
    let Some(current) = app.state::<RwLock<Core>>().read().unwrap().model_version().map(str::to_string) else { return Ok(()) };
    let Some(latest) = fetch(&model_release("model-version.txt"))? else { return Ok(()) };
    let latest = String::from_utf8_lossy(&latest).trim().to_string();
    let rejected = std::fs::read(rejected_file(app)).ok().and_then(|b| serde_json::from_slice::<Rejeicao>(&b).ok());
    if latest == current || rejected.is_some_and(|r| r.data_version == latest) {
        return Ok(());
    }
    let json = download(app, &model_url())?;
    // ponytail: the accuracy gate runs under the write lock, freezing the UI for its two fold-ins;
    // measure on a second Core if that ever shows.
    let rejeicao = app.state::<RwLock<Core>>().write().unwrap().trocar_modelo(&json).map_err(|e| e.to_string())?;
    match rejeicao {
        Some(rejeicao) => {
            write_atomically(&rejected_file(app), &serde_json::to_vec(&rejeicao).unwrap()).map_err(|e| e.to_string())?
        }
        None => {
            write_atomically(&model_file(app), &json).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(rejected_file(app));
        }
    }
    let _ = app.emit("atualizado", ());
    Ok(())
}

/// GETs `url`; `None` on 404 (not published yet).
fn fetch(url: &str) -> Result<Option<Vec<u8>>, String> {
    let response = ureq::get(url).config().timeout_global(Some(Duration::from_secs(60))).build().call();
    match response {
        Ok(mut r) => r.body_mut().with_config().limit(u64::MAX).read_to_vec().map(Some).map_err(|e| e.to_string()),
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn gunzip(gz: &[u8]) -> Result<Vec<u8>, String> {
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(gz).read_to_end(&mut json).map_err(|e| e.to_string())?;
    Ok(json)
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
    gunzip(&gz)
}

/// Like `write_atomically`, readable by the owner's account only from the start.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("private.tmp");
    options.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(tmp, path)
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
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_opener::init()).plugin(tauri_plugin_notification::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(google::init());
    builder
        .setup(|app| {
            let mut core = Core::default();
            // Unlike the catálogo, owner data cannot be downloaded again: refuse to start over it.
            // Cheap here, with no catálogo or model to rank yet.
            match std::fs::read(owner_file(app.handle())) {
                Ok(bytes) => core.load_owner_data(&bytes)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
            app.manage(RwLock::new(core));
            // The catálogo and model take seconds to parse (tens in a debug build on a phone), and
            // the setup runs on the main thread, before the webview shows: load them behind the
            // lock instead, so `status` waits for them rather than seeing a first run.
            let handle = app.handle().clone();
            let (locked, wait) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                {
                    let state = handle.state::<RwLock<Core>>();
                    let mut core = state.write().unwrap();
                    let _ = locked.send(());
                    // A missing or unreadable catálogo means the UI downloads it again.
                    if let Ok(json) = std::fs::read(catalog_file(&handle)) {
                        let _ = core.load_catalog(&json);
                    }
                    // Same for the model.
                    if let Ok(json) = std::fs::read(model_file(&handle)) {
                        let _ = core.load_model(&json);
                    }
                }
                loop {
                    let updates = [("catálogo", update_catalog(&handle)), ("model", update_model(&handle))];
                    for (what, result) in updates.into_iter().chain([("notification", notify(&handle))]) {
                        if let Err(e) = result {
                            eprintln!("{what} update: {e}");
                        }
                    }
                    std::thread::sleep(UPDATE_EVERY);
                }
            });
            wait.recv()?;
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                schedule_sync(&handle);
                std::thread::sleep(SYNC_EVERY);
            });
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("poster", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let name = request.uri().path().trim_start_matches('/').to_string();
            tauri::async_runtime::spawn_blocking(move || responder.respond(poster(&app, &name)));
        })
        .invoke_handler(tauri::generate_handler![
            explorar,
            inicio,
            minhas_notas,
            opcoes,
            status,
            download_catalog,
            download_model,
            import_movielens,
            movie,
            rate,
            marcar,
            pessoa,
            set_fa,
            decay,
            set_decay,
            display_mode,
            set_display_mode,
            notificacao,
            set_notificacao,
            servicos,
            set_servicos,
            parecidos,
            parecidos_mas,
            set_pesou,
            pesou_oculto,
            set_pesou_oculto,
            rodada,
            nao_vi,
            estatisticas,
            google_entrar,
            google_conectado,
            modelo_rejeitado
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
