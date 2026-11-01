//! The system share sheet, for a movie's card and text. Android only: no desktop share sheet here.

#[cfg(target_os = "android")]
mod platform {
    use serde::Serialize;
    use tauri::plugin::PluginHandle;
    use tauri::{AppHandle, Manager, Runtime};

    struct Plugin<R: Runtime>(PluginHandle<R>);

    #[derive(Serialize)]
    struct Args {
        imagem: String,
        texto: String,
    }

    pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
        tauri::plugin::Builder::new("compartilhar")
            .setup(|app, api| {
                let handle = api.register_android_plugin("io.github.lucasjlbraz.cinelens", "CompartilharPlugin")?;
                app.manage(Plugin(handle));
                Ok(())
            })
            .build()
    }

    pub fn enviar<R: Runtime>(app: &AppHandle<R>, imagem: String, texto: String) -> Result<(), String> {
        let plugin = app.state::<Plugin<R>>();
        plugin.0.run_mobile_plugin::<()>("compartilhar", Args { imagem, texto }).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::{AppHandle, Runtime};

    pub fn enviar<R: Runtime>(_app: &AppHandle<R>, _imagem: String, _texto: String) -> Result<(), String> {
        Err("share is Android only".to_string())
    }
}

#[cfg(target_os = "android")]
pub use platform::init;
pub use platform::enviar;
