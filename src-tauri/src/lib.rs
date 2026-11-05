use serde::Serialize;
use tauri::Manager;

mod project_store;

#[derive(Serialize)]
struct RuntimeStatus {
    application: &'static str,
    stage: &'static str,
    generation_ready: bool,
    project_storage_ready: bool,
}

#[tauri::command]
fn runtime_status() -> RuntimeStatus {
    RuntimeStatus {
        application: "Optic Rift",
        stage: "pre-alpha architecture spike",
        generation_ready: false,
        project_storage_ready: false,
    }
}

fn store_root<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Cannot resolve application data directory: {e}"))
}

// Project commands are async so Tauri runs their file I/O off the main (UI) thread.

#[tauri::command]
async fn project_create<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    name: String,
) -> Result<project_store::ProjectRecord, String> {
    project_store::create(&store_root(&app)?, &name)
}

#[tauri::command]
async fn project_list<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<project_store::ProjectListing, String> {
    project_store::list(&store_root(&app)?)
}

#[tauri::command]
async fn project_open<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    id: String,
) -> Result<project_store::ProjectRecord, String> {
    project_store::open(&store_root(&app)?, &id)
}

/// Register the application's IPC commands. Shared by `run` and the IPC integration tests so
/// tests exercise exactly the production command set.
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        runtime_status,
        project_create,
        project_list,
        project_open
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    with_commands(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while starting Optic Rift");
}
