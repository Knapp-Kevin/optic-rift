//! IPC commands for model discovery, installation, the image runtime and local generation.
//! None of these commands accepts arbitrary filesystem paths except the explicit model-folder
//! setting, and none returns credentials.

use crate::adapters::{self, Capability};
use crate::generation::{self, GenerateRequest, GenerationMode, GpuInfo, Provenance};
use crate::hf_catalog::{self, ModelDetail, SearchResult};
use crate::model_store::{self, InstallRequest, InstalledModel, Manifest};
use crate::runtime::{self, RuntimeManifest, RuntimeStatus};
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
        installed: model_store::find_installed(&models, &detail.id, &detail.revision),
        gpu: generation::gpu_info(),
        runtime_installed: runtime::manifest(&runtime::root(&local)).is_some(),
        free_bytes: model_store::free_bytes(&models).or_else(|| model_store::free_bytes(&local)),
        model_dir: models,
        detail,
    })
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
    Ok(ModelLibrary {
        free_bytes: model_store::free_bytes(&models).or_else(|| model_store::free_bytes(&local)),
        installed: model_store::list_installed(&models),
        loaded: generation::loaded_model(),
        runtime: runtime::status(&local),
        gpu: generation::gpu_info(),
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

#[tauri::command]
pub async fn generation_save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    job_id: String,
) -> Result<PathBuf, String> {
    let (_, local) = model_store::dirs(&app)?;
    generation::save_copy(&local, &generation::pictures_dir(&app)?, &job_id)
}

#[derive(Serialize)]
pub struct GpuCheck {
    snapshot: crate::gpu_guard::GpuSnapshot,
    decision: crate::gpu_guard::Decision,
    need_mb: u64,
    idle_unload_seconds: u64,
}

/// Read-only check of who else is using the GPU, before the user starts a generation.
#[tauri::command]
pub async fn gpu_check<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    repo: String,
    revision: String,
    width: u32,
    height: u32,
    mode: Option<GenerationMode>,
    source_job_id: Option<String>,
    strength: Option<f32>,
) -> Result<GpuCheck, String> {
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
