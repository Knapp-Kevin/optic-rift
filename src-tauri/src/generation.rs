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

    fn kill(&self) {
        let mut child = self.child.lock().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        self.alive.store(false, Ordering::SeqCst);
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
    let _ = w.send(&json!({"op": "shutdown"}));
    std::thread::sleep(Duration::from_millis(300));
    w.kill();
    result
}

pub fn shutdown() {
    if let Some(w) = slot().lock().unwrap().take() {
        let _ = w.send(&json!({"op": "shutdown"}));
        std::thread::sleep(Duration::from_millis(500));
        w.kill();
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
/// Conservative until a measured receipt exists for the model.
pub fn job_need_mb(weights_bytes: u64, width: u32, height: u32) -> u64 {
    let pixels = (width as u64 * height as u64).max(1);
    let workspace = (2048 * pixels / (1024 * 1024)).max(1024);
    weights_bytes / (1024 * 1024) + workspace
}

/// Check the GPU is free enough to use right now, considering every other process.
pub fn gpu_decision(
    weights_bytes: u64,
    width: u32,
    height: u32,
    acknowledged: &[u32],
) -> (gpu_guard::GpuSnapshot, gpu_guard::Decision, u64) {
    let snapshot = gpu_guard::snapshot();
    let need = job_need_mb(weights_bytes, width, height);
    let decision = gpu_guard::evaluate(&snapshot, need, acknowledged);
    (snapshot, decision, need)
}

fn gpu_budget(model: &InstalledModel, req: &GenerateRequest) -> Result<u64, String> {
    match gpu_decision(
        model.manifest.total_bytes,
        req.width,
        req.height,
        &req.gpu_acknowledged,
    )
    .1
    {
        gpu_guard::Decision::Proceed { budget_mb } => Ok(budget_mb),
        gpu_guard::Decision::Confirm { message, .. } => Err(format!("GPU_BUSY: {message}")),
        gpu_guard::Decision::Block { message } => Err(message),
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
    pub lineage_note: String,
}

fn valid_job_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|u| u.hyphenated().to_string() == id)
}

pub fn outputs_dir(app_local: &Path) -> PathBuf {
    app_local.join("outputs")
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
    let (data, local) = model_store::dirs(app)?;
    let models = model_store::model_dir(&data, &local);
    let installed: InstalledModel = model_store::find_installed(&models, &req.repo, &req.revision)
        .ok_or("That model is not installed.")?;
    if !installed.files_present {
        return Err("The installed model is incomplete. Remove and reinstall it.".into());
    }
    let root = runtime::root(&local);
    let rt = runtime::manifest(&root).ok_or("Install the image runtime first (Models page).")?;
    let w = worker(app, &root)?;
    if w.busy.swap(true, Ordering::SeqCst) {
        return Err("A generation is already running.".into());
    }
    let result = run_job(app, &w, &installed, &rt, &local, &req);
    w.busy.store(false, Ordering::SeqCst);
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
    // Re-checked for every job: a game may have started since the model was loaded.
    let budget_mb = gpu_budget(model, req)?;
    if w.loaded.lock().unwrap().as_ref() != Some(&key) {
        let _ = app.emit(
            "generation",
            json!({"event": "loading", "job_id": req.job_id}),
        );
        w.send(&json!({"op": "load", "model_dir": model.dir, "adapter": model.manifest.adapter, "variant": model.manifest.variant, "vram_budget_mb": budget_mb}))?;
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
    let out_dir = outputs_dir(local);
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let png = out_dir.join(format!("{}.png", req.job_id));
    w.send(&json!({
        "op": "generate", "job_id": req.job_id, "prompt": req.prompt, "negative_prompt": req.negative_prompt,
        "width": req.width, "height": req.height, "steps": req.steps, "guidance": req.guidance,
        "seed": req.seed, "output_path": png, "vram_budget_mb": budget_mb,
    }))?;
    let job = req.job_id.clone();
    let end = w.wait_for(
        move |e| {
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
        parameters: json!({"width": req.width, "height": req.height, "steps": req.steps, "guidance": req.guidance, "seed": end["seed"]}),
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

pub fn cancel(job_id: &str) -> Result<(), String> {
    let w = slot()
        .lock()
        .unwrap()
        .clone()
        .ok_or("No generation is running.")?;
    w.send(&json!({"op": "cancel", "job_id": job_id}))
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
    let mut out: Vec<Provenance> = fs::read_dir(outputs_dir(app_local))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice::<Provenance>(&fs::read(e.path()).ok()?).ok())
        .filter(|p| valid_job_id(&p.job_id))
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.created_unix_ms));
    out.truncate(100);
    out
}

pub fn output_png(app_local: &Path, job_id: &str) -> Result<Vec<u8>, String> {
    if !valid_job_id(job_id) {
        return Err("Invalid output id.".into());
    }
    fs::read(outputs_dir(app_local).join(format!("{job_id}.png")))
        .map_err(|_| "That output no longer exists.".to_string())
}

/// Copy an output and its provenance record to the user's Pictures\Optic Rift folder.
pub fn save_copy(app_local: &Path, pictures: &Path, job_id: &str) -> Result<PathBuf, String> {
    let png = output_png(app_local, job_id)?;
    let dest_dir = pictures.join("Optic Rift");
    fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Cannot create {}: {e}", dest_dir.display()))?;
    let stem = format!("optic-rift-{}", &job_id[..8]);
    let dest = dest_dir.join(format!("{stem}.png"));
    if dest.exists() {
        return Err(format!("{} already exists.", dest.display()));
    }
    fs::write(&dest, png).map_err(|e| e.to_string())?;
    let record = outputs_dir(app_local).join(format!("{job_id}.json"));
    if let Ok(bytes) = fs::read(record) {
        let _ = fs::write(dest_dir.join(format!("{stem}.provenance.json")), bytes);
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
        assert_eq!(job_need_mb(weights, 1024, 1024), 4261 + 2048);
        assert_eq!(
            job_need_mb(weights, 512, 512),
            4261 + 1024,
            "floor for small images"
        );
        assert!(job_need_mb(weights, 1536, 1536) > job_need_mb(weights, 1024, 1024));
    }

    #[test]
    fn png_header_parsing() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(1024u32.to_be_bytes());
        png.extend(768u32.to_be_bytes());
        assert_eq!(png_size(&png), Some((1024, 768)));
        assert_eq!(png_size(b"GIF89a.................."), None);
    }

    #[test]
    fn output_ids_are_strict_and_save_never_overwrites() {
        let local = std::env::temp_dir().join(format!("optic-rift-gen-{}", uuid::Uuid::new_v4()));
        let pictures = local.join("Pictures");
        assert!(output_png(&local, "../../secret").is_err());
        let id = uuid::Uuid::new_v4().to_string();
        fs::create_dir_all(outputs_dir(&local)).unwrap();
        fs::write(outputs_dir(&local).join(format!("{id}.png")), b"png").unwrap();
        let saved = save_copy(&local, &pictures, &id).unwrap();
        assert_eq!(fs::read(&saved).unwrap(), b"png");
        assert!(save_copy(&local, &pictures, &id).is_err());
        fs::remove_dir_all(local).unwrap();
    }
}
