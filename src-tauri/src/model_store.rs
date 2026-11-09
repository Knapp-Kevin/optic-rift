//! Local model installation: configurable model folder, explicit consent, pinned revisions,
//! resumable downloads, SHA-256 / git-blob verification, cancellation and an install manifest that
//! is written only after every file and the adapter's static checks pass.

use crate::adapters::{self, InstallPlan, RepoFile};
use crate::hf_access::USER_SLOT;
use crate::hf_catalog::{self, Access, HUB};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

const MANIFEST: &str = "optic-rift-install.json";
const PARTIAL_DIR: &str = ".partial";
const DISK_MARGIN: u64 = 2 * 1024 * 1024 * 1024;
const MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    pub model_dir: Option<PathBuf>,
}

fn settings_path(app_data: &Path) -> PathBuf {
    app_data.join("settings.json")
}

pub fn load_settings(app_data: &Path) -> Settings {
    fs::read(settings_path(app_data))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Settings { model_dir: None })
}

pub fn model_dir(app_data: &Path, app_local: &Path) -> PathBuf {
    load_settings(app_data)
        .model_dir
        .unwrap_or_else(|| app_local.join("models"))
}

/// Accept an absolute, writable folder for models. Existing models are not moved.
pub fn set_model_dir(app_data: &Path, raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw.trim());
    if !path.is_absolute() || raw.trim().is_empty() {
        return Err("Enter a full folder path, for example D:\\Optic Rift Models.".into());
    }
    fs::create_dir_all(&path).map_err(|e| format!("Cannot use that folder: {e}"))?;
    let probe = path.join(format!(".optic-rift-write-test-{}", uuid::Uuid::new_v4()));
    File::create(&probe)
        .and_then(|_| fs::remove_file(&probe))
        .map_err(|e| format!("That folder is not writable: {e}"))?;
    let path = fs::canonicalize(&path).map_err(|e| e.to_string())?;
    let path = PathBuf::from(path.to_string_lossy().trim_start_matches(r"\\?\"));
    fs::create_dir_all(app_data).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(&Settings {
        model_dir: Some(path.clone()),
    })
    .map_err(|e| e.to_string())?;
    let tmp = settings_path(app_data).with_extension("json.tmp");
    fs::write(&tmp, bytes)
        .and_then(|_| fs::rename(&tmp, settings_path(app_data)))
        .map_err(|e| e.to_string())?;
    Ok(path)
}

fn repo_folder(repo: &str) -> String {
    repo.replace('/', "--")
}

pub fn install_dir(models: &Path, repo: &str, revision: &str) -> PathBuf {
    models.join(repo_folder(repo)).join(revision)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Consent {
    /// License identifier the user was shown and acknowledged (or "unknown").
    pub license_acknowledged: String,
    pub gated: String,
    pub audience: String,
    pub acknowledged_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstalledFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub manifest_version: u32,
    pub repo: String,
    pub revision: String,
    pub adapter: String,
    pub adapter_version: u32,
    pub variant: Option<String>,
    pub license: Option<String>,
    pub license_name: Option<String>,
    pub base_models: Option<String>,
    pub consent: Consent,
    pub files: Vec<InstalledFile>,
    pub total_bytes: u64,
    pub installed_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstalledModel {
    pub manifest: Manifest,
    pub dir: PathBuf,
    /// Every file is present with its recorded size (cheap check; full hashing on demand).
    pub files_present: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn read_manifest(dir: &Path) -> Result<Manifest, String> {
    let bytes = fs::read(dir.join(MANIFEST)).map_err(|e| format!("No install manifest: {e}"))?;
    if bytes.len() > 1024 * 1024 {
        return Err("Install manifest is too large.".into());
    }
    let m: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Install manifest is corrupt: {e}"))?;
    if m.manifest_version != MANIFEST_VERSION
        || !hf_catalog::valid_repo_id(&m.repo)
        || !hf_catalog::valid_revision_sha(&m.revision)
    {
        return Err("Install manifest has an unsupported version or invalid identity.".into());
    }
    if m.files.iter().any(|f| !safe_relative(&f.path)) {
        return Err("Install manifest lists an unsafe path.".into());
    }
    Ok(m)
}

fn safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

fn files_present(dir: &Path, m: &Manifest) -> bool {
    m.files.iter().all(|f| {
        fs::metadata(dir.join(&f.path))
            .map(|md| md.is_file() && md.len() == f.size)
            .unwrap_or(false)
    })
}

pub fn list_installed(models: &Path) -> Vec<InstalledModel> {
    let mut out = Vec::new();
    let Ok(repos) = fs::read_dir(models) else {
        return out;
    };
    for repo in repos.flatten() {
        let Ok(revisions) = fs::read_dir(repo.path()) else {
            continue;
        };
        for rev in revisions.flatten() {
            let dir = rev.path();
            if let Ok(manifest) = read_manifest(&dir) {
                if dir == install_dir(models, &manifest.repo, &manifest.revision) {
                    out.push(InstalledModel {
                        files_present: files_present(&dir, &manifest),
                        manifest,
                        dir,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| {
        b.manifest
            .installed_unix_ms
            .cmp(&a.manifest.installed_unix_ms)
    });
    out
}

pub fn find_installed(models: &Path, repo: &str, revision: &str) -> Option<InstalledModel> {
    let dir = install_dir(models, repo, revision);
    let manifest = read_manifest(&dir).ok()?;
    (manifest.repo == repo && manifest.revision == revision).then(|| InstalledModel {
        files_present: files_present(&dir, &manifest),
        manifest,
        dir,
    })
}

/// Remove one installed revision. Only folders holding a valid manifest inside the model folder.
pub fn remove(models: &Path, repo: &str, revision: &str) -> Result<(), String> {
    if !hf_catalog::valid_repo_id(repo) || !hf_catalog::valid_revision_sha(revision) {
        return Err("Invalid model identity.".into());
    }
    let dir = install_dir(models, repo, revision);
    read_manifest(&dir)?;
    let meta = fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Refusing to remove a link or non-folder.".into());
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("Could not remove the model: {e}"))?;
    let _ = fs::remove_dir(dir.parent().unwrap_or(models)); // only succeeds when empty
    Ok(())
}

/// The installer hashes model files once, but their contents can be changed afterwards.
/// Verify every file again before a *fresh worker load*, not between warm image jobs.
/// The snapshot is not a defense against another local process replacing a file
/// after verification. Never treat size-only installed-state checks as integrity proof.
pub fn verify_installed_files(model: &InstalledModel) -> Result<(), String> {
    let dir = &model.dir;
    let manifest = &model.manifest;
    if manifest.files.is_empty() {
        return Err("The installed model has no recorded files; reinstall it.".into());
    }
    verify_real_directory(dir)?;
    verify_real_directory(
        dir.parent()
            .ok_or("The model installation folder is invalid.")?,
    )?;
    for file in &manifest.files {
        if !safe_relative(&file.path)
            || file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        {
            return Err(
                "The model manifest contains an invalid path or digest; reinstall it.".into(),
            );
        }

        // Reject in-tree symlinks and Windows junctions before hashing. Without this
        // check, the file could be redirected outside the user-approved model folder.
        let mut path = dir.to_path_buf();
        let segments: Vec<_> = file.path.split('/').collect();
        for segment in &segments[..segments.len() - 1] {
            path.push(segment);
            verify_real_directory(&path)?;
        }
        path.push(segments[segments.len() - 1]);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| format!("A model file is missing: {}.", file.path))?;
        if !metadata.file_type().is_file()
            || metadata.file_type().is_symlink()
            || file_is_reparse_point(&metadata)
        {
            return Err(format!(
                "A model file is a link or not a regular file: {}.",
                file.path
            ));
        }
        if metadata.len() != file.size {
            return Err(format!(
                "A model file changed size after installation: {}.",
                file.path
            ));
        }
        let (actual, _) = hash_file(&path, false)
            .map_err(|_| format!("Could not verify model file: {}.", file.path))?;
        if actual != file.sha256 {
            return Err(format!(
                "Model file integrity check failed for {}. Remove and reinstall this revision.",
                file.path
            ));
        }
    }
    Ok(())
}

fn verify_real_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "A model installation directory is missing.".to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || file_is_reparse_point(&metadata) {
        return Err("A model installation directory is a link or reparse point.".into());
    }
    Ok(())
}

#[cfg(windows)]
fn file_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn file_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

/// Hash a whole file with SHA-256 (and git-blob SHA-1 when asked).
fn hash_file(path: &Path, git_blob: bool) -> std::io::Result<(String, Option<String>)> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut sha256 = Sha256::new();
    let mut sha1 = git_blob.then(|| {
        let mut h = Sha1::new();
        h.update(format!("blob {len}\0").as_bytes());
        h
    });
    let mut buf = vec![0u8; 4 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        sha256.update(&buf[..n]);
        if let Some(h) = sha1.as_mut() {
            h.update(&buf[..n]);
        }
    }
    Ok((hex(&sha256.finalize()), sha1.map(|h| hex(&h.finalize()))))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify a downloaded file against the hash Hugging Face published for it.
fn verify_file(path: &Path, expected: &RepoFile) -> Result<String, String> {
    let len = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len != expected.size {
        return Err(format!(
            "{} has {len} bytes, expected {}.",
            expected.path, expected.size
        ));
    }
    let (sha256, sha1) =
        hash_file(path, expected.lfs_sha256.is_none()).map_err(|e| e.to_string())?;
    match (&expected.lfs_sha256, &expected.blob_id, sha1) {
        (Some(want), _, _) if *want == sha256 => Ok(sha256),
        (None, Some(want), Some(got)) if *want == got => Ok(sha256),
        _ => Err(format!(
            "{} failed integrity verification; it was discarded.",
            expected.path
        )),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallEvent {
    pub install_id: String,
    pub repo: String,
    pub state: &'static str,
    pub file: Option<String>,
    pub file_index: usize,
    pub file_count: usize,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub message: Option<String>,
}

/// The one native model download allowed at a time. It is claimed before the pre-download
/// inspection, so a Cancel during that window is honoured. Previously the cancel flag was
/// registered only after inspection, and an early Cancel returned `false` and was lost. It is
/// released on every exit path by [`ActiveGuard`]. Kept in memory only: no download history
/// is persisted.
struct Active {
    install_id: String,
    repo: String,
    revision: String,
    /// `None` until the pinned source inspection completes: unknown, so the UI must conceal.
    source_flagged: Option<bool>,
    cancel: Arc<AtomicBool>,
    last: Option<InstallEvent>,
}

fn active() -> &'static Mutex<Option<Active>> {
    static SLOT: OnceLock<Mutex<Option<Active>>> = OnceLock::new();
    SLOT.get_or_init(Default::default)
}

/// Read-only view of the active download, so a remounted or reloaded UI can show
/// progress and a working Cancel for the exact native `install_id`.
#[derive(Debug, Clone, Serialize)]
pub struct InstallStatus {
    pub install_id: String,
    pub repo: String,
    pub revision: String,
    /// Source "not-for-all-audiences" flag: `null` until the pinned inspection completes.
    pub source_flagged: Option<bool>,
    pub cancel_requested: bool,
    pub last: Option<InstallEvent>,
}

pub fn status() -> Option<InstallStatus> {
    active().lock().unwrap().as_ref().map(|a| InstallStatus {
        install_id: a.install_id.clone(),
        repo: a.repo.clone(),
        revision: a.revision.clone(),
        source_flagged: a.source_flagged,
        cancel_requested: a.cancel.load(Ordering::SeqCst),
        last: a.last.clone(),
    })
}

/// Releases the slot only if it still belongs to this install.
#[derive(Debug)]
struct ActiveGuard {
    install_id: String,
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        let mut slot = active().lock().unwrap_or_else(|e| e.into_inner());
        if slot
            .as_ref()
            .is_some_and(|a| a.install_id == self.install_id)
        {
            *slot = None;
        }
    }
}

fn claim(req: &InstallRequest) -> Result<(ActiveGuard, Arc<AtomicBool>), String> {
    let mut slot = active().lock().unwrap();
    if slot.is_some() {
        return Err(
            "Another model download is already in progress. Wait for it or cancel it first.".into(),
        );
    }
    let flag = Arc::new(AtomicBool::new(false));
    *slot = Some(Active {
        install_id: req.install_id.clone(),
        repo: req.repo.clone(),
        revision: req.revision.clone(),
        source_flagged: None,
        cancel: flag.clone(),
        last: None,
    });
    Ok((
        ActiveGuard {
            install_id: req.install_id.clone(),
        },
        flag,
    ))
}

fn update_active(install_id: &str, change: impl FnOnce(&mut Active)) {
    if let Some(a) = active().lock().unwrap().as_mut() {
        if a.install_id == install_id {
            change(a);
        }
    }
}

/// Cancels only the exact active install. A stale or unknown id is refused (`false`).
pub fn cancel(install_id: &str) -> bool {
    active()
        .lock()
        .unwrap()
        .as_ref()
        .filter(|a| a.install_id == install_id)
        .map(|a| a.cancel.store(true, Ordering::SeqCst))
        .is_some()
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallRequest {
    pub install_id: String,
    pub repo: String,
    pub revision: String,
    pub adapter: String,
    /// Must equal the license shown in the inspection (or "unknown").
    pub license_acknowledged: String,
    pub accept: bool,
}

struct Ctx<'a, R: tauri::Runtime> {
    app: &'a tauri::AppHandle<R>,
    req: &'a InstallRequest,
    cancel: Arc<AtomicBool>,
    file_count: usize,
    bytes_total: u64,
    last_emit: Instant,
}

impl<R: tauri::Runtime> Ctx<'_, R> {
    fn emit(
        &mut self,
        state: &'static str,
        file: Option<&RepoFile>,
        index: usize,
        done: u64,
        message: Option<String>,
        force: bool,
    ) {
        if !force && self.last_emit.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.last_emit = Instant::now();
        let event = InstallEvent {
            install_id: self.req.install_id.clone(),
            repo: self.req.repo.clone(),
            state,
            file: file.map(|f| f.path.clone()),
            file_index: index,
            file_count: self.file_count,
            bytes_done: done,
            bytes_total: self.bytes_total,
            message,
        };
        update_active(&self.req.install_id, |a| a.last = Some(event.clone()));
        let _ = self.app.emit("model-install", event);
    }
}

#[cfg(windows)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0u64;
    // SAFETY: valid NUL-terminated wide string and out-pointer; null for unused outputs.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
}

#[cfg(not(windows))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

/// Run an approved install end to end. Returns the manifest only after full verification.
pub async fn install<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    models: &Path,
    req: InstallRequest,
) -> Result<Manifest, String> {
    if !req.accept {
        return Err("Installation needs your explicit approval.".into());
    }
    if !hf_catalog::valid_repo_id(&req.repo)
        || !hf_catalog::valid_revision_sha(&req.revision)
        || req.install_id.len() > 64
    {
        return Err("Install requests need a model id and an exact 40-character revision.".into());
    }
    let adapter = adapters::adapter(&req.adapter).ok_or("Unknown adapter.")?;
    // One download at a time, claimed before any network work so Cancel is honoured from now on.
    let (_guard, flag) = claim(&req)?;
    let cancelled_early =
        || "Installation cancelled before any model file was downloaded.".to_string();
    // Re-inspect at the pinned revision; never trust a plan supplied by the UI.
    let detail = hf_catalog::inspect(&req.repo, &req.revision, adapter.capability).await?;
    update_active(&req.install_id, |a| {
        a.source_flagged = Some(detail.audience == "not-for-all-audiences")
    });
    if flag.load(Ordering::SeqCst) {
        return Err(cancelled_early());
    }
    if detail.revision != req.revision {
        return Err("The repository revision changed; inspect it again.".into());
    }
    let license = detail.license.clone().unwrap_or_else(|| "unknown".into());
    if req.license_acknowledged != license {
        return Err(
            "The license you acknowledged does not match this revision's license. Review it again."
                .into(),
        );
    }
    if detail.access == Access::GatedNeedsTerms {
        return Err("This gated model is not accessible with your credentials. Accept its terms on Hugging Face first.".into());
    }
    let plan: InstallPlan = detail
        .plans
        .into_iter()
        .map(|p| p.plan)
        .find(|p| p.adapter == adapter.id)
        .ok_or("This model cannot be run by the selected adapter.")?;
    let dir = install_dir(models, &req.repo, &req.revision);
    if read_manifest(&dir).is_ok() {
        return Err("This revision is already installed.".into());
    }
    let partial = dir.join(PARTIAL_DIR);
    fs::create_dir_all(&partial).map_err(|e| format!("Cannot create the model folder: {e}"))?;
    let already: u64 = plan
        .files
        .iter()
        .filter_map(|f| {
            fs::metadata(partial.join(&f.path))
                .ok()
                .map(|m| m.len().min(f.size))
        })
        .sum();
    if let Some(free) = free_bytes(&partial) {
        let needed = plan.total_bytes.saturating_sub(already) + DISK_MARGIN;
        if free < needed {
            return Err(format!(
                "Not enough disk space: {:.1} GB free, {:.1} GB needed (including a 2 GB margin).",
                free as f64 / 1e9,
                needed as f64 / 1e9
            ));
        }
    }
    let token = if detail.access == Access::GatedAccessible {
        USER_SLOT.token()?
    } else {
        None
    };
    if flag.load(Ordering::SeqCst) {
        return Err(cancelled_early());
    }
    let mut ctx = Ctx {
        app,
        req: &req,
        cancel: flag,
        file_count: plan.files.len(),
        bytes_total: plan.total_bytes,
        last_emit: Instant::now() - Duration::from_secs(1),
    };
    let result = download_all(&mut ctx, &plan, &dir, &partial, token.as_deref()).await;
    let files = match result {
        Ok(files) => files,
        Err(e) => {
            let cancelled = ctx.cancel.load(Ordering::SeqCst);
            ctx.emit(
                if cancelled { "cancelled" } else { "failed" },
                None,
                0,
                0,
                Some(e.clone()),
                true,
            );
            return Err(if cancelled {
                "Installation cancelled. Downloaded parts are kept so you can resume.".into()
            } else {
                e
            });
        }
    };
    ctx.emit(
        "validating",
        None,
        files.len(),
        plan.total_bytes,
        None,
        true,
    );
    // Move verified files into place, then write the manifest last.
    for f in &plan.files {
        let target = dir.join(&f.path);
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::rename(partial.join(&f.path), &target)
            .map_err(|e| format!("Could not finalise {}: {e}", f.path))?;
    }
    let manifest = Manifest {
        manifest_version: MANIFEST_VERSION,
        repo: req.repo.clone(),
        revision: req.revision.clone(),
        adapter: adapter.id.into(),
        adapter_version: adapter.version,
        variant: plan.variant.clone(),
        license: detail.license.clone(),
        license_name: detail.license_name.clone(),
        base_models: detail.base_models.clone(),
        consent: Consent {
            license_acknowledged: license,
            gated: detail.gated.clone(),
            audience: detail.audience.into(),
            acknowledged_unix_ms: now_ms(),
        },
        files,
        total_bytes: plan.total_bytes,
        installed_unix_ms: now_ms(),
    };
    let tmp = dir.join(format!("{MANIFEST}.tmp"));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&tmp, dir.join(MANIFEST)).map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&partial);
    ctx.emit(
        "installed",
        None,
        plan.files.len(),
        plan.total_bytes,
        None,
        true,
    );
    Ok(manifest)
}

async fn download_all<R: tauri::Runtime>(
    ctx: &mut Ctx<'_, R>,
    plan: &InstallPlan,
    dir: &Path,
    partial: &Path,
    token: Option<&str>,
) -> Result<Vec<InstalledFile>, String> {
    let http = hf_catalog::client(Duration::from_secs(60 * 60 * 6))?;
    let mut installed = Vec::new();
    let mut done_before = 0u64;
    for (index, file) in plan.files.iter().enumerate() {
        if !safe_relative(&file.path) {
            return Err(format!("Unsafe file path {}.", file.path));
        }
        let dest = partial.join(&file.path);
        fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        let url = format!(
            "{HUB}/{}/resolve/{}/{}",
            ctx.req.repo, ctx.req.revision, file.path
        );
        let mut attempt = 0;
        loop {
            attempt += 1;
            let outcome =
                download_one(ctx, &http, &url, token, &dest, file, index, done_before).await;
            match outcome {
                Ok(()) => break,
                Err(e) if ctx.cancel.load(Ordering::SeqCst) => return Err(e),
                Err(e) if attempt < 3 && e.starts_with("network:") => {
                    ctx.emit("retrying", Some(file), index, done_before, Some(e), true);
                    tokio_sleep(Duration::from_secs(2 * attempt)).await;
                }
                Err(e) => return Err(e.trim_start_matches("network:").trim().to_string()),
            }
        }
        ctx.emit(
            "verifying",
            Some(file),
            index,
            done_before + file.size,
            None,
            true,
        );
        let (dest2, expected) = (dest.clone(), file.clone());
        let sha256 =
            match tauri::async_runtime::spawn_blocking(move || verify_file(&dest2, &expected))
                .await
                .map_err(|e| e.to_string())?
            {
                Ok(sha) => sha,
                Err(e) => {
                    let _ = fs::remove_file(&dest);
                    return Err(e);
                }
            };
        installed.push(InstalledFile {
            path: file.path.clone(),
            size: file.size,
            sha256,
        });
        done_before += file.size;
        let _ = dir; // final placement happens after every file verifies
    }
    Ok(installed)
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

#[allow(clippy::too_many_arguments)]
async fn download_one<R: tauri::Runtime>(
    ctx: &mut Ctx<'_, R>,
    http: &reqwest::Client,
    url: &str,
    token: Option<&str>,
    dest: &Path,
    file: &RepoFile,
    index: usize,
    done_before: u64,
) -> Result<(), String> {
    let mut have = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
    if have > file.size {
        fs::remove_file(dest).map_err(|e| e.to_string())?;
        have = 0;
    }
    if have == file.size {
        return Ok(());
    }
    let mut request = http.get(url);
    if let Some(t) = token {
        request = request.bearer_auth(t);
    }
    if have > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={have}-"));
    }
    let mut response = request.send().await.map_err(|e| {
        format!(
            "network: {}",
            if e.is_timeout() {
                "timed out"
            } else {
                "connection failed"
            }
        )
    })?;
    let status = response.status().as_u16();
    let mut out = match status {
        206 if have > 0 => OpenOptions::new()
            .append(true)
            .open(dest)
            .map_err(|e| e.to_string())?,
        200 => {
            have = 0;
            File::create(dest).map_err(|e| e.to_string())?
        }
        401 | 403 => {
            return Err("Hugging Face refused the download (access or terms required).".into())
        }
        404 => {
            return Err(format!(
                "{} is no longer available at this revision.",
                file.path
            ))
        }
        416 => {
            fs::remove_file(dest).map_err(|e| e.to_string())?;
            return Err("network: resume offset rejected; restarting file".into());
        }
        s if s >= 500 => return Err(format!("network: Hugging Face storage error (HTTP {s})")),
        s => {
            return Err(format!(
                "Unexpected HTTP {s} while downloading {}.",
                file.path
            ))
        }
    };
    ctx.emit(
        "downloading",
        Some(file),
        index,
        done_before + have,
        None,
        true,
    );
    loop {
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err("Cancelled.".into());
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                have += chunk.len() as u64;
                if have > file.size {
                    drop(out);
                    let _ = fs::remove_file(dest);
                    return Err(format!(
                        "{} is larger than published; discarded.",
                        file.path
                    ));
                }
                out.write_all(&chunk)
                    .map_err(|e| format!("Write failed (disk full?): {e}"))?;
                ctx.emit(
                    "downloading",
                    Some(file),
                    index,
                    done_before + have,
                    None,
                    false,
                );
            }
            Ok(None) => break,
            Err(e) => {
                return Err(format!(
                    "network: download interrupted ({})",
                    if e.is_timeout() {
                        "timed out"
                    } else {
                        "connection lost"
                    }
                ))
            }
        }
    }
    out.sync_all().map_err(|e| e.to_string())?;
    if have != file.size {
        return Err(format!("network: {} ended early", file.path));
    }
    Ok(())
}

