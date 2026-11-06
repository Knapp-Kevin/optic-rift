//! Supervises the isolated Python worker and runs local generations.
//!
//! The worker runs offline (no network, no tokens in its environment) from the app-owned runtime.
//! A generation produces a PNG plus a provenance record in Optic Rift's output folder; the image
//! is re-hashed and its PNG header checked here before success is reported.

use crate::gpu_guard;
use crate::model_store::{self, InstalledModel};
use crate::runtime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

struct Worker {
    child: Mutex<Child>,
    /// Kill-on-close job holding the launcher and the real interpreter it starts.
    tree: crate::process_tree::ProcessTree,
    stdin: Mutex<ChildStdin>,
    events: Mutex<Receiver<Value>>,
    alive: Arc<AtomicBool>,
    loaded: Mutex<Option<(String, String)>>,
    busy: AtomicBool,
    last_used: Mutex<Instant>,
}

/// Unload the model from the GPU after this long without use, so games and other apps get the
/// memory back without the user having to remember.
pub const IDLE_UNLOAD: Duration = Duration::from_secs(5 * 60);

fn slot() -> &'static Mutex<Option<Arc<Worker>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<Worker>>>> = OnceLock::new();
    SLOT.get_or_init(Default::default)
}

fn worker_command(root: &Path) -> Result<Command, String> {
    let py = runtime::python(root);
    if !py.is_file() {
        return Err("The image runtime is not installed.".into());
    }
    let script = runtime::ensure_worker_script(root)?;
    let mut command = Command::new(py);
    command
        .arg("-I")
        .arg(script)
        .current_dir(root)
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1")
        .env("HF_DATASETS_OFFLINE", "1")
        .env("HF_HUB_DISABLE_TELEMETRY", "1")
        .env("DO_NOT_TRACK", "1")
        .env("PYTHONNOUSERSITE", "1")
        .env("HF_HOME", root.join("hf-home"))
        .env_remove("HF_TOKEN")
        .env_remove("HUGGING_FACE_HUB_TOKEN")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    Ok(command)
}

/// Start the worker; every protocol event is forwarded to the UI as `generation` and queued here.
fn spawn<R: tauri::Runtime>(
    app: Option<tauri::AppHandle<R>>,
    root: &Path,
) -> Result<Arc<Worker>, String> {
    let mut child = worker_command(root)?
        .spawn()
        .map_err(|e| format!("Could not start the image worker: {e}"))?;
    let tree = match crate::process_tree::ProcessTree::adopt(&child) {
        Ok(tree) => tree,
        Err(e) => {
            let _ = child.kill();
            return Err(e);
        }
    };
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let log_path = root.join("worker-stderr.log");
    std::thread::spawn(move || {
        // Keep a bounded diagnostic log; prompts and tokens are never written by the worker to stderr.
        if let Ok(mut log) = fs::File::create(&log_path) {
            for line in BufReader::new(stderr)
                .lines()
                .map_while(Result::ok)
                .take(5000)
            {
                let _ = writeln!(log, "{line}");
            }
        }
    });
    let (tx, rx) = mpsc::channel();
    let alive = Arc::new(AtomicBool::new(true));
    let alive2 = alive.clone();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if let Some(app) = &app {
                let _ = app.emit("generation", &event);
            }
            if tx.send(event).is_err() {
                break;
            }
        }
        alive2.store(false, Ordering::SeqCst);
        let exited = json!({"event": "worker-exited"});
        if let Some(app) = &app {
            let _ = app.emit("generation", &exited);
        }
        let _ = tx.send(exited);
    });
    let worker = Worker {
        child: Mutex::new(child),
        tree,
        stdin: Mutex::new(stdin),
        events: Mutex::new(rx),
        alive,
        loaded: Mutex::new(None),
        busy: AtomicBool::new(false),
        last_used: Mutex::new(Instant::now()),
    };
    let ready = worker.wait_for(|e| e["event"] == "ready", Duration::from_secs(60))?;
    if ready["protocol"] != 1 {
        return Err("The image worker speaks an unexpected protocol version.".into());
    }
    Ok(Arc::new(worker))
}

impl Worker {
    fn send(&self, request: &Value) -> Result<(), String> {
        let mut stdin = self.stdin.lock().unwrap();
        writeln!(stdin, "{request}")
            .and_then(|_| stdin.flush())
            .map_err(|_| "The image worker is not running.".to_string())
    }

    /// Wait for an event matching `done`. Worker errors and exits end the wait.
    fn wait_for(&self, done: impl Fn(&Value) -> bool, timeout: Duration) -> Result<Value, String> {
        let events = self.events.lock().unwrap();
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match events.recv_timeout(left) {
                Ok(e) if done(&e) => return Ok(e),
                Ok(e) if e["event"] == "error" && e["job_id"].is_null() => {
                    return Err(e["message"].as_str().unwrap_or("Worker error").to_string())
                }
                Ok(e) if e["event"] == "worker-exited" => return Err(
                    "The image worker stopped unexpectedly. It will restart on the next request."
                        .into(),
                ),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    return Err("The image worker did not respond in time.".into())
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("The image worker stopped.".into())
                }
            }
        }
    }

    /// End the worker and every process it started (not just the venv launcher).
    fn kill(&self) {
        self.tree.terminate();
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.alive.store(false, Ordering::SeqCst);
    }

    /// Ask the worker to exit, wait briefly for a clean exit, then end the whole tree regardless.
    fn stop(&self) {
        let _ = self.send(&json!({"op": "shutdown"}));
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if matches!(self.child.lock().unwrap().try_wait(), Ok(Some(_))) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.kill();
    }
}

fn worker<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    root: &Path,
) -> Result<Arc<Worker>, String> {
    let mut guard = slot().lock().unwrap();
    if let Some(w) = guard.as_ref().filter(|w| w.alive.load(Ordering::SeqCst)) {
        return Ok(w.clone());
    }
    let w = spawn(Some(app.clone()), root)?;
    *guard = Some(w.clone());
    start_idle_watcher(app.clone());
    Ok(w)
}

fn start_idle_watcher<R: tauri::Runtime>(app: tauri::AppHandle<R>) {
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(30));
        let current = slot().lock().unwrap().clone();
        let Some(w) = current else { continue };
        let idle = w.last_used.lock().unwrap().elapsed();
        if idle >= IDLE_UNLOAD
            && !w.busy.load(Ordering::SeqCst)
            && w.loaded.lock().unwrap().is_some()
            && unload().is_ok()
        {
            let _ = app.emit(
                "generation",
                json!({"event": "auto-unloaded", "idle_seconds": idle.as_secs()}),
            );
        }
    });
}

/// One-shot health probe used by the runtime installer.
pub fn health_once(root: &Path) -> Result<Value, String> {
    let w = spawn::<tauri::Wry>(None, root)?;
    let result = w
        .send(&json!({"op": "health"}))
        .and_then(|_| w.wait_for(|e| e["event"] == "health", Duration::from_secs(600)));
    w.stop();
    result
}

