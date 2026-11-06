//! Public Hugging Face model discovery and inspection. Search is anonymous; a stored read token is
//! used only when inspecting or downloading a repository that refuses anonymous access. Model-card
//! prose is never fetched or rendered; only structured metadata is used.

use crate::adapters::{self, Capability, InstallPlan, Rejection, RepoFile, VerifiedReceipt};
use crate::hf_access::USER_SLOT;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const HUB: &str = "https://huggingface.co";
const MAX_INDEX_BYTES: usize = 64 * 1024;

pub fn valid_repo_id(id: &str) -> bool {
    let mut parts = id.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |s: &str| {
        (1..=96).contains(&s.len())
            && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
            && !s.contains("..")
    };
    ok(owner) && ok(name)
}

pub fn valid_revision_sha(rev: &str) -> bool {
    rev.len() == 40
        && rev
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// HTTPS client restricted to Hugging Face hosts, including redirects to its storage CDN.
pub fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    let policy = reqwest::redirect::Policy::custom(|attempt| {
        let url = attempt.url();
        let host = url.host_str().unwrap_or("");
        let allowed = url.scheme() == "https"
            && (host == "huggingface.co"
                || host.ends_with(".huggingface.co")
                || host.ends_with(".hf.co"));
        if attempt.previous().len() >= 5 {
            attempt.error("too many redirects")
        } else if allowed {
            attempt.follow()
        } else {
            attempt.stop()
        }
    });
    reqwest::Client::builder()
        .https_only(!cfg!(test))
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .redirect(policy)
        .user_agent(concat!("OpticRift/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| "Could not prepare a secure connection.".to_string())
}

fn net_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Hugging Face did not respond in time.".into()
    } else {
        "Could not reach Hugging Face. Check your connection.".into()
    }
}

/// What the file listing alone says about the packaging format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Diffusers,
    SingleFile,
    Gguf,
    Other,
    Unknown,
}

fn format_of(files: &[String]) -> Format {
    if files.is_empty() {
        Format::Unknown
    } else if files.iter().any(|f| f == "model_index.json") {
        Format::Diffusers
    } else if files.iter().any(|f| f.ends_with(".gguf")) {
        Format::Gguf
    } else if files
        .iter()
        .any(|f| !f.contains('/') && f.ends_with(".safetensors"))
    {
        Format::SingleFile
    } else {
        Format::Other
    }
}

/// Discovery-stage compatibility, stated only as far as metadata can establish it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "kebab-case")]
pub enum Compatibility {
    /// No Optic Rift adapter exists for this capability yet.
    NoAdapter,
    /// The packaging format cannot be run by any adapter for this capability.
    UnsupportedFormat(String),
    /// Format is plausible; inspecting the pinned revision decides.
    InspectToConfirm,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelSummary {
    pub id: String,
    pub sha: Option<String>,
    pub pipeline_tag: Option<String>,
    pub library: Option<String>,
    pub downloads: u64,
    pub likes: u64,
    pub gated: String,
    pub license: Option<String>,
    pub last_modified: Option<String>,
    /// "not-for-all-audiences" when the source flags it; otherwise unverified.
    pub audience: &'static str,
    pub format: Format,
    pub compatibility: Compatibility,
}

#[derive(Debug, Serialize)]
pub struct SearchResult {
    pub capability: Capability,
    pub models: Vec<ModelSummary>,
    /// Source-flagged adult/mature listings withheld because the user has not opted in.
    pub hidden_not_for_all_audiences: usize,
}

#[derive(Deserialize)]
struct RawSibling {
    rfilename: String,
    size: Option<u64>,
    #[serde(rename = "blobId")]
    blob_id: Option<String>,
    lfs: Option<RawLfs>,
}

#[derive(Deserialize)]
struct RawLfs {
    sha256: String,
    size: u64,
}

#[derive(Deserialize)]
struct RawModel {
    id: String,
    sha: Option<String>,
    pipeline_tag: Option<String>,
    library_name: Option<String>,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    likes: u64,
    #[serde(default)]
    gated: serde_json::Value,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(rename = "cardData", default)]
    card: Option<serde_json::Value>,
    #[serde(default)]
    siblings: Vec<RawSibling>,
}

