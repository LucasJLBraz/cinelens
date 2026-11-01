//! Which obra's notification the owner tapped. Android only: on the desktop a tap opens nothing.

#[cfg(target_os = "android")]
mod platform {
    use cinelens_core::ObraId;
    use serde::Deserialize;
    use tauri::plugin::PluginHandle;
    use tauri::{AppHandle, Manager, Runtime};

    struct Plugin<R: Runtime>(PluginHandle<R>);

    #[derive(Deserialize)]
    struct Tocada {
        obra: Option<ObraId>,
    }

    pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
        tauri::plugin::Builder::new("tocada")
            .setup(|app, api| {
                let handle = api.register_android_plugin("io.github.lucasjlbraz.cinelens", "TocadaPlugin")?;
                app.manage(Plugin(handle));
                Ok(())
            })
            .build()
    }

    pub fn obra<R: Runtime>(app: &AppHandle<R>) -> Result<Option<ObraId>, String> {
        let plugin = app.state::<Plugin<R>>();
        plugin.0.run_mobile_plugin::<Tocada>("tocada", ()).map(|t| t.obra).map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use cinelens_core::ObraId;
    use tauri::{AppHandle, Runtime};

    pub fn obra<R: Runtime>(_app: &AppHandle<R>) -> Result<Option<ObraId>, String> {
        Ok(None)
    }
}

#[cfg(target_os = "android")]
pub use platform::init;
pub use platform::obra;