pub fn shutdown() {
    if let Some(w) = slot().lock().unwrap().take() {
        w.stop();
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GpuInfo {
    pub name: Option<String>,
    pub driver: Option<String>,
    pub vram_total_mb: Option<u64>,
    pub vram_used_mb: Option<u64>,
    pub vram_free_mb: Option<u64>,
}

/// Read NVIDIA GPU state with nvidia-smi; empty when unavailable (no guessing).
pub fn gpu_info() -> GpuInfo {
    let mut command = Command::new("nvidia-smi");
    command.args([
        "--query-gpu=name,driver_version,memory.total,memory.used,memory.free",
        "--format=csv,noheader,nounits",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let Ok(out) = command.output() else {
        return GpuInfo::default();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let Some(line) = text.lines().next() else {
        return GpuInfo::default();
    };
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() != 5 {
        return GpuInfo::default();
    }
    GpuInfo {
        name: Some(parts[0].into()),
        driver: Some(parts[1].into()),
        vram_total_mb: parts[2].parse().ok(),
        vram_used_mb: parts[3].parse().ok(),
        vram_free_mb: parts[4].parse().ok(),
    }
}

/// Estimated total VRAM for a job: model weights plus working memory that grows with image area.
/// Calibrated from a measured run (SSD-1B fp16 at 1024x1024 peaked at 8,349 MB allocated:
/// 4,261 MB weights + about 4,088 MB working memory per megapixel).
pub fn job_need_mb(weights_bytes: u64, width: u32, height: u32) -> u64 {
    let pixels = (width as u64 * height as u64).max(1);
    let workspace = (4096 * pixels / (1024 * 1024)).max(1536);
    weights_bytes / (1024 * 1024) + workspace
}

/// Until an exact-machine GPU measurement exists for img2img, conservatively
/// reserve 2 GiB beyond the measured text-to-image estimate. This is a
/// preflight estimate, never a hard system-wide VRAM cap or qualification.
pub fn job_need_for_mode(weights_bytes: u64, width: u32, height: u32, mode: GenerationMode) -> u64 {
    let base = job_need_mb(weights_bytes, width, height);
    match mode {
        GenerationMode::TextToImage => base,
        GenerationMode::ImageToImage => base.saturating_add(2048),
    }
}

/// Check the GPU is free enough to use right now, considering every other process.
pub fn gpu_decision(
    weights_bytes: u64,
    width: u32,
    height: u32,
    mode: GenerationMode,
    acknowledged: &[u32],
) -> (gpu_guard::GpuSnapshot, gpu_guard::Decision, u64) {
    let snapshot = gpu_guard::snapshot();
    let need = job_need_for_mode(weights_bytes, width, height, mode);
    let decision = gpu_guard::evaluate(&snapshot, need, acknowledged);
    (snapshot, decision, need)
}

fn gpu_budget(model: &InstalledModel, req: &GenerateRequest) -> Result<u64, String> {
    match gpu_decision(
        model.manifest.total_bytes,
        req.width,
        req.height,
        req.mode,
        &req.gpu_acknowledged,
    )
    .1
    {
        gpu_guard::Decision::Proceed { budget_mb } => Ok(budget_mb),
        gpu_guard::Decision::Confirm { message, .. } => Err(format!("GPU_BUSY: {message}")),
        gpu_guard::Decision::Block { message } => Err(message),
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GenerationMode {
    #[default]
    TextToImage,
    ImageToImage,
}

impl GenerationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TextToImage => "text-to-image",
            Self::ImageToImage => "image-to-image",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GenerateRequest {
    pub job_id: String,
    pub repo: String,
    pub revision: String,
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    pub width: u32,
    pub height: u32,
    pub steps: u32,
    pub guidance: f32,
    pub seed: Option<u64>,
    #[serde(default)]
    pub mode: GenerationMode,
    #[serde(default)]
    pub source_job_id: Option<String>,
    #[serde(default)]
    pub strength: Option<f32>,
    pub project_id: Option<String>,
    /// PIDs of other GPU apps the user explicitly agreed to share the GPU with for this job.
    #[serde(default)]
    pub gpu_acknowledged: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    pub record_version: u32,
    pub job_id: String,
    pub created_unix_ms: u64,
    pub prompt: String,
    pub negative_prompt: String,
    pub parameters: Value,
    pub model: Value,
    pub runtime: Value,
    pub output: Value,
    pub metrics: Value,
    pub project_id: Option<String>,
    #[serde(default)]
    pub source: Option<Value>,
    pub lineage_note: String,
}

fn valid_job_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|u| u.hyphenated().to_string() == id)
}

pub fn outputs_dir(app_local: &Path) -> PathBuf {
    app_local.join("outputs")
}

/// Path and digest constructed only from verified app-owned output history.
/// The browser never supplies a reference path or reference SHA to the worker.
#[derive(Debug, Clone, Serialize)]
struct ImageReference {
    job_id: String,
    path: PathBuf,
    sha256: String,
}

/// JSON number for an f32 setting using its shortest decimal form, so a slider value of 0.65 is
/// sent and recorded as 0.65 rather than the widened 0.6499999761581421.
fn decimal_f32(value: Option<f32>) -> Value {
    value
        .filter(|v| v.is_finite())
        .and_then(|v| v.to_string().parse::<f64>().ok())
        .map_or(Value::Null, Value::from)
}

/// SDXL img2img truncates steps * strength to an integer. Reject a zero-step
/// combination before model loading or starting a GPU job. This is independent
/// of source-file integrity validation and does not change text-to-image.
pub(crate) fn validate_image_to_image_steps(
    mode: GenerationMode,
    steps: u32,
    strength: Option<f32>,
) -> Result<(), String> {
    if mode == GenerationMode::ImageToImage
        && strength
            .is_some_and(|value| value.is_finite() && (f64::from(value) * f64::from(steps)) < 1.0)
    {
        return Err("Increase Steps or image strength to run at least one denoising step.".into());
    }
    Ok(())
}

pub(crate) fn preflight_image_reference(
    app_local: &Path,
    mode: GenerationMode,
    source_job_id: Option<&str>,
    strength: Option<f32>,
    width: u32,
    height: u32,
) -> Result<(), String> {
    // Preflight from a UI action never produces or allocates a generation ID.
    verified_image_reference(app_local, mode, source_job_id, strength, width, height, "")
        .map(|_| ())
}

fn verified_image_reference(
    app_local: &Path,
    mode: GenerationMode,
    source_job_id: Option<&str>,
    strength: Option<f32>,
    width: u32,
    height: u32,
    output_job_id: &str,
) -> Result<Option<ImageReference>, String> {
    match mode {
        GenerationMode::TextToImage => {
            if source_job_id.is_some() || strength.is_some() {
                return Err(
                    "Text-to-image requests must not include a source or image strength.".into(),
                );
            }
            Ok(None)
        }
        GenerationMode::ImageToImage => {
            let id = source_job_id.ok_or("Choose a previous local generation first.")?;
            if !valid_job_id(id) || id == output_job_id {
                return Err("Choose a valid, different source generation.".into());
            }
            if !strength.is_some_and(|s| s.is_finite() && (0.05..=0.95).contains(&s)) {
                return Err("Image strength must be between 0.05 and 0.95.".into());
            }
            let (png, record) = verified_output(app_local, id)?;
            if png.len() > 32 * 1024 * 1024 || png_size(&png) != Some((width, height)) {
                return Err(
                    "The local source must be at most 32 MB and match the generation dimensions."
                        .into(),
                );
            }
            let provenance: Provenance = serde_json::from_slice(&record)
                .map_err(|_| "The source provenance is invalid.".to_string())?;
            let sha256 = provenance.output["sha256"]
                .as_str()
                .ok_or("The source record has no image digest.")?
                .to_string();
            Ok(Some(ImageReference {
                job_id: id.into(),
                path: outputs_dir(app_local).join(format!("{id}.png")),
                sha256,
            }))
        }
    }
}

/// Parse width/height from a PNG IHDR chunk.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some((
        u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    ))
}

pub async fn generate<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    req: GenerateRequest,
) -> Result<Provenance, String> {
    tauri::async_runtime::spawn_blocking(move || generate_blocking(&app, req))
        .await
        .map_err(|e| e.to_string())?
}

fn generate_blocking<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    req: GenerateRequest,
) -> Result<Provenance, String> {
    if !valid_job_id(&req.job_id) {
        return Err("Invalid job id.".into());
    }
    if let Some(p) = &req.project_id {
        if !valid_job_id(p) {
            return Err("Invalid project id.".into());
        }
    }
    validate_image_to_image_steps(req.mode, req.steps, req.strength)?;
    let (data, local) = model_store::dirs(app)?;
    // Fail closed before loading the GPU worker on malformed or mismatched sources.
    verified_image_reference(
        &local,
        req.mode,
        req.source_job_id.as_deref(),
        req.strength,
        req.width,
        req.height,
        &req.job_id,
    )?;
    let models = model_store::model_dir(&data, &local);
    let installed: InstalledModel = model_store::find_installed(&models, &req.repo, &req.revision)
        .ok_or("That model is not installed.")?;
    if !installed.files_present {
        return Err("The installed model is incomplete. Remove and reinstall it.".into());
    }
    if req.mode == GenerationMode::ImageToImage
        && installed.manifest.adapter != "diffusers-sdxl-text-to-image"
    {
        return Err("Image-to-image currently requires an installed SDXL model.".into());
    }
    let root = runtime::root(&local);
    let rt = runtime::manifest(&root).ok_or("Install the image runtime first (Models page).")?;
    let w = worker(app, &root)?;
    if w.busy.swap(true, Ordering::SeqCst) {
        return Err("A generation is already running.".into());
    }
    let result = run_job(app, &w, &installed, &rt, &local, &req);
    w.busy.store(false, Ordering::SeqCst);
    forget_cancel(&req.job_id);
    result
}

