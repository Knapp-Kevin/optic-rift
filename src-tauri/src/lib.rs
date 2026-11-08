use serde::Serialize;
use tauri::Manager;

mod adapters;
mod generation;
mod gpu_guard;
mod hf_access;
mod hf_catalog;
pub mod media;
mod model_commands;
mod model_store;
mod photo_export;
mod process_tree;
mod project_store;
mod runtime;
mod video_metadata_cache;
mod video_trim;

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

fn export_roots<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<video_trim::ExportRoots, String> {
    Ok(video_trim::ExportRoots {
        app_data: store_root(app)?,
        pictures: generation::pictures_dir(app)?,
        resource_dir: app.path().resource_dir().ok(),
    })
}

/// Debug builds only: lets automated Windows qualification answer the native file dialog,
/// which DevTools cannot click. Release builds always show the real dialog.
#[cfg(debug_assertions)]
fn qa_picked_video() -> Option<std::path::PathBuf> {
    std::env::var_os("OPTIC_RIFT_QA_PICK_VIDEO").map(std::path::PathBuf::from)
}
#[cfg(not(debug_assertions))]
fn qa_picked_video() -> Option<std::path::PathBuf> {
    None
}

/// Grant export access to the clip open in Video Studio. The user picks the file in the native
/// dialog; the WebView never supplies a path. `Ok(None)` means the dialog was cancelled.
#[tauri::command]
async fn video_source_grant<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: video_trim::GrantRequest,
) -> Result<Option<video_trim::GrantInfo>, String> {
    video_trim::validate_grant_request(&request)?;
    let roots = export_roots(&app)?;
    #[cfg(windows)]
    let owner = app
        .get_webview_window("main")
        .and_then(|window| window.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize)
        .unwrap_or(0);
    #[cfg(not(windows))]
    let owner = 0isize;
    tauri::async_runtime::spawn_blocking(move || {
        let trusted = video_trim::trusted_tools(roots.resource_dir.as_deref())?;
        let picked = match qa_picked_video() {
            Some(path) => Some(path),
            None => video_trim::pick_video_file(owner, &request.name)?,
        };
        match picked {
            None => Ok(None),
            Some(path) => video_trim::grant_picked(&trusted, &request, &path).map(Some),
        }
    })
    .await
    .map_err(|_| "The source check ended unexpectedly.".to_string())?
}

#[tauri::command]
fn video_source_revoke(grant_id: String) -> bool {
    video_trim::revoke(&grant_id)
}

#[tauri::command]
async fn video_trim_resolve(
    request: video_trim::TrimRequest,
) -> Result<video_trim::TrimPlan, String> {
    video_trim::resolve(&request)
}

#[tauri::command]
async fn video_trim_start<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: video_trim::StartRequest,
) -> Result<video_trim::JobStatus, String> {
    let roots = export_roots(&app)?;
    tauri::async_runtime::spawn_blocking(move || video_trim::start(&roots, request))
        .await
        .map_err(|_| "The export could not be started.".to_string())?
}

#[tauri::command]
fn video_trim_status(job_id: String) -> Option<video_trim::JobStatus> {
    video_trim::status(&job_id)
}

#[tauri::command]
fn video_trim_cancel(job_id: String) -> Result<video_trim::JobStatus, String> {
    video_trim::cancel(&job_id)
}

#[tauri::command]
fn video_trim_jobs() -> Vec<video_trim::JobStatus> {
    video_trim::list()
}

/// Show a finished export in File Explorer. The path comes from the job, never the WebView.
#[tauri::command]
fn video_export_reveal<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    job_id: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = video_trim::published_video(&job_id).ok_or("That export is not available.")?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|_| "Could not open File Explorer.".to_string())
}

/// Register the application's IPC commands. Shared by `run` and the IPC integration tests so
/// tests exercise exactly the production command set.
pub fn with_commands<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        runtime_status,
        project_create,
        project_list,
        project_open,
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
        video_source_grant,
        video_source_revoke,
        video_trim_resolve,
        video_trim_start,
        video_trim_status,
        video_trim_cancel,
        video_trim_jobs,
        video_export_reveal,
        model_commands::gpu_check
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    with_commands(tauri::Builder::default().plugin(tauri_plugin_opener::init()))
        .setup(|app| {
            // Mark exports that were running when the app last stopped, and delete their partials.
            if let Ok(roots) = export_roots(app.handle()) {
                video_trim::reconcile(&roots);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while starting Optic Rift")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Stop the GPU worker so it never outlives the app.
                generation::shutdown();
            }
        });
}
