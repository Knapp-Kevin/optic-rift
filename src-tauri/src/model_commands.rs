//! IPC commands for model discovery, installation, the image runtime and local generation.
//! None of these commands accepts arbitrary filesystem paths except the explicit model-folder
//! setting, and none returns credentials.

use crate::adapters::{self, Capability};
use crate::generation::{self, GenerateRequest, GenerationMode, GpuInfo, Provenance};
use crate::hf_catalog::{self, ModelDetail, SearchResult};
use crate::model_store::{self, InstallRequest, InstalledModel, Manifest};
use crate::runtime::{self, RuntimeManifest, RuntimeStatus};
use crate::video_metadata_cache::{self, CacheReceipt, CacheRequest};
use base64::Engine;
use serde::Serialize;
use std::path::PathBuf;
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
pub struct CapabilityInfo {
    id: Capability,
    label: &'static str,
    adapters: Vec<&'static str>,
}

#[tauri::command]
pub fn catalog_capabilities() -> Vec<CapabilityInfo> {
    Capability::ALL
        .iter()
        .map(|&c| CapabilityInfo {
            id: c,
            label: c.label(),
            adapters: adapters::adapters_for(c).map(|a| a.label).collect(),
        })
        .collect()
}

#[tauri::command]
pub async fn catalog_search(
    query: String,
    capability: Capability,
    include_adult: bool,
) -> Result<SearchResult, String> {
    hf_catalog::search(&query, capability, include_adult, 40).await
}

#[derive(Serialize)]
pub struct Inspection {
    detail: ModelDetail,
    cached_metadata: bool,
    installed: Option<InstalledModel>,
    gpu: GpuInfo,
    runtime_installed: bool,
    model_dir: PathBuf,
    free_bytes: Option<u64>,
}

#[tauri::command]
pub async fn catalog_inspect<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    repo: String,
    revision: Option<String>,
    capability: Capability,
) -> Result<Inspection, String> {
    let detail =
        hf_catalog::inspect(&repo, revision.as_deref().unwrap_or("main"), capability).await?;
    let (data, local) = model_store::dirs(&app)?;
    let models = model_store::model_dir(&data, &local);
    Ok(Inspection {
        cached_metadata: video_metadata_cache::cached_receipt(&local, &detail).is_some(),
        installed: model_store::find_installed(&models, &detail.id, &detail.revision),
        gpu: generation::gpu_info(),
        runtime_installed: runtime::manifest(&runtime::root(&local)).is_some(),
        free_bytes: model_store::free_bytes(&models).or_else(|| model_store::free_bytes(&local)),
        model_dir: models,
        detail,
    })
}

/// Download and verify one inert public video configuration at an immutable source SHA.
/// This does not install any weights or register an inference adapter.
#[tauri::command]
pub async fn video_cache_metadata<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: CacheRequest,
) -> Result<CacheReceipt, String> {
    let (_, local) = model_store::dirs(&app)?;
    video_metadata_cache::cache_public_metadata(local, request).await
}

/// Clear only the receipt and JSON configuration belonging to one pinned model.
#[tauri::command]
pub async fn video_cache_clear<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    repo: String,
    revision: String,
    accept: bool,
) -> Result<(), String> {
    let (_, local) = model_store::dirs(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        video_metadata_cache::remove_metadata(&local, &repo, &revision, accept)
    })
    .await
    .map_err(|_| "Metadata cache cleanup stopped.".to_string())?
}

#[tauri::command]
pub fn open_model_page<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    repo: String,
) -> Result<(), String> {
    if !hf_catalog::valid_repo_id(&repo) {
        return Err("Invalid model identifier.".into());
    }
    app.opener()
        .open_url(format!("{}/{repo}", hf_catalog::HUB), None::<&str>)
        .map_err(|_| "Could not open your browser.".to_string())
}

#[derive(Serialize)]
pub struct ModelLibrary {
    model_dir: PathBuf,
    /// UI default only: execution is still gated separately at each native request.
    qualified_image_ready: bool,
    free_bytes: Option<u64>,
    installed: Vec<InstalledModel>,
    loaded: Option<(String, String)>,
    runtime: RuntimeStatus,
    gpu: GpuInfo,
}

#[tauri::command]
pub async fn models_library<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<ModelLibrary, String> {
    let (data, local) = model_store::dirs(&app)?;
    let models = model_store::model_dir(&data, &local);
    let installed = model_store::list_installed(&models);
    let image_runtime = runtime::status(&local);
    let gpu = generation::gpu_info();
    let qualified_image_ready = image_runtime.installed.is_some()
        && gpu.vram_total_mb.is_some()
        && installed.iter().any(|model| {
            model.files_present
                && model.manifest.adapter == "diffusers-sdxl-text-to-image"
                && adapters::verified_receipt(
                    &model.manifest.repo,
                    &model.manifest.revision,
                    &model.manifest.adapter,
                )
                .is_some()
        });
    Ok(ModelLibrary {
        free_bytes: model_store::free_bytes(&models).or_else(|| model_store::free_bytes(&local)),
        installed,
        loaded: generation::loaded_model(),
        runtime: image_runtime,
        gpu,
        qualified_image_ready,
        model_dir: models,
    })
}

#[tauri::command]
pub async fn models_set_dir<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    path: String,
) -> Result<PathBuf, String> {
    let (data, _) = model_store::dirs(&app)?;
    model_store::set_model_dir(&data, &path)
}