pub fn dirs<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<(PathBuf, PathBuf), String> {
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let local = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    Ok((data, local))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The active-download slot is process-global; serialise the tests that use it.
    fn slot_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn install_req(id: &str) -> InstallRequest {
        InstallRequest {
            install_id: id.into(),
            repo: "example/model".into(),
            revision: "a".repeat(40),
            adapter: "diffusers-sdxl-text-to-image".into(),
            license_acknowledged: "unknown".into(),
            accept: true,
        }
    }

    #[test]
    fn one_download_at_a_time_and_cancel_targets_the_exact_active_id() {
        let _l = slot_lock();
        assert!(status().is_none());
        assert!(!cancel("nothing-running"));
        let (guard, flag) = claim(&install_req("first")).unwrap();
        // A second install (e.g. from a reloaded UI) is refused rather than racing the same files.
        assert!(claim(&install_req("second"))
            .unwrap_err()
            .contains("already in progress"));
        let s = status().unwrap();
        assert_eq!(
            (s.install_id.as_str(), s.repo.as_str(), s.cancel_requested),
            ("first", "example/model", false)
        );
        // A stale id never cancels the active download.
        assert!(!cancel("second"));
        assert!(!flag.load(Ordering::SeqCst));
        // Cancel before any network work is recorded (previously lost before registration).
        assert!(cancel("first"));
        assert!(flag.load(Ordering::SeqCst));
        assert!(status().unwrap().cancel_requested);
        // Unknown until the pinned inspection completes, never a premature "not flagged".
        assert_eq!(status().unwrap().source_flagged, None);
        update_active("first", |a| a.source_flagged = Some(true));
        update_active("other", |a| a.source_flagged = Some(false));
        assert_eq!(status().unwrap().source_flagged, Some(true));
        drop(guard);
        assert!(status().is_none(), "released on every exit path");
        assert!(!cancel("first"), "a finished id is stale");
        let (_g, _) = claim(&install_req("third")).unwrap();
        assert_eq!(status().unwrap().install_id, "third");
    }

    #[test]
    fn a_stale_guard_never_releases_a_newer_download() {
        let _l = slot_lock();
        let stale = ActiveGuard {
            install_id: "old".into(),
        };
        let (_g, _) = claim(&install_req("new")).unwrap();
        drop(stale);
        assert_eq!(status().unwrap().install_id, "new");
    }

    fn scratch() -> PathBuf {
        let p = std::env::temp_dir().join(format!("optic-rift-models-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn verify_accepts_matching_lfs_and_git_blob_hashes_only() {
        let dir = scratch();
        let path = dir.join("f.json");
        fs::write(&path, b"hello\n").unwrap();
        // sha256("hello\n") and git hash-object of "hello\n".
        let lfs = RepoFile {
            path: "f.json".into(),
            size: 6,
            lfs_sha256: Some(
                "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03".into(),
            ),
            blob_id: None,
        };
        let blob = RepoFile {
            path: "f.json".into(),
            size: 6,
            lfs_sha256: None,
            blob_id: Some("ce013625030ba8dba906f756967f9e9ca394464a".into()),
        };
        assert!(verify_file(&path, &lfs).is_ok());
        assert!(verify_file(&path, &blob).is_ok());
        let wrong = RepoFile {
            lfs_sha256: Some("0".repeat(64)),
            ..lfs.clone()
        };
        assert!(verify_file(&path, &wrong)
            .unwrap_err()
            .contains("integrity"));
        let short = RepoFile { size: 7, ..lfs };
        assert!(verify_file(&path, &short).unwrap_err().contains("bytes"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manifest_paths_and_identity_are_validated() {
        for bad in ["../x", "/abs", "a\\b", "C:x", "a//b", "a/./b", ""] {
            assert!(!safe_relative(bad), "{bad}");
        }
        assert!(safe_relative(
            "unet/diffusion_pytorch_model.fp16.safetensors"
        ));
        let dir = scratch();
        fs::write(
            dir.join(MANIFEST),
            br#"{"manifest_version":1,"repo":"../../x","revision":"main"}"#,
        )
        .unwrap();
        assert!(read_manifest(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn listing_ignores_folders_without_a_valid_manifest_and_remove_is_scoped() {
        let models = scratch();
        let rev = "a".repeat(40);
        let dir = install_dir(&models, "owner/name", &rev);
        fs::create_dir_all(&dir).unwrap();
        fs::create_dir_all(models.join("junk/zzz")).unwrap();
        assert!(list_installed(&models).is_empty());
        let manifest = Manifest {
            manifest_version: 1,
            repo: "owner/name".into(),
            revision: rev.clone(),
            adapter: "diffusers-sdxl-text-to-image".into(),
            adapter_version: 1,
            variant: None,
            license: Some("apache-2.0".into()),
            license_name: None,
            base_models: None,
            consent: Consent {
                license_acknowledged: "apache-2.0".into(),
                gated: "no".into(),
                audience: "unverified".into(),
                acknowledged_unix_ms: 1,
            },
            files: vec![InstalledFile {
                path: "model_index.json".into(),
                size: 2,
                sha256: "x".into(),
            }],
            total_bytes: 2,
            installed_unix_ms: 1,
        };
        fs::write(dir.join(MANIFEST), serde_json::to_vec(&manifest).unwrap()).unwrap();
        let listed = list_installed(&models);
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].files_present);
        fs::write(dir.join("model_index.json"), b"{}").unwrap();
        assert!(
            find_installed(&models, "owner/name", &rev)
                .unwrap()
                .files_present
        );
        assert!(remove(&models, "owner/name", "main").is_err());
        assert!(remove(&models, "../escape", &rev).is_err());
        remove(&models, "owner/name", &rev).unwrap();
        assert!(!dir.exists());
        assert!(models.join("junk").exists(), "unrelated folders untouched");
        fs::remove_dir_all(models).unwrap();
    }

    #[test]
    fn model_dir_setting_requires_absolute_writable_folder() {
        let data = scratch();
        assert!(set_model_dir(&data, "relative\\models").is_err());
        assert!(set_model_dir(&data, "   ").is_err());
        let target = data.join("My Models");
        let chosen = set_model_dir(&data, target.to_str().unwrap()).unwrap();
        assert!(chosen.ends_with("My Models"));
        assert_eq!(load_settings(&data).model_dir.unwrap(), chosen);
        assert_eq!(model_dir(&data, Path::new("C:\\unused")), chosen);
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn reverified_model_rejects_same_size_tampering_before_worker_load() {
        let models = scratch();
        let revision = "a".repeat(40);
        let repo = "owner/name";
        let dir = install_dir(&models, repo, &revision);
        fs::create_dir_all(dir.join("unet")).unwrap();
        let relative = "unet/model.safetensors";
        let content = b"known verified model data";
        let path = dir.join(relative);
        fs::write(&path, content).unwrap();
        let digest = hex(&Sha256::digest(content));
        let manifest = Manifest {
            manifest_version: MANIFEST_VERSION,
            repo: repo.into(),
            revision,
            adapter: "diffusers-sdxl-text-to-image".into(),
            adapter_version: 1,
            variant: None,
            license: Some("apache-2.0".into()),
            license_name: None,
            base_models: None,
            consent: Consent {
                license_acknowledged: "apache-2.0".into(),
                gated: "no".into(),
                audience: "unverified".into(),
                acknowledged_unix_ms: 1,
            },
            files: vec![InstalledFile {
                path: relative.into(),
                size: content.len() as u64,
                sha256: digest,
            }],
            total_bytes: content.len() as u64,
            installed_unix_ms: 1,
        };
        let model = InstalledModel {
            manifest,
            dir: dir.clone(),
            files_present: true,
        };
        assert!(verify_installed_files(&model).is_ok());
        let altered = b"wrong verified model data";
        assert_eq!(altered.len(), content.len());
        fs::write(&path, altered).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), content.len() as u64);
        assert!(verify_installed_files(&model)
            .unwrap_err()
            .contains("integrity check failed"));
        fs::remove_dir_all(models).unwrap();
    }

    #[test]
    fn reverified_model_rejects_invalid_manifest_digest_and_missing_file() {
        let models = scratch();
        let revision = "b".repeat(40);
        let dir = install_dir(&models, "owner/other", &revision);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("model_index.json");
        fs::write(&path, b"{}").unwrap();
        let digest = hex(&Sha256::digest(b"{}"));
        let mut manifest = Manifest {
            manifest_version: MANIFEST_VERSION,
            repo: "owner/other".into(),
            revision,
            adapter: "diffusers-sdxl-text-to-image".into(),
            adapter_version: 1,
            variant: None,
            license: None,
            license_name: None,
            base_models: None,
            consent: Consent {
                license_acknowledged: "unknown".into(),
                gated: "no".into(),
                audience: "unverified".into(),
                acknowledged_unix_ms: 1,
            },
            files: vec![InstalledFile {
                path: "model_index.json".into(),
                size: 2,
                sha256: digest,
            }],
            total_bytes: 2,
            installed_unix_ms: 1,
        };
        manifest.files[0].sha256 = "invalid".into();
        let mut model = InstalledModel {
            manifest,
            dir,
            files_present: true,
        };
        assert!(verify_installed_files(&model)
            .unwrap_err()
            .contains("invalid path or digest"));
        model.manifest.files[0].sha256 = hex(&Sha256::digest(b"{}"));
        fs::remove_file(path).unwrap();
        assert!(verify_installed_files(&model)
            .unwrap_err()
            .contains("missing"));
        fs::remove_dir_all(models).unwrap();
    }

    #[test]
    fn free_space_is_reported_on_windows() {
        if cfg!(windows) {
            assert!(free_bytes(&std::env::temp_dir()).unwrap() > 0);
        }
    }
}
