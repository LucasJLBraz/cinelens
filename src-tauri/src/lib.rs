//! Thin Tauri adapters over the core facade. No logic lives here.

use cinelens_core::{Core, ExplorarPage};

// ponytail: fixture catálogo baked into the app until the real download lands (#3).
const CATALOG: &[u8] = include_bytes!("../../core/fixtures/catalog.json");

#[tauri::command]
fn explorar(core: tauri::State<Core>, page: usize) -> ExplorarPage {
    core.explorar(page)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Core::open(CATALOG).expect("fixture catálogo is valid"))
        .invoke_handler(tauri::generate_handler![explorar])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