fn run_job<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    w: &Worker,
    model: &InstalledModel,
    rt: &runtime::RuntimeManifest,
    local: &Path,
    req: &GenerateRequest,
) -> Result<Provenance, String> {
    let key = (model.manifest.repo.clone(), model.manifest.revision.clone());
    let mut load_seconds = Value::Null;
    *w.last_used.lock().unwrap() = Instant::now();
    // Re-checked for every job: a game may have started since the model was loaded. This early
    // check fails closed before any multi-GB verification; each later use re-checks again.
    gpu_budget(model, req)?;
    check_not_cancelled(&req.job_id)?;
    if w.loaded.lock().unwrap().as_ref() != Some(&key) {
        let _ = app.emit(
            "generation",
            json!({"event": "verifying-model", "job_id": req.job_id}),
        );
        model_store::verify_installed_files_until(model, &|| cancel_requested(&req.job_id))
            .map_err(|e| {
                if cancel_requested(&req.job_id) {
                    "Generation cancelled.".to_string()
                } else {
                    e
                }
            })?;
        // A Cancel during the multi-GB hash check must stop before anything reaches the GPU.
        check_not_cancelled(&req.job_id)?;
        // Verification can read gigabytes. The GPU sharing snapshot taken before
        // that disk I/O may now be stale, so re-check immediately before loading.
        let load_budget_mb = gpu_budget(model, req)?;
        let _ = app.emit(
            "generation",
            json!({"event": "loading", "job_id": req.job_id}),
        );
        w.send(&json!({"op": "load", "model_dir": model.dir, "adapter": model.manifest.adapter, "variant": model.manifest.variant, "vram_budget_mb": load_budget_mb}))?;
        let loaded = w.wait_for(
            |e| e["event"] == "loaded" || (e["event"] == "error" && e["op"] == "load"),
            Duration::from_secs(600),
        )?;
        if loaded["event"] == "error" {
            *w.loaded.lock().unwrap() = None;
            return Err(format!(
                "Could not load the model: {}",
                loaded["message"].as_str().unwrap_or("unknown error")
            ));
        }
        load_seconds = loaded["seconds"].clone();
        *w.loaded.lock().unwrap() = Some(key);
    }
    // A hash check before model load may be old by now; revalidate the source
    // immediately before sending the job. The worker independently hashes again.
    let reference = verified_image_reference(
        local,
        req.mode,
        req.source_job_id.as_deref(),
        req.strength,
        req.width,
        req.height,
        &req.job_id,
    )?;
    // GPU sharing state may change during the source hash/decode checks.
    let budget_mb = gpu_budget(model, req)?;
    check_not_cancelled(&req.job_id)?;
    let out_dir = outputs_dir(local);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let png = out_dir.join(format!("{}.png", req.job_id));
    w.send(&json!({
        "op": "generate", "job_id": req.job_id, "prompt": req.prompt, "negative_prompt": req.negative_prompt,
        "width": req.width, "height": req.height, "steps": req.steps, "guidance": req.guidance,
        "seed": req.seed, "output_path": png, "vram_budget_mb": budget_mb,
        "mode": req.mode.as_str(), "reference": reference.as_ref(), "strength": decimal_f32(req.strength),
    }))?;
    let job = req.job_id.clone();
    // The worker clears its cancel flag when a job starts and ignores a cancel for a job it has
    // not started yet. Re-send a pending cancel once the job reports progress, closing that race.
    let resent = AtomicBool::new(false);
    let end = w.wait_for(
        |e| {
            if e["job_id"] == job.as_str()
                && e["event"] == "progress"
                && cancel_requested(&job)
                && !resent.swap(true, Ordering::SeqCst)
            {
                let _ = w.send(&json!({"op": "cancel", "job_id": job}));
            }
            e["job_id"] == job.as_str()
                && matches!(e["event"].as_str(), Some("done" | "error" | "cancelled"))
        },
        Duration::from_secs(1800),
    )?;
    match end["event"].as_str() {
        Some("cancelled") => return Err("Generation cancelled.".into()),
        Some("error") => {
            let category = end["category"].as_str().unwrap_or("worker_error");
            return Err(match category {
                "out_of_memory" => end["message"]
                    .as_str()
                    .unwrap_or("Out of graphics memory.")
                    .to_string(),
                _ => format!(
                    "Generation failed: {}",
                    end["message"].as_str().unwrap_or("unknown error")
                ),
            });
        }
        _ => {}
    }
    // Independent validation of what the worker wrote.
    let bytes = fs::read(&png)
        .map_err(|_| "The worker reported success but no image was written.".to_string())?;
    let sha256: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if Some(sha256.as_str()) != end["sha256"].as_str() {
        let _ = fs::remove_file(&png);
        return Err("The generated image failed its integrity check and was discarded.".into());
    }
    if png_size(&bytes) != Some((req.width, req.height)) {
        let _ = fs::remove_file(&png);
        return Err("The generated image has unexpected dimensions and was discarded.".into());
    }
    let gpu = gpu_info();
    let provenance = Provenance {
        record_version: 1,
        job_id: req.job_id.clone(),
        created_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        prompt: req.prompt.trim().into(),
        negative_prompt: req.negative_prompt.trim().into(),
        parameters: json!({"width": req.width, "height": req.height, "steps": req.steps, "guidance": decimal_f32(Some(req.guidance)), "seed": end["seed"], "mode": req.mode.as_str(), "strength": decimal_f32(req.strength)}),
        model: json!({
            "repo": model.manifest.repo, "revision": model.manifest.revision, "adapter": model.manifest.adapter,
            "adapter_version": model.manifest.adapter_version, "variant": model.manifest.variant,
            "license": model.manifest.license, "license_acknowledged": model.manifest.consent.license_acknowledged,
            "weight_files": model.manifest.files.iter().map(|f| json!({"path": f.path, "sha256": f.sha256})).collect::<Vec<_>>(),
        }),
        runtime: json!({"id": rt.runtime, "python": rt.python, "torch": rt.torch, "diffusers": rt.diffusers, "device": rt.device, "driver": gpu.driver}),
        output: json!({"file": png.file_name().map(|n| n.to_string_lossy().to_string()), "sha256": sha256, "width": req.width, "height": req.height, "format": "png", "bytes": bytes.len()}),
        metrics: json!({"generation_seconds": end["seconds"], "load_seconds": load_seconds, "peak_vram_allocated_mb": end["peak_vram_mb"]}),
        project_id: req.project_id.clone(),
        source: reference.map(|r| json!({
            "role": "initial-image", "job_id": r.job_id, "sha256": r.sha256,
            "width": req.width, "height": req.height,
        })),
        lineage_note: "Stored in Optic Rift's local output folder. Not yet linked into project asset lineage (decision #5 pending).".into(),
    };
    let record = out_dir.join(format!("{}.json", req.job_id));
    fs::write(
        &record,
        serde_json::to_vec_pretty(&provenance).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(provenance)
}

/// Job ids the user cancelled. The Python worker only knows a job once `generate` is sent, so
/// without this a Cancel during model verification or loading was dropped and the job then ran
/// on the GPU anyway. `run_job` checks it before each expensive stage.
fn cancelled_jobs() -> &'static Mutex<Vec<String>> {
    static CANCELLED: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    CANCELLED.get_or_init(|| Mutex::new(Vec::new()))
}

