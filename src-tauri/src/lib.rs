use serde::Serialize;
use tauri::Manager;

mod adapters;
mod generation;
mod gpu_guard;
mod hf_access;
mod hf_catalog;
mod model_commands;
mod model_store;
mod photo_export;
mod project_photos;
mod process_tree;
mod project_store;
mod runtime;
mod video_metadata_cache;

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

/// Managed project photos: native project ID and immutable source authority.
/// All potentially large decode/file operations run away from the UI thread.
#[tauri::command]
async fn project_photo_save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>, request: project_photos::SavePhotoRequest,
) -> Result<project_photos::PhotoDocument, String> {
    let root = store_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || project_photos::save(&root, request))
        .await.map_err(|_| "Project photo save task ended unexpectedly.".to_string())?
}

#[tauri::command]
async fn project_photo_list<R: tauri::Runtime>(
    app: tauri::AppHandle<R>, project_id: String,
) -> Result<project_photos::PhotoListing, String> {
    let root = store_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || project_photos::list(&root, &project_id))
        .await.map_err(|_| "Project photo listing task ended unexpectedly.".to_string())?
}

#[tauri::command]
async fn project_photo_open<R: tauri::Runtime>(
    app: tauri::AppHandle<R>, project_id: String, asset_id: String,
) -> Result<project_photos::OpenedPhoto, String> {
    let root = store_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || project_photos::open(&root, &project_id, &asset_id))
        .await.map_err(|_| "Project photo open task ended unexpectedly.".to_string())?
}

/// Explicit, CPU-only image export. No model/GPU, arbitrary output path, or
/// implicit cloud-sync permission. A native check repeats the client's consent.
#[tauri::command]
async fn photo_export_copy<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: photo_export::PhotoExportRequest,
) -> Result<photo_export::PhotoExportReceipt, String> {
    let pictures = generation::pictures_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        photo_export::export_to_pictures(&pictures, request)
    })
    .await
    .map_err(|_| "Photo export task ended unexpectedly.".to_string())?
}

/// Register the application's IPC commands. Shared by `run` and the IPC integration tests so
/// tests exercise exactly the production command set.
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        runtime_status,
        project_create,
        project_list,
        project_open,
        project_photo_save,
        project_photo_list,
        project_photo_open,
        hf_access::hf_token_status,
        hf_access::hf_token_save,
        hf_access::hf_token_remove,
        hf_access::hf_token_test,
        hf_access::hf_open_token_page,
        model_commands::catalog_capabilities,
        model_commands::catalog_search,
        model_commands::catalog_inspect,
        model_commands::video_cache_metadata,
        model_commands::video_cache_clear,
        model_commands::open_model_page,
        model_commands::models_library,
        model_commands::models_set_dir,
        model_commands::model_install,
        model_commands::model_install_cancel,
        model_commands::model_install_status,
        model_commands::model_remove,
        model_commands::image_runtime_install,
        model_commands::generation_generate,
        model_commands::generation_cancel,
        model_commands::generation_unload,
        model_commands::generation_outputs,
        model_commands::generation_output_image,
        model_commands::generation_output_thumbnail,
        model_commands::generation_save,
        photo_export_copy,
        model_commands::gpu_check
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    with_commands(tauri::Builder::default().plugin(tauri_plugin_opener::init()))
        .build(tauri::generate_context!())
        .expect("error while starting Optic Rift")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Stop the GPU worker so it never outlives the app.
                generation::shutdown();
            }
        });
}
