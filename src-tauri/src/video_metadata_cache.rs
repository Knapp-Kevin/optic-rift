//! Conservative public video-model metadata caching. This is deliberately NOT
//! a weight downloader or inference adapter. The only permitted payload is a
//! small, validated, hash-addressed JSON configuration at an immutable revision.
//! Public-only: no credentials and no redirect-following at this boundary.

use crate::adapters::{Capability, RepoFile};
use crate::hf_catalog::{self, ModelDetail};
use serde::{Deserialize, Serialize};
use sha1::{Digest as _, Sha1};
use sha2::Sha256;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_JSON_BYTES: usize = 256 * 1024;
const CACHE_DIR: &str = "video-model-metadata";
const RECEIPT_NAME: &str = "optic-rift-metadata-receipt.json";

#[derive(Clone, Debug, Deserialize)]
pub struct CacheRequest {
    pub repo: String,
    pub revision: String,
    pub capability: Capability,
    pub license_acknowledged: String,
    pub adult_acknowledged: bool,
    pub accept: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheReceipt {
    pub repo: String,
    pub revision: String,
    pub capability: Capability,
    pub source_license: String,
    pub source_audience: String,
    pub filename: String,
    pub sha256: String,
    pub bytes: u64,
    pub cached_unix_ms: u64,
    /// File data only: never infer runtime support or downloadable weights.
    pub status: String,
}

fn is_video(capability: Capability) -> bool {
    matches!(
        capability,
        Capability::TextToVideo | Capability::ImageToVideo | Capability::VideoToVideo
    )
}

/// The UI's consent fields are rechecked by native code against fresh metadata
/// at the exact pinned revision. No trust is placed in client-supplied file lists.
pub fn admit(request: &CacheRequest, detail: &ModelDetail) -> Result<RepoFile, String> {
    if !request.accept {
        return Err("Download configuration only after explicit approval.".into());
    }
    if !is_video(request.capability) {
        return Err("This metadata-only cache is for video model discovery.".into());
    }
    if !hf_catalog::valid_repo_id(&request.repo)
        || !hf_catalog::valid_revision_sha(&request.revision)
        || detail.id != request.repo
        || detail.revision != request.revision
        || detail.capability != request.capability
    {
        return Err("Model identity or pinned revision does not match inspection.".into());
    }
    // Gated metadata is a different authorization path. No token is ever
    // placed in this download-only request or forwarded across redirects.
    if detail.gated != "no" {
        return Err("Gated video configuration needs separate authorized support; no download was started.".into());
    }
    let license = detail.license.as_deref().unwrap_or("unknown");
    if request.license_acknowledged != license {
        return Err("Review the model's current license and acknowledge it again.".into());
    }
    if detail.audience == "not-for-all-audiences" && !request.adult_acknowledged {
        return Err("This source-flagged 18+ model needs explicit acknowledgment.".into());
    }
    let candidate = detail.cache_metadata_file.clone().ok_or(
        "No small, integrity-identifiable configuration file is available for this model.".to_string()
    )?;
    if !matches!(candidate.path.as_str(), "model_index.json" | "config.json")
        || candidate.size == 0
        || candidate.size > MAX_JSON_BYTES as u64
        || !(candidate.blob_id.as_ref().is_some_and(|v| hf_catalog::valid_hash(v, 40))
            || candidate.lfs_sha256.as_ref().is_some_and(|v| hf_catalog::valid_hash(v, 64)))
    {
        return Err("Model configuration does not meet the safe metadata cache policy.".into());
    }
    Ok(candidate)
}

fn git_blob_id(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn digest_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn verify_download(bytes: &[u8], file: &RepoFile) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_JSON_BYTES || bytes.len() as u64 != file.size {
        return Err("Configuration download has an unexpected size.".into());
    }
    if let Some(sha) = &file.lfs_sha256 {
        if digest_sha256(bytes) != *sha {
            return Err("Configuration SHA-256 does not match source metadata.".into());
        }
    } else if let Some(blob) = &file.blob_id {
        if git_blob_id(bytes) != *blob {
            return Err("Configuration Git blob identity does not match source metadata.".into());
        }
    } else {
        return Err("Configuration lacks source integrity evidence.".into());
    }
    serde_json::from_slice::<serde_json::Value>(bytes)
        .map_err(|_| "Source configuration was not valid JSON.".to_string())?;
    Ok(())
}

/// Anonymous HTTPS only, with redirects refused. The exact path is chosen by
/// the native allowlist and never comes from an arbitrary user-provided URL.
async fn read_public_config(repo: &str, revision: &str, file: &RepoFile) -> Result<Vec<u8>, String> {
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(25))
        .user_agent(concat!("OpticRift/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Could not establish a public metadata connection.".to_string())?;
    let url = format!("{}/{repo}/resolve/{revision}/{}", hf_catalog::HUB, file.path);
    let mut response = client.get(&url)
        .send().await
        .map_err(|_| "Could not reach the public metadata file.".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "Public metadata request was not approved (HTTP {}). Redirects or gated access are not followed.",
            response.status().as_u16()
        ));
    }
    if response.content_length().is_some_and(|len| len > MAX_JSON_BYTES as u64 || len != file.size) {
        return Err("Public metadata response exceeded the expected size.".into());
    }
    let mut data = Vec::with_capacity((file.size as usize).min(MAX_JSON_BYTES));
    while let Some(chunk) = response.chunk().await
        .map_err(|_| "Public metadata transfer stopped.".to_string())? {
        if data.len().saturating_add(chunk.len()) > MAX_JSON_BYTES {
            return Err("Public metadata transfer exceeded the safe byte limit.".into());
        }
        data.extend_from_slice(&chunk);
    }
    verify_download(&data, file)?;
    Ok(data)
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_metadata: &fs::Metadata) -> bool { false }