fn cancel_requested(job_id: &str) -> bool {
    cancelled_jobs()
        .lock()
        .unwrap()
        .iter()
        .any(|id| id == job_id)
}

fn forget_cancel(job_id: &str) {
    cancelled_jobs().lock().unwrap().retain(|id| id != job_id);
}

fn check_not_cancelled(job_id: &str) -> Result<(), String> {
    if cancel_requested(job_id) {
        Err("Generation cancelled.".into())
    } else {
        Ok(())
    }
}

pub fn cancel(job_id: &str) -> Result<(), String> {
    if !valid_job_id(job_id) {
        return Err("Invalid job id.".into());
    }
    {
        let mut ids = cancelled_jobs().lock().unwrap();
        // Bounded: ids are removed when their job ends; this only drops stale unknown ids.
        if ids.len() >= 32 {
            ids.remove(0);
        }
        if !ids.iter().any(|id| id == job_id) {
            ids.push(job_id.to_string());
        }
    }
    // Forward to a running worker too, so an in-progress denoise stops at its next step.
    if let Some(w) = slot().lock().unwrap().clone() {
        w.send(&json!({"op": "cancel", "job_id": job_id}))?;
    }
    Ok(())
}

pub fn unload() -> Result<Value, String> {
    let w = slot()
        .lock()
        .unwrap()
        .clone()
        .ok_or("No model is loaded.")?;
    if w.busy.load(Ordering::SeqCst) {
        return Err("Wait for the current generation to finish.".into());
    }
    w.send(&json!({"op": "unload"}))?;
    let e = w.wait_for(|e| e["event"] == "unloaded", Duration::from_secs(60))?;
    *w.loaded.lock().unwrap() = None;
    Ok(e)
}

pub fn loaded_model() -> Option<(String, String)> {
    slot()
        .lock()
        .unwrap()
        .as_ref()
        .filter(|w| w.alive.load(Ordering::SeqCst))
        .and_then(|w| w.loaded.lock().unwrap().clone())
}

pub fn list_outputs(app_local: &Path) -> Vec<Provenance> {
    let root = outputs_dir(app_local);
    let Ok(metadata) = fs::symlink_metadata(&root) else {
        return Vec::new();
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() || output_is_reparse_point(&metadata)
    {
        return Vec::new();
    }
    let mut out: Vec<Provenance> = fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            let bytes = read_output_file(&e.path(), MAX_OUTPUT_RECORD_BYTES).ok()?;
            let record = serde_json::from_slice::<Provenance>(&bytes).ok()?;
            let expected_name = format!("{}.json", record.job_id);
            (valid_job_id(&record.job_id)
                && e.file_name().to_string_lossy().as_ref() == expected_name.as_str())
            .then_some(record)
        })
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.created_unix_ms));
    out.truncate(100);
    out
}

const MAX_OUTPUT_PNG_BYTES: u64 = 128 * 1024 * 1024;
const MAX_OUTPUT_RECORD_BYTES: u64 = 512 * 1024;

/// Read a local output without following direct file links or Windows reparse
/// points. Restrict read sizes even if a file grows after metadata inspection.
fn read_output_file(path: &Path, max_bytes: u64) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "A generated output file is missing.".to_string())?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || output_is_reparse_point(&metadata)
    {
        return Err("Generated output is a link or is not a regular file.".into());
    }
    if metadata.len() > max_bytes {
        return Err("Generated output exceeds the safe local read limit.".into());
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(max_bytes + 1).read_to_end(&mut bytes))
        .map_err(|_| "Could not read generated output.".to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err("Generated output exceeds the safe local read limit.".into());
    }
    Ok(bytes)
}

#[cfg(windows)]
fn output_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn output_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