fn gated_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Bool(false) | serde_json::Value::Null => "no".into(),
        serde_json::Value::Bool(true) => "yes".into(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn card_str(card: &Option<serde_json::Value>, key: &str) -> Option<String> {
    card.as_ref()?.get(key).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Array(a) => Some(
            a.iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ),
        _ => None,
    })
}

fn license_of(raw: &RawModel) -> Option<String> {
    card_str(&raw.card, "license").or_else(|| {
        raw.tags
            .iter()
            .find_map(|t| t.strip_prefix("license:").map(str::to_owned))
    })
}

fn flagged_adult(tags: &[String]) -> bool {
    tags.iter().any(|t| t == "not-for-all-audiences")
}

fn summarize(raw: RawModel, capability: Capability) -> ModelSummary {
    let names: Vec<String> = raw.siblings.iter().map(|s| s.rfilename.clone()).collect();
    let format = format_of(&names);
    let has_adapter = adapters::adapters_for(capability).next().is_some();
    let compatibility = if !has_adapter {
        Compatibility::NoAdapter
    } else {
        match format {
            Format::Diffusers => Compatibility::InspectToConfirm,
            Format::Unknown => Compatibility::InspectToConfirm,
            Format::SingleFile => Compatibility::UnsupportedFormat("Single-file checkpoint; Optic Rift's adapters need the diffusers folder layout.".into()),
            Format::Gguf => Compatibility::UnsupportedFormat("GGUF quantized weights are not supported by an Optic Rift adapter yet.".into()),
            Format::Other => Compatibility::UnsupportedFormat("No recognised diffusers pipeline files (this may be a LoRA, adapter or other format).".into()),
        }
    };
    ModelSummary {
        license: license_of(&raw),
        audience: if flagged_adult(&raw.tags) {
            "not-for-all-audiences"
        } else {
            "unverified"
        },
        gated: gated_text(&raw.gated),
        id: raw.id,
        sha: raw.sha,
        pipeline_tag: raw.pipeline_tag,
        library: raw.library_name,
        downloads: raw.downloads,
        likes: raw.likes,
        last_modified: raw.last_modified,
        format,
        compatibility,
    }
}

pub fn search_url(
    query: &str,
    capability: Capability,
    limit: usize,
) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(&format!("{HUB}/api/models")).map_err(|e| e.to_string())?;
    {
        let mut q = url.query_pairs_mut();
        for (k, v) in capability.query() {
            q.append_pair(k, v);
        }
        let query = query.trim();
        if !query.is_empty() {
            q.append_pair("search", query);
        }
        q.append_pair("sort", "downloads")
            .append_pair("direction", "-1")
            .append_pair("limit", &limit.to_string());
        for field in [
            "cardData",
            "gated",
            "downloads",
            "likes",
            "tags",
            "library_name",
            "pipeline_tag",
            "sha",
            "lastModified",
            "siblings",
        ] {
            q.append_pair("expand[]", field);
        }
    }
    Ok(url)
}

/// Anonymous catalog search. Never sends the stored token.
pub async fn search(
    query: &str,
    capability: Capability,
    include_adult: bool,
    limit: usize,
) -> Result<SearchResult, String> {
    if query.chars().count() > 120 || query.chars().any(char::is_control) {
        return Err("Search text must be at most 120 characters.".into());
    }
    let limit = limit.clamp(1, 60);
    let response = client(Duration::from_secs(20))?
        .get(search_url(query, capability, limit)?)
        .send()
        .await
        .map_err(net_error)?;
    if !response.status().is_success() {
        return Err(format!(
            "Hugging Face search failed (HTTP {}).",
            response.status().as_u16()
        ));
    }
    let raw: Vec<RawModel> = response
        .json()
        .await
        .map_err(|_| "Hugging Face returned an unexpected search response.".to_string())?;
    let mut hidden = 0;
    let models = raw
        .into_iter()
        .filter(|m| {
            let keep = include_adult || !flagged_adult(&m.tags);
            hidden += usize::from(!keep);
            keep
        })
        .map(|m| summarize(m, capability))
        .collect();
    Ok(SearchResult {
        capability,
        models,
        hidden_not_for_all_audiences: hidden,
    })
}