fn checked_dir(path: &Path) -> Result<(), String> {
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Could not create the metadata cache folder.".into()),
    }
    let m = fs::symlink_metadata(path)
        .map_err(|_| "Cannot inspect the metadata cache folder.".to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() || is_reparse(&m) {
        return Err("Metadata cache path is redirected or unsafe.".into());
    }
    Ok(())
}

fn cache_folder(app_local: &Path, repo: &str, revision: &str) -> Result<PathBuf, String> {
    if !hf_catalog::valid_repo_id(repo) || !hf_catalog::valid_revision_sha(revision) {
        return Err("Invalid cache model identity.".into());
    }
    // The local application directory itself must be a regular directory.
    checked_dir(app_local)?;
    let root = app_local.join(CACHE_DIR);
    checked_dir(&root)?;
    let repo_dir = root.join(digest_sha256(repo.as_bytes()));
    checked_dir(&repo_dir)?;
    let rev_dir = repo_dir.join(revision);
    checked_dir(&rev_dir)?;
    Ok(rev_dir)
}

fn safe_read_file(path: &Path, bound: u64) -> Result<Vec<u8>, String> {
    let m = fs::symlink_metadata(path)
        .map_err(|_| "Metadata cache file is missing.".to_string())?;
    if !m.is_file() || m.file_type().is_symlink() || is_reparse(&m) || m.len() > bound {
        return Err("Metadata cache file is redirected or too large.".into());
    }
    let mut buf = Vec::new();
    File::open(path)
        .and_then(|f| f.take(bound + 1).read_to_end(&mut buf))
        .map_err(|_| "Could not read the metadata cache file.".to_string())?;
    if buf.len() as u64 > bound {
        return Err("Metadata cache file changed size during reading.".into());
    }
    Ok(buf)
}

