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

#[tauri::command]
fn project_create(app: tauri::AppHandle, name: String) -> Result<project_store::ProjectRecord, String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    project_store::create(&root, &name)
}

#[tauri::command]
fn project_list(app: tauri::AppHandle) -> Result<Vec<project_store::ProjectSummary>, String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    project_store::list(&root)
}

#[tauri::command]
fn project_open(app: tauri::AppHandle, id: String) -> Result<project_store::ProjectRecord, String> {
    let root = app.path().app_data_dir().map_err(|e| e.to_string())?;
    project_store::open(&root, &id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![runtime_status, project_create, project_list, project_open])
        .run(tauri::generate_context!())
        .expect("error while starting Optic Rift");
}
