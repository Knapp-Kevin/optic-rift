//! Isolated Python runtime for the diffusers worker: a dedicated virtual environment created with
//! `uv`, pinned package versions, an app-owned worker script and a health check that must pass
//! before the runtime counts as installed. Nothing from a model repository is ever installed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::Emitter;

pub const RUNTIME_ID: &str = "diffusers-v1";
pub const WORKER_SOURCE: &str = include_str!("../workers/diffusers_worker.py");
const PYTHON_VERSION: &str = "3.12";
const TORCH_INDEX: &str = "https://download.pytorch.org/whl/cu130";
const TORCH: &str = "torch==2.14.1+cu130";
/// Exact versions resolved and exercised on the qualification host.
pub const PACKAGES: &[&str] = &[
    "diffusers==0.42.1",
    "transformers==5.22.1",
    "safetensors==0.8.0",
    "accelerate==1.15.0",
    "huggingface-hub==2.4.0",
    "tokenizers==0.23.3",
    "numpy==2.5.4",
    "pillow==13.0.0",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeManifest {
    pub runtime: String,
    pub python: String,
    pub torch: String,
    pub diffusers: String,
    pub cuda_available: bool,
    pub device: Option<String>,
    pub packages: Vec<String>,
    pub installed_unix_ms: u64,
}

pub fn root(app_local: &Path) -> PathBuf {
    app_local.join("runtimes").join(RUNTIME_ID)
}

pub fn python(root: &Path) -> PathBuf {
    root.join("venv").join("Scripts").join("python.exe")
}

pub fn worker_path(root: &Path) -> PathBuf {
    root.join("diffusers_worker.py")
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join("runtime.json")
}

pub fn manifest(root: &Path) -> Option<RuntimeManifest> {
    let m: RuntimeManifest = serde_json::from_slice(&fs::read(manifest_path(root)).ok()?).ok()?;
    (m.runtime == RUNTIME_ID && python(root).is_file()).then_some(m)
}

/// Write the app-owned worker script, replacing any modified copy.
pub fn ensure_worker_script(root: &Path) -> Result<PathBuf, String> {
    let path = worker_path(root);
    let want = Sha256::digest(WORKER_SOURCE.as_bytes());
    let current = fs::read(&path).ok().map(|b| Sha256::digest(&b));
    if current.as_deref() != Some(want.as_slice()) {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        fs::write(&path, WORKER_SOURCE)
            .map_err(|e| format!("Cannot write the worker script: {e}"))?;
    }
    Ok(path)
}

fn find_uv() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("OPTIC_RIFT_UV")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join("uv.exe"))
        .find(|p| p.is_file())
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub installed: Option<RuntimeManifest>,
    pub uv_available: bool,
    pub root: PathBuf,
    pub torch_index: &'static str,
    pub torch: &'static str,
    pub packages: &'static [&'static str],
}

pub fn status(app_local: &Path) -> RuntimeStatus {
    let root = root(app_local);
    RuntimeStatus {
        installed: manifest(&root),
        uv_available: find_uv().is_some(),
        root,
        torch_index: TORCH_INDEX,
        torch: TORCH,
        packages: PACKAGES,
    }
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeEvent {
    stage: &'static str,
    line: Option<String>,
}

fn run_logged<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    stage: &'static str,
    program: &Path,
    args: &[&str],
) -> Result<(), String> {
    let _ = app.emit("runtime-install", RuntimeEvent { stage, line: None });
    let mut command = Command::new(program);
    command
        .args(args)
        .env("UV_NO_CONFIG", "1")
        .env_remove("PIP_INDEX_URL")
        .env_remove("UV_INDEX_URL")
        .env_remove("UV_EXTRA_INDEX_URL")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not start {}: {e}", program.display()))?;
    let stderr = child.stderr.take().unwrap();
    let app2 = app.clone();
    let tail = std::thread::spawn(move || {
        let mut last = Vec::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let line: String = line.chars().take(400).collect();
            let _ = app2.emit(
                "runtime-install",
                RuntimeEvent {
                    stage,
                    line: Some(line.clone()),
                },
            );
            last.push(line);
            if last.len() > 8 {
                last.remove(0);
            }
        }
        last
    });
    // uv reports progress on stderr; stdout is drained and discarded.
    let _ = std::io::copy(&mut child.stdout.take().unwrap(), &mut std::io::sink());
    let status = child.wait().map_err(|e| e.to_string())?;
    let last = tail.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        Err(format!("{stage} failed: {}", last.join(" | ")))
    }
}

/// Create the environment and install pinned packages. Blocking; call from a worker thread.
pub fn install<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    app_local: &Path,
) -> Result<RuntimeManifest, String> {
    let uv = find_uv().ok_or("uv was not found. Install uv from https://docs.astral.sh/uv/ (Optic Rift does not bundle a Python runtime yet), then try again.")?;
    let root = root(app_local);
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(manifest_path(&root));
    let venv = root.join("venv");
    let venv_s = venv.to_string_lossy().to_string();
    if !python(&root).is_file() {
        run_logged(
            app,
            "Creating Python environment",
            &uv,
            &["venv", "--python", PYTHON_VERSION, &venv_s],
        )?;
    }
    let py = python(&root).to_string_lossy().to_string();
    run_logged(
        app,
        "Installing PyTorch (CUDA)",
        &uv,
        &[
            "pip",
            "install",
            "--python",
            &py,
            "--index-url",
            TORCH_INDEX,
            TORCH,
        ],
    )?;
    let mut args = vec![
        "pip",
        "install",
        "--python",
        &py,
        "--index-url",
        "https://pypi.org/simple",
    ];
    args.extend(PACKAGES);
    run_logged(app, "Installing diffusion libraries", &uv, &args)?;
    let _ = app.emit(
        "runtime-install",
        RuntimeEvent {
            stage: "Checking GPU access",
            line: None,
        },
    );
    let health = crate::generation::health_once(&root)?;
    let get = |k: &str| {
        health
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let manifest = RuntimeManifest {
        runtime: RUNTIME_ID.into(),
        python: get("python"),
        torch: get("torch"),
        diffusers: get("diffusers"),
        cuda_available: health
            .get("cuda_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        device: health
            .get("device")
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        packages: std::iter::once(TORCH)
            .chain(PACKAGES.iter().copied())
            .map(str::to_owned)
            .collect(),
        installed_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    };
    if !manifest.cuda_available {
        return Err(
            "The runtime installed, but PyTorch cannot use a CUDA GPU on this computer.".into(),
        );
    }
    fs::write(
        manifest_path(&root),
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let _ = app.emit(
        "runtime-install",
        RuntimeEvent {
            stage: "Installed",
            line: None,
        },
    );
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packages_are_exactly_pinned() {
        assert!(TORCH.contains("==") && TORCH_INDEX.starts_with("https://download.pytorch.org/"));
        for p in PACKAGES {
            assert!(
                p.contains("==") && !p.contains('>') && !p.contains('<'),
                "{p}"
            );
        }
    }

    #[test]
    fn worker_script_is_restored_when_modified() {
        let root = std::env::temp_dir().join(format!("optic-rift-rt-{}", uuid::Uuid::new_v4()));
        let path = ensure_worker_script(&root).unwrap();
        fs::write(&path, "print('tampered')").unwrap();
        ensure_worker_script(&root).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), WORKER_SOURCE);
        assert!(
            manifest(&root).is_none(),
            "no runtime without a manifest and interpreter"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