#[tauri::command]
pub async fn model_install<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: InstallRequest,
) -> Result<Manifest, String> {
    let (data, local) = model_store::dirs(&app)?;
    let models = model_store::model_dir(&data, &local);
    model_store::install(&app, &models, request).await
}

#[tauri::command]
pub fn model_install_cancel(install_id: String) -> bool {
    model_store::cancel(&install_id)
}

/// Read-only: the one active native model download, if any, so the UI can reconcile after a
/// remount or reload. In-memory only; nothing is persisted.
#[tauri::command]
pub fn model_install_status() -> Option<model_store::InstallStatus> {
    model_store::status()
}

#[tauri::command]
pub async fn model_remove<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    repo: String,
    revision: String,
) -> Result<(), String> {
    if generation::loaded_model() == Some((repo.clone(), revision.clone())) {
        generation::unload()?;
    }
    let (data, local) = model_store::dirs(&app)?;
    model_store::remove(&model_store::model_dir(&data, &local), &repo, &revision)
}

#[tauri::command]
pub async fn image_runtime_install<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    uv_sha256: String,
) -> Result<RuntimeManifest, String> {
    let (_, local) = model_store::dirs(&app)?;
    generation::shutdown();
    tauri::async_runtime::spawn_blocking(move || runtime::install(&app, &local, &uv_sha256))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn generation_generate<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: GenerateRequest,
) -> Result<Provenance, String> {
    generation::generate(app, request).await
}

#[tauri::command]
pub fn generation_cancel(job_id: String) -> Result<(), String> {
    generation::cancel(&job_id)
}

#[tauri::command]
pub async fn generation_unload() -> Result<serde_json::Value, String> {
    tauri::async_runtime::spawn_blocking(generation::unload)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn generation_outputs<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<Provenance>, String> {
    let (_, local) = model_store::dirs(&app)?;
    Ok(generation::list_outputs(&local))
}

#[tauri::command]
pub async fn generation_output_image<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    job_id: String,
) -> Result<String, String> {
    let (_, local) = model_store::dirs(&app)?;
    let png = generation::output_png(&local, &job_id)?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

/// Preview-only data, decoded and downscaled after the same source/provenance checks as Open.
/// No thumbnail file is stored on disk.
#[tauri::command]
pub async fn generation_output_thumbnail<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    job_id: String,
) -> Result<String, String> {
    let (_, local) = model_store::dirs(&app)?;
    let png = tauri::async_runtime::spawn_blocking(move || {
        generation::output_thumbnail_png(&local, &job_id)
    })
    .await
    .map_err(|_| "Thumbnail processing stopped unexpectedly.".to_string())??;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    ))
}

/// Bounded, read-only public disclosure preview. Not a C2PA signature check.
#[tauri::command]
pub async fn generation_provenance_preview<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    job_id: String,
) -> Result<serde_json::Value, String> {
    let (_, local) = model_store::dirs(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        generation::preview_public_disclosure(&local, &job_id)
    })
    .await
    .map_err(|_| "Provenance inspection ended unexpectedly.".to_string())?
}

#[tauri::command]
pub async fn generation_save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    job_id: String,
) -> Result<PathBuf, String> {
    let (_, local) = model_store::dirs(&app)?;
    let pictures = generation::pictures_dir(&app)?;
    // Up to sixteen validated ancestry records may be inspected. Do not run
    // that bounded filesystem traversal on Tauri's async reactor thread.
    tauri::async_runtime::spawn_blocking(move || {
        generation::save_copy(&local, &pictures, &job_id)
    })
    .await
    .map_err(|_| "Generation export ended unexpectedly.".to_string())?
}

#[derive(Serialize)]
pub struct GpuCheck {
    snapshot: crate::gpu_guard::GpuSnapshot,
    decision: crate::gpu_guard::Decision,
    need_mb: u64,
    idle_unload_seconds: u64,
}

/// Read-only check of who else is using the GPU, before the user starts a generation.
// Each argument is a named IPC field the Studio already sends; bundling them into a struct would
// change the frontend contract, so the argument count is accepted here.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn gpu_check<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    repo: String,
    revision: String,
    width: u32,
    height: u32,
    steps: u32,
    mode: Option<GenerationMode>,
    source_job_id: Option<String>,
    strength: Option<f32>,
) -> Result<GpuCheck, String> {
    generation::validate_image_to_image_steps(mode.unwrap_or_default(), steps, strength)?;
    let (data, local) = model_store::dirs(&app)?;
    let mode = mode.unwrap_or_default();
    let model =
        model_store::find_installed(&model_store::model_dir(&data, &local), &repo, &revision)
            .ok_or("That model is not installed.")?;
    if mode == GenerationMode::ImageToImage
        && model.manifest.adapter != "diffusers-sdxl-text-to-image"
    {
        return Err("Image-to-image currently requires an installed SDXL model.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        // Hashing/reading the optional local reference must stay off the UI runtime.
        generation::preflight_image_reference(
            &local,
            mode,
            source_job_id.as_deref(),
            strength,
            width,
            height,
        )?;
        let (snapshot, decision, need_mb) =
            generation::gpu_decision(model.manifest.total_bytes, width, height, mode, &[]);
        Ok(GpuCheck {
            snapshot,
            decision,
            need_mb,
            idle_unload_seconds: generation::IDLE_UNLOAD.as_secs(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