/// Access situation for a repository, as observed from actual HTTP responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Access {
    Public,
    /// Gated, and the stored token can read it (terms were accepted on Hugging Face).
    GatedAccessible,
    /// Gated and not readable with the current credentials.
    GatedNeedsTerms,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanView {
    pub plan: InstallPlan,
    pub adapter_label: &'static str,
    pub verified: Option<&'static VerifiedReceipt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelDetail {
    pub id: String,
    pub revision: String,
    pub pipeline_tag: Option<String>,
    pub capability: Capability,
    pub license: Option<String>,
    pub license_name: Option<String>,
    pub license_link: Option<String>,
    pub base_models: Option<String>,
    pub gated: String,
    pub access: Access,
    pub audience: &'static str,
    pub format: Format,
    pub repo_files: usize,
    pub repo_bytes: u64,
    pub plans: Vec<PlanView>,
    pub rejections: Vec<Rejection>,
    pub page_url: String,
}

async fn get_with_optional_token(
    url: &str,
    timeout: Duration,
) -> Result<(reqwest::Response, bool), String> {
    let http = client(timeout)?;
    let anonymous = http.get(url).send().await.map_err(net_error)?;
    if !matches!(anonymous.status().as_u16(), 401 | 403) {
        return Ok((anonymous, false));
    }
    match USER_SLOT.token()? {
        Some(token) => Ok((
            http.get(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(net_error)?,
            true,
        )),
        None => Ok((anonymous, false)),
    }
}

fn repo_files(siblings: &[RawSibling]) -> Vec<RepoFile> {
    siblings
        .iter()
        .map(|s| RepoFile {
            path: s.rfilename.clone(),
            size: s.lfs.as_ref().map(|l| l.size).or(s.size).unwrap_or(0),
            lfs_sha256: s.lfs.as_ref().map(|l| l.sha256.to_ascii_lowercase()),
            blob_id: s.blob_id.clone(),
        })
        .collect()
}

/// Inspect one repository at `revision` ("main" or a commit SHA) and evaluate every adapter.
pub async fn inspect(
    id: &str,
    revision: &str,
    capability: Capability,
) -> Result<ModelDetail, String> {
    if !valid_repo_id(id) {
        return Err("Invalid model identifier.".into());
    }
    if revision != "main" && !valid_revision_sha(revision) {
        return Err("Invalid revision.".into());
    }
    let url = format!("{HUB}/api/models/{id}/revision/{revision}?blobs=true");
    let (response, _) = get_with_optional_token(&url, Duration::from_secs(20)).await?;
    match response.status().as_u16() {
        200 => {}
        401 | 403 | 404 => {
            return Err("Model not found, private, or not accessible with your credentials.".into())
        }
        code => return Err(format!("Hugging Face returned HTTP {code} for this model.")),
    }
    let raw: RawModel = response
        .json()
        .await
        .map_err(|_| "Unexpected model metadata.".to_string())?;
    let sha = raw
        .sha
        .clone()
        .filter(|s| valid_revision_sha(s))
        .ok_or("Hugging Face did not report a commit SHA.")?;
    let files = repo_files(&raw.siblings);
    let names: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    let gated = gated_text(&raw.gated);
    let mut access = if gated == "no" {
        Access::Public
    } else {
        Access::GatedNeedsTerms
    };
    let mut model_index = None;
    if names.iter().any(|n| n == "model_index.json") {
        let (resp, used_token) = get_with_optional_token(
            &format!("{HUB}/{id}/resolve/{sha}/model_index.json"),
            Duration::from_secs(20),
        )
        .await?;
        if resp.status().is_success() {
            if gated != "no" {
                access = if used_token {
                    Access::GatedAccessible
                } else {
                    Access::Public
                };
            }
            let bytes = resp.bytes().await.map_err(net_error)?;
            if bytes.len() <= MAX_INDEX_BYTES {
                model_index = serde_json::from_slice::<serde_json::Value>(&bytes).ok();
            }
        }
    }
    let index_listed = names.iter().any(|n| n == "model_index.json");
    let (plans, mut rejections) = if index_listed
        && model_index.is_none()
        && access == Access::GatedNeedsTerms
    {
        // The files exist but cannot be read: compatibility is unknown, not unsupported.
        let has_adapter = adapters::adapters_for(capability).next().is_some();
        let reason = "Gated model: its files can't be read until you accept its terms on Hugging Face and connect a read token, so compatibility can't be checked yet.";
        (
            Vec::new(),
            if has_adapter {
                vec![Rejection {
                    adapter: "access",
                    reason: reason.into(),
                }]
            } else {
                Vec::new()
            },
        )
    } else {
        adapters::evaluate(capability, &files, model_index.as_ref())
    };
    let plans = plans
        .into_iter()
        .map(|plan| {
            let adapter = adapters::adapter(plan.adapter).expect("registered");
            PlanView {
                verified: adapters::verified_receipt(id, &sha, adapter.id),
                adapter_label: adapter.label,
                plan,
            }
        })
        .collect();
    Ok(ModelDetail {
        license: license_of(&raw),
        license_name: card_str(&raw.card, "license_name"),
        license_link: card_str(&raw.card, "license_link"),
        base_models: card_str(&raw.card, "base_model"),
        audience: if flagged_adult(&raw.tags) {
            "not-for-all-audiences"
        } else {
            "unverified"
        },
        format: format_of(&names),
        repo_files: files.len(),
        repo_bytes: files.iter().map(|f| f.size).sum(),
        page_url: format!("{HUB}/{id}"),
        id: raw.id,
        revision: sha,
        pipeline_tag: raw.pipeline_tag,
        capability,
        gated,
        access,
        plans,
        rejections,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_ids_and_revisions_are_strict() {
        assert!(valid_repo_id("segmind/SSD-1B"));
        assert!(valid_repo_id("stable-diffusion-v1-5/stable-diffusion-v1-5"));
        for bad in [
            "segmind",
            "a/b/c",
            "../x",
            "a/..",
            "a/b?c",
            "a/b#x",
            "a /b",
            "/b",
            "a/",
            "a/.hidden",
        ] {
            assert!(!valid_repo_id(bad), "{bad}");
        }
        assert!(valid_revision_sha(
            "60987f37e94cd59c36b1cba832b9f97b57395a10"
        ));
        assert!(!valid_revision_sha("main"));
        assert!(!valid_revision_sha(
            "60987F37E94CD59C36B1CBA832B9F97B57395A10"
        ));
    }

    #[test]
    fn search_url_carries_capability_and_never_credentials() {
        let url = search_url(" lighthouse ", Capability::Upscaling, 30).unwrap();
        let s = url.as_str();
        assert!(s.starts_with("https://huggingface.co/api/models?"));
        assert!(s.contains("pipeline_tag=image-to-image") && s.contains("filter=super-resolution"));
        assert!(s.contains("search=lighthouse") && s.contains("limit=30"));
        assert!(!s.to_lowercase().contains("token"));
    }

    #[test]
    fn format_comes_from_files_not_names() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            format_of(&v(&["model_index.json", "unet/config.json"])),
            Format::Diffusers
        );
        assert_eq!(
            format_of(&v(&["sdxl-diffusers-model.safetensors"])),
            Format::SingleFile
        );
        assert_eq!(format_of(&v(&["x-Q4_K_M.gguf"])), Format::Gguf);
        assert_eq!(
            format_of(&v(&["adapter_model.safetensors.json", "README.md"])),
            Format::Other
        );
        assert_eq!(format_of(&[]), Format::Unknown);
    }

    #[test]
    fn summaries_separate_audience_license_and_compatibility() {
        let raw: RawModel = serde_json::from_value(serde_json::json!({
            "id": "someone/model", "sha": "a".repeat(40), "pipeline_tag": "text-to-image",
            "tags": ["not-for-all-audiences", "license:other"], "gated": "auto",
            "siblings": [{"rfilename": "model.safetensors"}]
        }))
        .unwrap();
        let s = summarize(raw, Capability::TextToImage);
        assert_eq!(s.audience, "not-for-all-audiences");
        assert_eq!(s.license.as_deref(), Some("other"));
        assert_eq!(s.gated, "auto");
        assert!(matches!(
            s.compatibility,
            Compatibility::UnsupportedFormat(_)
        ));
        let raw: RawModel = serde_json::from_value(
            serde_json::json!({"id": "a/b", "siblings": [{"rfilename": "model_index.json"}]}),
        )
        .unwrap();
        assert_eq!(
            summarize(raw, Capability::ImageToVideo).compatibility,
            Compatibility::NoAdapter
        );
    }

    // Live network checks against huggingface.co, run explicitly with `cargo test -- --ignored`.
    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(f)
    }

    #[test]
    #[ignore]
    fn live_search_every_capability_anonymously() {
        for capability in Capability::ALL {
            let result = block(search("", capability, false, 10)).unwrap();
            assert!(!result.models.is_empty(), "{capability:?} returned nothing");
            assert!(result
                .models
                .iter()
                .all(|m| m.audience != "not-for-all-audiences"));
            println!(
                "{capability:?}: {} models, {} hidden as not-for-all-audiences, first {}",
                result.models.len(),
                result.hidden_not_for_all_audiences,
                result.models[0].id
            );
        }
        let adult = block(search("", Capability::TextToImage, true, 60)).unwrap();
        println!(
            "text-to-image with opt-in: {} flagged of {}",
            adult
                .models
                .iter()
                .filter(|m| m.audience == "not-for-all-audiences")
                .count(),
            adult.models.len()
        );
    }

    #[test]
    #[ignore]
    fn live_inspect_ssd1b_pinned_revision() {
        let d = block(inspect(
            "segmind/SSD-1B",
            "60987f37e94cd59c36b1cba832b9f97b57395a10",
            Capability::TextToImage,
        ))
        .unwrap();
        assert_eq!(d.access, Access::Public);
        assert_eq!(d.license.as_deref(), Some("apache-2.0"));
        assert_eq!(d.plans.len(), 1, "{:?}", d.rejections);
        assert_eq!(d.plans[0].plan.files.len(), 18);
        assert_eq!(d.plans[0].plan.total_bytes, 4_468_829_801);
        assert_eq!(d.plans[0].plan.variant.as_deref(), Some("fp16"));
    }

    #[test]
    #[ignore]
    fn live_inspect_unsupported_and_gated_models_explain_why() {
        let flux = block(inspect(
            "black-forest-labs/FLUX.1-schnell",
            "main",
            Capability::TextToImage,
        ))
        .unwrap();
        assert!(flux.plans.is_empty());
        println!(
            "FLUX.1-schnell gated={} access={:?} rejections={:?}",
            flux.gated, flux.access, flux.rejections
        );
        let dev = block(inspect(
            "black-forest-labs/FLUX.1-dev",
            "main",
            Capability::TextToImage,
        ))
        .unwrap();
        assert_ne!(dev.gated, "no");
        println!(
            "FLUX.1-dev gated={} access={:?} rejections={:?}",
            dev.gated, dev.access, dev.rejections
        );
        let video = block(inspect(
            "Lightricks/LTX-Video",
            "main",
            Capability::ImageToVideo,
        ))
        .unwrap();
        assert!(
            video.plans.is_empty() && video.rejections.is_empty(),
            "no image-to-video adapter exists yet"
        );
        assert!(block(inspect(
            "segmind/does-not-exist-optic-rift",
            "main",
            Capability::TextToImage
        ))
        .is_err());
    }
}