fn publish_new_file(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    if !matches!(name, "model_index.json" | "config.json" | RECEIPT_NAME) {
        return Err("Refusing an unexpected metadata cache filename.".into());
    }
    let destination = dir.join(name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err("Metadata cache destination exists; no file was overwritten.".into());
    }
    let temp = dir.join(format!(".{}.partial-{}", name, uuid::Uuid::new_v4()));
    let mut output = OpenOptions::new().write(true).create_new(true).open(&temp)
        .map_err(|_| "Cannot create the metadata staging file.".to_string())?;
    let result = (|| -> Result<(), String> {
        output.write_all(bytes).and_then(|_| output.sync_all())
            .map_err(|_| "Could not finish the metadata staging write.".to_string())?;
        drop(output);
        fs::hard_link(&temp, &destination)
            .map_err(|_| "Metadata destination already exists or cannot be published safely.".to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp);
    result
}

/// A cache receipt is shown only when both files are regular, size-bounded and
/// the payload matches both the receipt SHA-256 and the inspected upstream hash.
pub fn cached_receipt(
    app_local: &Path, detail: &ModelDetail,
) -> Option<CacheReceipt> {
    let file = detail.cache_metadata_file.as_ref()?;
    let dir = cache_folder(app_local, &detail.id, &detail.revision).ok()?;
    let bytes = safe_read_file(&dir.join(&file.path), MAX_JSON_BYTES as u64).ok()?;
    verify_download(&bytes, file).ok()?;
    let receipt_bytes = safe_read_file(&dir.join(RECEIPT_NAME), 16 * 1024).ok()?;
    let receipt: CacheReceipt = serde_json::from_slice(&receipt_bytes).ok()?;
    (receipt.repo == detail.id
        && receipt.revision == detail.revision
        && receipt.capability == detail.capability
        && receipt.filename == file.path
        && receipt.bytes == bytes.len() as u64
        && receipt.sha256 == digest_sha256(&bytes)
        && receipt.status == "metadata-only")
        .then_some(receipt)
}

/// Safe, read-only metadata fetch then verified local publication.
/// This does not install weights, make a model runnable or consume GPU.
pub async fn cache_public_metadata(
    app_local: PathBuf, request: CacheRequest,
) -> Result<CacheReceipt, String> {
    if !hf_catalog::valid_repo_id(&request.repo)
        || !hf_catalog::valid_revision_sha(&request.revision)
        || !is_video(request.capability)
        || !request.accept
    {
        return Err("Cache request lacks a valid model revision, capability or approval.".into());
    }
    // Re-read public source metadata at the pinned commit. A UI-provided
    // inspection, hash, path or license is never sufficient authorization.
    let detail = hf_catalog::inspect(&request.repo, &request.revision, request.capability).await?;
    let file = admit(&request, &detail)?;
    let bytes = read_public_config(&request.repo, &request.revision, &file).await?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as u64).unwrap_or(0);
    let receipt = CacheReceipt {
        repo: detail.id.clone(),
        revision: detail.revision.clone(),
        capability: detail.capability,
        source_license: detail.license.clone().unwrap_or_else(|| "unknown".into()),
        source_audience: detail.audience.into(),
        filename: file.path.clone(),
        sha256: digest_sha256(&bytes),
        bytes: bytes.len() as u64,
        cached_unix_ms: now,
        status: "metadata-only".into(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        let dir = cache_folder(&app_local, &detail.id, &detail.revision)?;
        if cached_receipt(&app_local, &detail).is_some() {
            return Err("Configuration metadata is already cached and verified.".into());
        }
        publish_new_file(&dir, &file.path, &bytes)?;
        let manifest = serde_json::to_vec_pretty(&receipt)
            .map_err(|_| "Could not create metadata receipt.".to_string())?;
        publish_new_file(&dir, RECEIPT_NAME, &manifest)?;
        Ok(receipt)
    })
    .await
    .map_err(|_| "Metadata cache task could not complete.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hf_catalog::{Access, Format};

    fn detail(flagged: bool, gated: &str, license: Option<&str>) -> ModelDetail {
        let content = b"{\"_class_name\":\"LTXPipeline\"}";
        ModelDetail {
            id: "example/video-model".into(),
            revision: "a".repeat(40),
            capability: Capability::TextToVideo,
            pipeline_tag: Some("text-to-video".into()),
            license: license.map(str::to_owned),
            license_name: None,
            license_link: None,
            base_models: None,
            gated: gated.into(),
            access: Access::Public,
            audience: if flagged { "not-for-all-audiences" } else { "unverified" },
            format: Format::Diffusers,
            repo_files: 1,
            repo_bytes: content.len() as u64,
            cache_metadata_file: Some(RepoFile {
                path: "model_index.json".into(), size: content.len() as u64,
                blob_id: Some(git_blob_id(content)), lfs_sha256: None,
            }),
            plans: vec![], rejections: vec![],
            page_url: "https://huggingface.co/example/video-model".into(),
        }
    }

    fn req() -> CacheRequest {
        CacheRequest {
            repo: "example/video-model".into(),
            revision: "a".repeat(40),
            capability: Capability::TextToVideo,
            license_acknowledged: "unknown".into(),
            adult_acknowledged: false,
            accept: true,
        }
    }

    #[test]
    fn admission_checks_consent_license_access_and_task_independently() {
        assert!(admit(&req(), &detail(false, "no", None)).is_ok());
        let mut r = req();
        r.accept = false;
        assert!(admit(&r, &detail(false, "no", None)).is_err());
        assert!(admit(&req(), &detail(true, "no", None)).unwrap_err().contains("18+"));
        let mut r = req();
        r.adult_acknowledged = true;
        assert!(admit(&r, &detail(true, "no", None)).is_ok());
        assert!(admit(&req(), &detail(false, "auto", None)).unwrap_err().contains("Gated"));
        assert!(admit(&req(), &detail(false, "no", Some("other"))).unwrap_err().contains("license"));
        let mut r = req();
        r.revision = "b".repeat(40);
        assert!(admit(&r, &detail(false, "no", None)).is_err());
        let mut r = req();
        r.capability = Capability::TextToImage;
        assert!(admit(&r, &detail(false, "no", None)).is_err());
    }

    #[test]
    fn downloaded_config_requires_source_blob_identity_and_valid_json() {
        let source = br#"{"_class_name":"VideoPipeline"}"#;
        let mut f = RepoFile {
            path: "model_index.json".into(),
            size: source.len() as u64,
            blob_id: Some(git_blob_id(source)),
            lfs_sha256: None,
        };
        assert!(verify_download(source, &f).is_ok());
        assert!(verify_download(b"{}", &f).is_err());
        f.blob_id = Some(git_blob_id(b"not-json"));
        f.size = 8;
        assert!(verify_download(b"not-json", &f).is_err());
        f.blob_id = None;
        f.lfs_sha256 = Some(digest_sha256(source));
        f.size = source.len() as u64;
        assert!(verify_download(source, &f).is_ok());
    }

    #[test]
    fn cache_paths_have_no_repository_path_segments_and_do_not_overwrite() {
        let root = std::env::temp_dir().join(format!("optic-video-cache-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let folder = cache_folder(&root, "example/video-model", &"a".repeat(40)).unwrap();
        assert!(folder.starts_with(&root));
        assert!(!folder.to_string_lossy().contains("example/video-model"));
        publish_new_file(&folder, "model_index.json", br#"{"a":1}"#).unwrap();
        assert!(publish_new_file(&folder, "model_index.json", br#"{"a":2}"#).is_err());
        assert_eq!(fs::read(folder.join("model_index.json")).unwrap(), br#"{"a":1}"#);
        assert!(publish_new_file(&folder, "../../outside.json", b"{}").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