/// Reopening and exporting share one integrity boundary. The manifest itself
/// lives alongside the image, so this detects accidental/stale modification,
/// not coordinated edits to both files by a local attacker.
fn verified_output(app_local: &Path, job_id: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
    if !valid_job_id(job_id) {
        return Err("Invalid output id.".into());
    }
    let root = outputs_dir(app_local);
    let meta = fs::symlink_metadata(&root)
        .map_err(|_| "The local output folder does not exist.".to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() || output_is_reparse_point(&meta) {
        return Err("The local output folder is redirected or invalid.".into());
    }
    let png = read_output_file(&root.join(format!("{job_id}.png")), MAX_OUTPUT_PNG_BYTES)?;
    let record = read_output_file(
        &root.join(format!("{job_id}.json")),
        MAX_OUTPUT_RECORD_BYTES,
    )?;
    let provenance: Provenance = serde_json::from_slice(&record)
        .map_err(|_| "The output's provenance record is invalid.".to_string())?;
    let actual_digest: String = Sha256::digest(&png)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let dimensions = png_size(&png);
    if provenance.record_version != 1
        || provenance.job_id != job_id
        || provenance.output["sha256"].as_str() != Some(actual_digest.as_str())
        || dimensions.is_none()
        || provenance.output["width"].as_u64() != dimensions.map(|d| d.0 as u64)
        || provenance.output["height"].as_u64() != dimensions.map(|d| d.1 as u64)
        || provenance.output["bytes"].as_u64() != Some(png.len() as u64)
    {
        return Err("The image does not match its local provenance record.".into());
    }
    Ok((png, record))
}

pub fn output_png(app_local: &Path, job_id: &str) -> Result<Vec<u8>, String> {
    verified_output(app_local, job_id).map(|(png, _)| png)
}

/// Return a bounded, freshly decoded thumbnail of a verified local generation.
///
/// No derivative is written to disk: this avoids creating an unprotected cache
/// as future private-output modes are added. A caller can cache only the data URI
/// in its current WebView session. Every request revalidates the original pair.
pub fn output_thumbnail_png(app_local: &Path, job_id: &str) -> Result<Vec<u8>, String> {
    let original = output_png(app_local, job_id)?;
    thumbnail_png(&original)
}

/// Pure, CPU-only PNG decoder and downsampler. The source must be an app-verified
/// output before this is called in production. No untrusted arbitrary paths.
fn thumbnail_png(source: &[u8]) -> Result<Vec<u8>, String> {
    const EDGE: u32 = 256;
    const MAX_DIM: u32 = 1536;
    const MAX_DECODED_BYTES: usize = MAX_DIM as usize * MAX_DIM as usize * 4;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(source));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder
        .read_info()
        .map_err(|_| "Could not decode this verified PNG for preview.".to_string())?;
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
        return Err("Image dimensions exceed the preview limit.".into());
    }
    let allocation = reader
        .output_buffer_size()
        .ok_or("Preview dimensions would overflow memory.")?;
    if allocation > MAX_DECODED_BYTES {
        return Err("Image preview exceeds its memory limit.".into());
    }
    let mut decoded = vec![0u8; allocation];
    let frame = reader
        .next_frame(&mut decoded)
        .map_err(|_| "The PNG pixel data could not be decoded.".to_string())?;
    if frame.width != width || frame.height != height || frame.bit_depth != png::BitDepth::Eight {
        return Err("Unsupported PNG frame layout for thumbnail.".into());
    }
    let channels = match frame.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        _ => return Err("Unsupported PNG color layout for thumbnail.".into()),
    };
    let line = frame.line_size;
    if line < width as usize * channels || frame.buffer_size() > decoded.len() {
        return Err("Invalid PNG row size for thumbnail.".into());
    }
    // Downscale only: a source smaller than the preview edge keeps its own size.
    let scale = width.max(height) as u64;
    let edge = (EDGE as u64).min(scale);
    let thumb_w = ((width as u64 * edge + scale / 2) / scale).max(1) as u32;
    let thumb_h = ((height as u64 * edge + scale / 2) / scale).max(1) as u32;
    let mut pixels = vec![0u8; thumb_w as usize * thumb_h as usize * 3];
    for y in 0..thumb_h {
        let from_y = (y as u64 * height as u64 / thumb_h as u64) as usize;
        for x in 0..thumb_w {
            let from_x = (x as u64 * width as u64 / thumb_w as u64) as usize;
            let from = from_y * line + from_x * channels;
            if from + channels > frame.buffer_size() {
                return Err("Invalid PNG pixel offset.".into());
            }
            let (r, g, b, alpha) = match channels {
                1 => (decoded[from], decoded[from], decoded[from], 255u16),
                2 => (
                    decoded[from],
                    decoded[from],
                    decoded[from],
                    decoded[from + 1] as u16,
                ),
                3 => (decoded[from], decoded[from + 1], decoded[from + 2], 255u16),
                _ => (
                    decoded[from],
                    decoded[from + 1],
                    decoded[from + 2],
                    decoded[from + 3] as u16,
                ),
            };
            let dest = (y as usize * thumb_w as usize + x as usize) * 3;
            for (i, v) in [r, g, b].into_iter().enumerate() {
                // Composite alpha against the Stage's dark canvas.
                pixels[dest + i] =
                    ((v as u16 * alpha + [17u16, 22, 36][i] * (255 - alpha)) / 255) as u8;
            }
        }
    }
    let mut result = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut result, thumb_w, thumb_h);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| "Could not start thumbnail encoding.".to_string())?;
        writer
            .write_image_data(&pixels)
            .map_err(|_| "Could not encode thumbnail pixels.".to_string())?;
    }
    if result.len() > 512 * 1024 {
        return Err("Encoded thumbnail exceeded its size limit.".into());
    }
    Ok(result)
}

/// Export a validated image and its matching provenance to Pictures\Optic Rift.
/// Destination filenames use the full UUID, not just an eight-character prefix.
/// Create-new prevents races from silently overwriting another application's file.
/// Copying two files cannot be atomic across cloud-sync software; failures are reported
/// and best-effort local cleanup is performed. The caller must obtain explicit
/// permission after explaining Windows Pictures/OneDrive synchronization.
pub fn save_copy(app_local: &Path, pictures: &Path, job_id: &str) -> Result<PathBuf, String> {
    let (png, record) = verified_output(app_local, job_id)?;

    let dest_dir = pictures.join("Optic Rift");
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Cannot create {}: {e}", dest_dir.display()))?;
    let stem = format!("optic-rift-{job_id}");
    let dest = dest_dir.join(format!("{stem}.png"));
    let metadata_dest = dest_dir.join(format!("{stem}.provenance.json"));
    // Reject pre-existing file *or symlink* for both members of the pair.
    // create_new is still mandatory: this pre-check by itself is not race-safe.
    if fs::symlink_metadata(&dest).is_ok() || fs::symlink_metadata(&metadata_dest).is_ok() {
        return Err("An export with this job ID already exists; no files were overwritten.".into());
    }
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dest)
        .map_err(|e| {
            format!("Cannot create the image without overwriting an existing file: {e}")
        })?;
    let result = output.write_all(&png).and_then(|_| output.sync_all());
    drop(output);
    if let Err(e) = result {
        let _ = fs::remove_file(&dest);
        return Err(format!("Failed to save the image: {e}"));
    }

    let mut sidecar = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&metadata_dest)
    {
        Ok(file) => file,
        Err(e) => {
            let _ = fs::remove_file(&dest);
            return Err(format!(
                "Cannot create the provenance record without overwriting an existing file: {e}"
            ));
        }
    };
    let result = sidecar.write_all(&record).and_then(|_| sidecar.sync_all());
    drop(sidecar);
    if let Err(e) = result {
        let _ = fs::remove_file(&metadata_dest);
        let _ = fs::remove_file(&dest);
        return Err(format!("Failed to save the provenance record: {e}"));
    }
    Ok(dest)
}

pub fn pictures_dir<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    app.path().picture_dir().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_need_grows_with_image_area() {
        let weights = 4_468_829_801;
        assert_eq!(job_need_mb(weights, 1024, 1024), 4261 + 4096);
        assert!(
            job_need_mb(weights, 1024, 1024) >= 8349,
            "covers the measured peak"
        );
        assert_eq!(
            job_need_mb(weights, 512, 512),
            4261 + 1536,
            "floor for small images"
        );
        assert!(job_need_mb(weights, 1536, 1536) > job_need_mb(weights, 1024, 1024));
    }

    #[test]
    fn f32_settings_are_recorded_as_their_decimal_values() {
        assert_eq!(decimal_f32(Some(0.65)), json!(0.65));
        assert_eq!(decimal_f32(Some(7.5)), json!(7.5));
        assert_eq!(decimal_f32(None), Value::Null);
        assert_eq!(decimal_f32(Some(f32::NAN)), Value::Null);
        assert_ne!(
            json!(0.65f32),
            json!(0.65),
            "documents the widening this avoids"
        );
    }

    #[test]
    fn thumbnail_is_bounded_and_does_not_copy_original_full_size() {
        let mut source = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut source, 512, 256);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer
                .write_image_data(&vec![138u8; 512 * 256 * 3])
                .unwrap();
        }
        let resized = thumbnail_png(&source).unwrap();
        assert!(resized.len() < 512 * 1024);
        let mut reader = png::Decoder::new(std::io::Cursor::new(resized))
            .read_info()
            .unwrap();
        assert_eq!((reader.info().width, reader.info().height), (256, 128));
        let mut buffer = vec![0u8; reader.output_buffer_size().unwrap()];
        let result = reader.next_frame(&mut buffer).unwrap();
        assert_eq!(result.color_type, png::ColorType::Rgb);
        assert_eq!(buffer[0..3], [138, 138, 138]);
        assert!(thumbnail_png(b"not a PNG").is_err());
    }

    #[test]
    fn thumbnail_rejects_oversize_before_pixel_decode() {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 1537, 1537);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            // Only a valid header is needed: dimensions are rejected before reading pixel data.
            writer
                .write_image_data(&vec![0u8; 1537 * 1537 * 3])
                .unwrap();
        }
        assert!(thumbnail_png(&png).unwrap_err().contains("preview limit"));
    }

    #[test]
    fn cancel_before_worker_knows_the_job_is_remembered_until_the_job_ends() {
        let id = uuid::Uuid::new_v4().to_string();
        assert!(check_not_cancelled(&id).is_ok());
        assert!(cancel("../not-a-job").is_err());
        // No worker has to exist: this is the verification/loading window that used to drop Cancel.
        cancel(&id).unwrap();
        cancel(&id).unwrap();
        assert_eq!(
            check_not_cancelled(&id).unwrap_err(),
            "Generation cancelled."
        );
        let other = uuid::Uuid::new_v4().to_string();
        assert!(
            check_not_cancelled(&other).is_ok(),
            "only the cancelled job stops"
        );
        forget_cancel(&id);
        assert!(check_not_cancelled(&id).is_ok());
        for _ in 0..40 {
            cancel(&uuid::Uuid::new_v4().to_string()).unwrap();
        }
        assert!(
            cancelled_jobs().lock().unwrap().len() <= 32,
            "stale ids stay bounded"
        );
    }

    /// Encode a real PNG with the given layout, so the thumbnail path decodes actual pixels.
    fn encode_png(width: u32, height: u32, color: png::ColorType, depth: png::BitDepth) -> Vec<u8> {
        let samples = match color {
            png::ColorType::Grayscale | png::ColorType::Indexed => 1,
            png::ColorType::GrayscaleAlpha => 2,
            png::ColorType::Rgb => 3,
            png::ColorType::Rgba => 4,
        };
        let row_bytes = (width as usize * samples * depth as usize).div_ceil(8);
        let data: Vec<u8> = (0..row_bytes * height as usize)
            .map(|i| {
                if color == png::ColorType::Indexed {
                    (i % 2) as u8
                } else {
                    (i % 251) as u8
                }
            })
            .collect();
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, width, height);
            encoder.set_color(color);
            encoder.set_depth(depth);
            if color == png::ColorType::Indexed {
                encoder.set_palette(vec![10, 20, 30, 200, 210, 220]);
            }
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&data).unwrap();
        }
        out
    }

    /// Write `png` and a provenance record that matches it exactly.
    fn write_real_output(local: &Path, id: &str, png: &[u8]) {
        let (width, height) = png_size(png).unwrap_or((0, 0));
        let sha = Sha256::digest(png)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        fs::create_dir_all(outputs_dir(local)).unwrap();
        fs::write(outputs_dir(local).join(format!("{id}.png")), png).unwrap();
        let record = json!({
            "record_version": 1, "job_id": id, "created_unix_ms": 0,
            "prompt": "test", "negative_prompt": "", "parameters": {},
            "model": {}, "runtime": {}, "metrics": {},
            "output": {"sha256": sha, "width": width, "height": height, "bytes": png.len()},
            "project_id": null, "lineage_note": "test"
        });
        fs::write(
            outputs_dir(local).join(format!("{id}.json")),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
    }

    /// Every file name and its bytes, to prove a call wrote, renamed or deleted nothing.
    fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_file())
            .map(|p| {
                (
                    p.file_name().unwrap().to_string_lossy().into_owned(),
                    fs::read(&p).unwrap(),
                )
            })
            .collect();
        entries.sort();
        entries
    }

    fn decoded_size(thumb: &[u8]) -> (u32, u32, png::ColorType) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(thumb))
            .read_info()
            .unwrap();
        let mut buffer = vec![0u8; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut buffer).unwrap();
        (frame.width, frame.height, frame.color_type)
    }

    #[test]
    fn verified_thumbnail_keeps_aspect_and_writes_nothing() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        // (source width, height) -> expected thumbnail; never cropped, stretched or upscaled.
        let cases = [
            ((768, 1344), (146, 256)),
            ((1344, 768), (256, 146)),
            ((1024, 1024), (256, 256)),
            ((1536, 256), (256, 43)),
            ((256, 1536), (43, 256)),
            ((64, 32), (64, 32)),
        ];
        for ((w, h), expected) in cases {
            let id = uuid::Uuid::new_v4().to_string();
            write_real_output(
                &local,
                &id,
                &encode_png(w, h, png::ColorType::Rgb, png::BitDepth::Eight),
            );
            let before = snapshot(&outputs_dir(&local));
            let thumb = output_thumbnail_png(&local, &id).unwrap();
            assert!(thumb.len() <= 512 * 1024);
            let (tw, th, color) = decoded_size(&thumb);
            assert_eq!((tw, th), expected, "source {w}x{h}");
            assert!(tw.max(th) <= 256);
            assert_eq!(
                color,
                png::ColorType::Rgb,
                "metadata and alpha are not carried over"
            );
            assert_eq!(
                snapshot(&outputs_dir(&local)),
                before,
                "no cache, sidecar or source change"
            );
        }
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn verified_thumbnail_handles_every_png_color_layout() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        for (color, depth) in [
            (png::ColorType::Grayscale, png::BitDepth::Eight),
            (png::ColorType::Grayscale, png::BitDepth::Four),
            (png::ColorType::GrayscaleAlpha, png::BitDepth::Eight),
            (png::ColorType::Rgba, png::BitDepth::Eight),
            (png::ColorType::Rgb, png::BitDepth::Sixteen),
            (png::ColorType::Rgba, png::BitDepth::Sixteen),
            (png::ColorType::Indexed, png::BitDepth::Eight),
        ] {
            let id = uuid::Uuid::new_v4().to_string();
            write_real_output(&local, &id, &encode_png(640, 480, color, depth));
            let thumb = output_thumbnail_png(&local, &id)
                .unwrap_or_else(|e| panic!("{color:?}/{depth:?}: {e}"));
            assert_eq!(decoded_size(&thumb), (256, 192, png::ColorType::Rgb));
        }
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn thumbnail_refuses_unverified_sources_without_changing_them() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let source = encode_png(512, 512, png::ColorType::Rgb, png::BitDepth::Eight);
        let id = uuid::Uuid::new_v4().to_string();
        write_real_output(&local, &id, &source);
        assert!(output_thumbnail_png(&local, &id).is_ok());

        for bad in [
            "../../secret",
            "not-a-uuid",
            "",
            &id.to_uppercase(),
            &id.replace('-', ""),
        ] {
            assert!(output_thumbnail_png(&local, bad)
                .unwrap_err()
                .contains("Invalid output id"));
        }
        let missing = uuid::Uuid::new_v4().to_string();
        assert!(output_thumbnail_png(&local, &missing).is_err());

        let png_path = outputs_dir(&local).join(format!("{id}.png"));
        let record_path = outputs_dir(&local).join(format!("{id}.json"));
        let record = fs::read(&record_path).unwrap();

        // Altered pixels: hash no longer matches provenance.
        let mut altered = source.clone();
        let mid = altered.len() / 2;
        altered[mid] ^= 0xff;
        fs::write(&png_path, &altered).unwrap();
        let before = snapshot(&outputs_dir(&local));
        assert!(output_thumbnail_png(&local, &id)
            .unwrap_err()
            .contains("does not match"));
        assert_eq!(
            snapshot(&outputs_dir(&local)),
            before,
            "a failed check repairs nothing"
        );
        fs::write(&png_path, &source).unwrap();

        // Invalid, missing or misleading provenance.
        fs::write(&record_path, b"not-json").unwrap();
        assert!(output_thumbnail_png(&local, &id).is_err());
        fs::remove_file(&record_path).unwrap();
        assert!(output_thumbnail_png(&local, &id).is_err());
        let mut wrong: Value = serde_json::from_slice(&record).unwrap();
        wrong["output"]["width"] = json!(256);
        fs::write(&record_path, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(output_thumbnail_png(&local, &id)
            .unwrap_err()
            .contains("does not match"));
        let mut other: Value = serde_json::from_slice(&record).unwrap();
        other["job_id"] = json!(missing);
        fs::write(&record_path, serde_json::to_vec(&other).unwrap()).unwrap();
        assert!(output_thumbnail_png(&local, &id).is_err());
        fs::write(&record_path, &record).unwrap();

        // Missing PNG with a record still present.
        fs::remove_file(&png_path).unwrap();
        assert!(output_thumbnail_png(&local, &id)
            .unwrap_err()
            .contains("missing"));

        // Non-PNG bytes and a PNG whose header verifies but whose pixel data is corrupt.
        let not_png = uuid::Uuid::new_v4().to_string();
        write_real_output(&local, &not_png, b"GIF89a not a png at all");
        assert!(output_thumbnail_png(&local, &not_png).is_err());
        let corrupt = uuid::Uuid::new_v4().to_string();
        let mut truncated = encode_png(512, 512, png::ColorType::Rgb, png::BitDepth::Eight);
        truncated.truncate(64);
        write_real_output(&local, &corrupt, &truncated);
        assert!(output_thumbnail_png(&local, &corrupt).is_err());
        assert_eq!(
            fs::read(outputs_dir(&local).join(format!("{corrupt}.png"))).unwrap(),
            truncated
        );
        fs::remove_dir_all(local).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn thumbnail_refuses_junctioned_outputs_folder_and_linked_png() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let elsewhere =
            std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        write_real_output(
            &elsewhere,
            &id,
            &encode_png(300, 200, png::ColorType::Rgb, png::BitDepth::Eight),
        );
        assert!(output_thumbnail_png(&elsewhere, &id).is_ok());
        fs::create_dir_all(&local).unwrap();
        // Directory junctions need no elevation, unlike symbolic links.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(outputs_dir(&local))
            .arg(outputs_dir(&elsewhere))
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed: {status:?}");
        assert!(
            outputs_dir(&local).join(format!("{id}.png")).exists(),
            "junction resolves"
        );
        assert!(output_thumbnail_png(&local, &id)
            .unwrap_err()
            .contains("redirected"));
        fs::remove_dir(outputs_dir(&local)).unwrap();

        // A linked PNG inside a genuine outputs folder (needs symlink privilege).
        let linked = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        write_real_output(
            &linked,
            &id,
            &fs::read(outputs_dir(&elsewhere).join(format!("{id}.png"))).unwrap(),
        );
        let png_path = outputs_dir(&linked).join(format!("{id}.png"));
        fs::remove_file(&png_path).unwrap();
        if std::os::windows::fs::symlink_file(
            outputs_dir(&elsewhere).join(format!("{id}.png")),
            &png_path,
        )
        .is_ok()
        {
            assert!(output_thumbnail_png(&linked, &id)
                .unwrap_err()
                .contains("link"));
        } else {
            eprintln!(
                "symlink creation needs Developer Mode or elevation; linked-PNG case skipped"
            );
        }
        for dir in [local, elsewhere, linked] {
            fs::remove_dir_all(dir).unwrap();
        }
    }

    #[test]
    fn png_header_parsing() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(1024u32.to_be_bytes());
        png.extend(768u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((1024, 768)));
        assert_eq!(png_size(b"GIF89a.................."), None);
    }

    /// A small PNG-header fixture is enough for the native save path's validation.
    /// The independent image-generation path validates actual worker output separately.
    fn write_output_pair(local: &Path, id: &str) -> Vec<u8> {
        write_output_pair_for_size(local, id, 1, 1)
    }

    fn write_output_pair_for_size(local: &Path, id: &str, width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend_from_slice(&width.to_be_bytes());
        png.extend_from_slice(&height.to_be_bytes());
        let sha = Sha256::digest(&png)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        fs::create_dir_all(outputs_dir(local)).unwrap();
        fs::write(outputs_dir(local).join(format!("{id}.png")), &png).unwrap();
        let record = json!({
            "record_version": 1, "job_id": id, "created_unix_ms": 0,
            "prompt": "test", "negative_prompt": "", "parameters": {},
            "model": {}, "runtime": {}, "metrics": {},
            "output": {"sha256": sha, "width": width, "height": height, "bytes": png.len()},
            "project_id": null, "lineage_note": "test"
        });
        fs::write(
            outputs_dir(local).join(format!("{id}.json")),
            serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        png
    }

    #[test]
    fn output_ids_are_strict_and_save_never_overwrites() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let pictures = local.join("Pictures");
        assert!(output_png(&local, "../../secret").is_err());
        assert!(save_copy(&local, &pictures, "../../secret").is_err());
        let id = uuid::Uuid::new_v4().to_string();
        let png = write_output_pair(&local, &id);
        let saved = save_copy(&local, &pictures, &id).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), png);
        assert!(
            saved.to_string_lossy().contains(&id),
            "full UUID is retained"
        );
        let provenance = saved.with_file_name(format!("optic-rift-{id}.provenance.json"));
        let saved_meta: Provenance =
            serde_json::from_slice(&fs::read(provenance).unwrap()).unwrap();
        assert_eq!(saved_meta.job_id, id);
        assert!(save_copy(&local, &pictures, &id).is_err());
        assert_eq!(fs::read(&saved).unwrap(), png);
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn refuses_missing_or_mismatched_provenance_before_creating_export() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let pictures = local.join("Pictures");
        let id = uuid::Uuid::new_v4().to_string();
        write_output_pair(&local, &id);
        let record = outputs_dir(&local).join(format!("{id}.json"));
        let bytes = fs::read(&record).unwrap();
        fs::remove_file(&record).unwrap();
        assert!(save_copy(&local, &pictures, &id).is_err());
        assert!(!pictures.join("Optic Rift").exists());
        let mut bad: Value = serde_json::from_slice(&bytes).unwrap();
        bad["output"]["sha256"] = json!("wronghash");
        fs::write(&record, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(save_copy(&local, &pictures, &id).is_err());
        assert!(!pictures.join("Optic Rift").exists());
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn history_only_lists_bounded_records_matching_their_filenames() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        write_output_pair(&local, &id);
        assert_eq!(list_outputs(&local).len(), 1);
        let invalid_name = outputs_dir(&local).join("unrelated.json");
        fs::copy(
            outputs_dir(&local).join(format!("{id}.json")),
            &invalid_name,
        )
        .unwrap();
        assert_eq!(
            list_outputs(&local).len(),
            1,
            "mismatched identity is ignored"
        );
        fs::write(invalid_name, vec![b'x'; 512 * 1024 + 1]).unwrap();
        assert_eq!(
            list_outputs(&local).len(),
            1,
            "oversized metadata is skipped"
        );
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn reopened_output_is_hash_checked_before_preview() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let png = write_output_pair(&local, &id);
        assert_eq!(output_png(&local, &id).unwrap(), png);
        let path = outputs_dir(&local).join(format!("{id}.png"));
        let mut corrupted = png.clone();
        let tail = corrupted.len() - 1;
        corrupted[tail] ^= 1;
        fs::write(&path, &corrupted).unwrap();
        assert!(output_png(&local, &id)
            .unwrap_err()
            .contains("does not match"));
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn reopened_output_rejects_invalid_record_and_large_files() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        write_output_pair(&local, &id);
        fs::write(outputs_dir(&local).join(format!("{id}.json")), b"not-json").unwrap();
        assert!(output_png(&local, &id).is_err());
        assert!(read_output_file(&outputs_dir(&local).join(format!("{id}.png")), 1).is_err());
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn provenance_collision_is_not_overwritten_or_partially_exported() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let pictures = local.join("Pictures");
        let id = uuid::Uuid::new_v4().to_string();
        write_output_pair(&local, &id);
        let dest_dir = pictures.join("Optic Rift");
        fs::create_dir_all(&dest_dir).unwrap();
        let sidecar = dest_dir.join(format!("optic-rift-{id}.provenance.json"));
        fs::write(&sidecar, b"existing").unwrap();
        assert!(save_copy(&local, &pictures, &id).is_err());
        assert_eq!(fs::read(&sidecar).unwrap(), b"existing");
        assert!(!dest_dir.join(format!("optic-rift-{id}.png")).exists());
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn concurrent_saves_never_overwrite_the_winner() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let pictures = local.join("Pictures");
        let id = uuid::Uuid::new_v4().to_string();
        let png = write_output_pair(&local, &id);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| save_copy(&local, &pictures, &id));
            let b = scope.spawn(|| save_copy(&local, &pictures, &id));
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        let dest = pictures
            .join("Optic Rift")
            .join(format!("optic-rift-{id}.png"));
        assert_eq!(fs::read(dest).unwrap(), png);
        fs::remove_dir_all(local).unwrap();
    }
    #[test]
    fn image_modes_default_to_text_and_unknown_modes_fail_closed() {
        let base = json!({
            "job_id": uuid::Uuid::new_v4().to_string(),
            "repo": "segmind/SSD-1B", "revision": "0".repeat(40),
            "prompt": "a garden", "width": 512, "height": 512,
            "steps": 20, "guidance": 7.0, "seed": null, "project_id": null
        });
        let request: GenerateRequest = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(request.mode, GenerationMode::TextToImage);
        let mut bad = base;
        bad["mode"] = json!("arbitrary-mode");
        assert!(serde_json::from_value::<GenerateRequest>(bad).is_err());
    }

    #[test]
    fn image_to_image_reference_must_match_verified_local_history() {
        let local =
            std::env::temp_dir().join(format!("optic-rift-img2img-{}", uuid::Uuid::new_v4()));
        let id = uuid::Uuid::new_v4().to_string();
        let dest = uuid::Uuid::new_v4().to_string();
        let original = write_output_pair_for_size(&local, &id, 512, 512);
        let reference = verified_image_reference(
            &local,
            GenerationMode::ImageToImage,
            Some(&id),
            Some(0.65),
            512,
            512,
            &dest,
        )
        .unwrap()
        .unwrap();
        assert_eq!(reference.job_id, id);
        assert_eq!(
            reference.path,
            outputs_dir(&local).join(format!("{id}.png"))
        );
        assert_eq!(reference.sha256, format!("{:x}", Sha256::digest(&original)));
        assert!(verified_image_reference(
            &local,
            GenerationMode::ImageToImage,
            Some(&id),
            Some(0.65),
            1024,
            1024,
            &dest,
        )
        .is_err());
        assert!(verified_image_reference(
            &local,
            GenerationMode::ImageToImage,
            Some(&id),
            Some(0.65),
            512,
            512,
            &id,
        )
        .is_err());
        fs::write(&reference.path, b"tampered same-origin local output").unwrap();
        assert!(verified_image_reference(
            &local,
            GenerationMode::ImageToImage,
            Some(&id),
            Some(0.65),
            512,
            512,
            &dest,
        )
        .is_err());
        fs::remove_dir_all(local).unwrap();
    }

    #[test]
    fn image_to_image_rejects_invalid_input_and_preserves_t2i_contract() {
        let local =
            std::env::temp_dir().join(format!("optic-rift-img2img-{}", uuid::Uuid::new_v4()));
        let dest = uuid::Uuid::new_v4().to_string();
        assert!(preflight_image_reference(
            &local,
            GenerationMode::TextToImage,
            None,
            None,
            512,
            512,
        )
        .is_ok());
        assert!(preflight_image_reference(
            &local,
            GenerationMode::TextToImage,
            Some("bogus"),
            None,
            512,
            512,
        )
        .is_err());
        for strength in [None, Some(f32::NAN), Some(0.0), Some(1.0)] {
            assert!(verified_image_reference(
                &local,
                GenerationMode::ImageToImage,
                Some(&dest),
                strength,
                512,
                512,
                "",
            )
            .is_err());
        }
        assert!(verified_image_reference(
            &local,
            GenerationMode::ImageToImage,
            Some("../not-a-job"),
            Some(0.5),
            512,
            512,
            "",
        )
        .is_err());
        assert!(verified_image_reference(
            &local,
            GenerationMode::ImageToImage,
            None,
            Some(0.5),
            512,
            512,
            "",
        )
        .is_err());
    }

    #[test]
    fn img2img_needs_at_least_one_effective_denoising_step() {
        assert!(
            validate_image_to_image_steps(GenerationMode::ImageToImage, 1, Some(0.65)).is_err()
        );
        assert!(
            validate_image_to_image_steps(GenerationMode::ImageToImage, 10, Some(0.05)).is_err()
        );
        assert!(
            validate_image_to_image_steps(GenerationMode::ImageToImage, 20, Some(0.05)).is_ok()
        );
        assert!(
            validate_image_to_image_steps(GenerationMode::ImageToImage, 25, Some(0.65)).is_ok()
        );
        assert!(validate_image_to_image_steps(GenerationMode::TextToImage, 1, None).is_ok());
    }

    #[test]
    fn img2img_gpu_preflight_uses_additional_unqualified_margin() {
        let weights = 4_468_829_801;
        let text = job_need_for_mode(weights, 1024, 1024, GenerationMode::TextToImage);
        let reference = job_need_for_mode(weights, 1024, 1024, GenerationMode::ImageToImage);
        assert_eq!(text, job_need_mb(weights, 1024, 1024));
        assert_eq!(reference, text + 2048);
    }
}
