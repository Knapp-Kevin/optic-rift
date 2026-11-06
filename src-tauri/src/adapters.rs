//! Capability-driven inference adapter registry.
//!
//! Discovery is broad; execution is qualified. An adapter decides whether it can run a model
//! from the repository's actual file listing and its `model_index.json` at a pinned revision,
//! never from names, tags, downloads or model-card prose. New adapters are added to
//! [`REGISTRY`] without changing discovery or the UI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Advertised task families used for discovery filters. Source claims, not Optic Rift support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    TextToImage,
    ImageToImage,
    Inpainting,
    Upscaling,
    Restoration,
    ImageToVideo,
    TextToVideo,
    VideoToVideo,
}

impl Capability {
    pub const ALL: [Capability; 8] = [
        Capability::TextToImage,
        Capability::ImageToImage,
        Capability::Inpainting,
        Capability::Upscaling,
        Capability::Restoration,
        Capability::ImageToVideo,
        Capability::TextToVideo,
        Capability::VideoToVideo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::TextToImage => "Text to image",
            Self::ImageToImage => "Image to image",
            Self::Inpainting => "Inpainting",
            Self::Upscaling => "Upscaling",
            Self::Restoration => "Restoration",
            Self::ImageToVideo => "Image to video",
            Self::TextToVideo => "Text to video",
            Self::VideoToVideo => "Video to video",
        }
    }

    /// Hugging Face query parameters for this capability (pipeline tag and/or tag filter).
    pub fn query(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::TextToImage => &[("pipeline_tag", "text-to-image")],
            Self::ImageToImage => &[("pipeline_tag", "image-to-image")],
            Self::Inpainting => &[("filter", "inpainting")],
            Self::Upscaling => &[
                ("pipeline_tag", "image-to-image"),
                ("filter", "super-resolution"),
            ],
            Self::Restoration => &[("pipeline_tag", "image-to-image"), ("search", "restor")],
            Self::ImageToVideo => &[("pipeline_tag", "image-to-video")],
            Self::TextToVideo => &[("pipeline_tag", "text-to-video")],
            Self::VideoToVideo => &[("pipeline_tag", "video-to-video")],
        }
    }
}

/// An audited way to execute one model family for one capability.
#[derive(Debug, Clone, Serialize)]
pub struct Adapter {
    pub id: &'static str,
    pub version: u32,
    pub label: &'static str,
    pub capability: Capability,
    /// Worker program that executes it (see `worker.rs`).
    pub worker: &'static str,
    /// Exact diffusers pipeline class required in `model_index.json`.
    pub pipeline_class: &'static str,
    /// `model_index.json` components that must be present and carry weights.
    pub weighted_components: &'static [&'static str],
    /// Components that must be present but carry only configuration/vocabulary.
    pub config_components: &'static [&'static str],
}

pub const REGISTRY: &[Adapter] = &[Adapter {
    id: "diffusers-sdxl-text-to-image",
    version: 1,
    label: "Stable Diffusion XL architecture (diffusers, safetensors)",
    capability: Capability::TextToImage,
    worker: "diffusers-v1",
    pipeline_class: "StableDiffusionXLPipeline",
    weighted_components: &["text_encoder", "text_encoder_2", "unet", "vae"],
    config_components: &["scheduler", "tokenizer", "tokenizer_2"],
}];

pub fn adapter(id: &str) -> Option<&'static Adapter> {
    REGISTRY.iter().find(|a| a.id == id)
}

pub fn adapters_for(capability: Capability) -> impl Iterator<Item = &'static Adapter> {
    REGISTRY.iter().filter(move |a| a.capability == capability)
}

/// Qualification evidence for a specific model revision on a hardware class. Only entries
/// backed by an actual executed run on that class belong here.
#[derive(Debug, Clone, Serialize)]
pub struct VerifiedReceipt {
    pub repo: &'static str,
    pub revision: &'static str,
    pub adapter: &'static str,
    pub hardware_class: &'static str,
    pub evidence: &'static str,
    /// Peak GPU memory observed during a 1024x1024 generation, if measured.
    pub measured_peak_vram_mb: Option<u32>,
}

pub const VERIFIED: &[VerifiedReceipt] = &[];

pub fn verified_receipt(
    repo: &str,
    revision: &str,
    adapter: &str,
) -> Option<&'static VerifiedReceipt> {
    VERIFIED
        .iter()
        .find(|r| r.repo == repo && r.revision == revision && r.adapter == adapter)
}

/// One file in a repository at a revision, as reported by the Hugging Face API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoFile {
    pub path: String,
    pub size: u64,
    /// SHA-256 of an LFS object, when the file is stored in LFS.
    pub lfs_sha256: Option<String>,
    /// Git blob id (SHA-1 of "blob <len>\0<bytes>") for non-LFS files.
    pub blob_id: Option<String>,
}

/// Why an adapter cannot run a model. Shown to users verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rejection {
    pub adapter: &'static str,
    pub reason: String,
}

/// Files to install and their total size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallPlan {
    pub adapter: &'static str,
    pub files: Vec<RepoFile>,
    pub total_bytes: u64,
    /// The weight variant chosen (e.g. "fp16"), if any.
    pub variant: Option<String>,
}

const FORBIDDEN_EXTENSIONS: &[&str] = &[
    ".py", ".pyc", ".bin", ".ckpt", ".pt", ".pth", ".pkl", ".pickle", ".msgpack", ".h5", ".onnx",
    ".gguf", ".sh", ".bat", ".cmd", ".ps1", ".exe", ".dll", ".so",
];
const CONFIG_FILES: &[&str] = &[
    ".json", ".txt", ".model", // tokenizer.model (sentencepiece) is data, not code
];
const ALLOWED_LIBRARIES: &[&str] = &["diffusers", "transformers"];

fn has_ext(path: &str, exts: &[&str]) -> bool {
    let lower = path.to_ascii_lowercase();
    exts.iter().any(|e| lower.ends_with(e))
}

/// Decide whether `adapter` can run this repository and, if so, exactly which files to install.
pub fn plan(
    adapter: &'static Adapter,
    files: &[RepoFile],
    model_index: Option<&serde_json::Value>,
) -> Result<InstallPlan, Rejection> {
    let reject = |reason: String| Rejection {
        adapter: adapter.id,
        reason,
    };
    let index = model_index
        .and_then(|v| v.as_object())
        .ok_or_else(|| reject("No diffusers model_index.json; single-file or other formats are not supported by this adapter.".into()))?;
    let class = index
        .get("_class_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if class != adapter.pipeline_class {
        return Err(reject(format!(
            "Pipeline is {}, but this adapter runs {} only.",
            if class.is_empty() {
                "unspecified"
            } else {
                class
            },
            adapter.pipeline_class
        )));
    }
    // Every declared component must load from an allowlisted library (never repository code).
    for (name, value) in index.iter().filter(|(k, _)| !k.starts_with('_')) {
        let Some(pair) = value.as_array() else {
            continue;
        };
        if pair.len() != 2 || pair.iter().all(|v| v.is_null()) {
            continue;
        }
        let library = pair[0].as_str().unwrap_or("");
        if !ALLOWED_LIBRARIES.contains(&library) {
            return Err(reject(format!(
                "Component {name} loads from '{library}', which could execute repository code."
            )));
        }
    }
    let mut chosen = Vec::new();
    let mut variant_used: Option<String> = None;
    let component_files = |component: &str| -> Vec<&RepoFile> {
        files
            .iter()
            .filter(|f| f.path.starts_with(&format!("{component}/")))
            .collect()
    };
    for &component in adapter
        .config_components
        .iter()
        .chain(adapter.weighted_components)
    {
        if !index.contains_key(component) {
            return Err(reject(format!(
                "model_index.json does not declare {component}."
            )));
        }
        let in_dir = component_files(component);
        if in_dir.is_empty() {
            return Err(reject(format!("The repository has no {component}/ files.")));
        }
        for f in &in_dir {
            if f.path[component.len() + 1..].contains('/') {
                continue; // nested folders are not used by these components
            }
            if has_ext(&f.path, FORBIDDEN_EXTENSIONS)
                && !adapter.weighted_components.contains(&component)
            {
                return Err(reject(format!(
                    "{} is executable or pickled content.",
                    f.path
                )));
            }
        }
        if adapter.weighted_components.contains(&component) {
            let safetensors: Vec<&RepoFile> = in_dir
                .iter()
                .copied()
                .filter(|f| {
                    f.path.ends_with(".safetensors") && !f.path[component.len() + 1..].contains('/')
                })
                .collect();
            if safetensors.is_empty() {
                return Err(reject(format!(
                    "{component} has no safetensors weights; pickled checkpoints are refused."
                )));
            }
            if safetensors
                .iter()
                .any(|f| f.path.contains(".safetensors.index.json"))
            {
                return Err(reject(format!(
                    "{component} uses sharded weights, not yet supported."
                )));
            }
            let fp16: Vec<&RepoFile> = safetensors
                .iter()
                .copied()
                .filter(|f| f.path.ends_with(".fp16.safetensors"))
                .collect();
            let pick: Vec<&RepoFile> = if !fp16.is_empty() {
                variant_used = Some("fp16".into());
                fp16
            } else {
                safetensors
                    .iter()
                    .copied()
                    .filter(|f| f.path.matches('.').count() == 1)
                    .collect()
            };
            if pick.len() != 1 {
                return Err(reject(format!(
                    "{component} has an ambiguous set of weight files."
                )));
            }
            chosen.push(pick[0].clone());
            let config = format!("{component}/config.json");
            match in_dir.iter().find(|f| f.path == config) {
                Some(f) => chosen.push((*f).clone()),
                None => return Err(reject(format!("{config} is missing."))),
            }
        } else {
            for f in in_dir {
                let name = &f.path[component.len() + 1..];
                if !name.contains('/') && has_ext(name, CONFIG_FILES) {
                    chosen.push(f.clone());
                }
            }
        }
    }
    match files.iter().find(|f| f.path == "model_index.json") {
        Some(f) => chosen.push(f.clone()),
        None => {
            return Err(reject(
                "model_index.json is missing from the file listing.".into(),
            ))
        }
    }
    // If one weighted component used fp16, all must, so the worker loads one consistent variant.
    if variant_used.is_some()
        && chosen
            .iter()
            .filter(|f| f.path.ends_with(".safetensors"))
            .any(|f| !f.path.ends_with(".fp16.safetensors"))
    {
        return Err(reject(
            "Weights mix fp16 and full-precision variants.".into(),
        ));
    }
    for f in &chosen {
        if f.path.ends_with(".safetensors") && f.lfs_sha256.is_none() {
            return Err(reject(format!(
                "{} has no published SHA-256; integrity cannot be verified.",
                f.path
            )));
        }
        if f.lfs_sha256.is_none() && f.blob_id.is_none() {
            return Err(reject(format!("{} has no published hash.", f.path)));
        }
    }
    chosen.sort_by(|a, b| a.path.cmp(&b.path));
    let unique: BTreeSet<_> = chosen.iter().map(|f| f.path.clone()).collect();
    if unique.len() != chosen.len() {
        return Err(reject("Duplicate files in plan.".into()));
    }
    let total_bytes = chosen.iter().map(|f| f.size).sum();
    Ok(InstallPlan {
        adapter: adapter.id,
        files: chosen,
        total_bytes,
        variant: variant_used,
    })
}

/// Evaluate every adapter for a capability; returns plans and per-adapter rejections.
pub fn evaluate(
    capability: Capability,
    files: &[RepoFile],
    model_index: Option<&serde_json::Value>,
) -> (Vec<InstallPlan>, Vec<Rejection>) {
    let mut plans = Vec::new();
    let mut rejections = Vec::new();
    for adapter in adapters_for(capability) {
        match plan(adapter, files, model_index) {
            Ok(p) => plans.push(p),
            Err(r) => rejections.push(r),
        }
    }
    (plans, rejections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn f(path: &str, size: u64, lfs: bool) -> RepoFile {
        RepoFile {
            path: path.into(),
            size,
            lfs_sha256: lfs.then(|| "a".repeat(64)),
            blob_id: (!lfs).then(|| "b".repeat(40)),
        }
    }

    /// Mirrors segmind/SSD-1B at 60987f37: root single-file checkpoints plus diffusers folders.
    fn ssd1b() -> (Vec<RepoFile>, serde_json::Value) {
        let files = vec![
            f(".gitattributes", 1519, false),
            f("README.md", 10772, false),
            f("SSD-1B.safetensors", 4_465_671_506, true),
            f("model_index.json", 577, false),
            f("scheduler/scheduler_config.json", 474, false),
            f("text_encoder/config.json", 560, false),
            f("text_encoder/model.fp16.safetensors", 246_144_864, true),
            f("text_encoder/model.safetensors", 492_265_880, true),
            f("text_encoder_2/config.json", 570, false),
            f("text_encoder_2/model.fp16.safetensors", 1_389_382_880, true),
            f("text_encoder_2/model.safetensors", 2_778_702_976, true),
            f("tokenizer/merges.txt", 524_619, false),
            f("tokenizer/special_tokens_map.json", 472, false),
            f("tokenizer/tokenizer_config.json", 737, false),
            f("tokenizer/vocab.json", 1_059_962, false),
            f("tokenizer_2/merges.txt", 524_619, false),
            f("tokenizer_2/special_tokens_map.json", 460, false),
            f("tokenizer_2/tokenizer_config.json", 725, false),
            f("tokenizer_2/vocab.json", 1_059_962, false),
            f("unet/config.json", 1768, false),
            f(
                "unet/diffusion_pytorch_model.fp16.safetensors",
                2_662_790_608,
                true,
            ),
            f(
                "unet/diffusion_pytorch_model.safetensors",
                5_325_459_760,
                true,
            ),
            f("vae/config.json", 602, false),
            f(
                "vae/diffusion_pytorch_model.fp16.safetensors",
                167_335_342,
                true,
            ),
            f("vae/diffusion_pytorch_model.safetensors", 334_643_268, true),
        ];
        let index = json!({
            "_class_name": "StableDiffusionXLPipeline", "_diffusers_version": "0.19.0",
            "scheduler": ["diffusers", "EulerDiscreteScheduler"],
            "text_encoder": ["transformers", "CLIPTextModel"],
            "text_encoder_2": ["transformers", "CLIPTextModelWithProjection"],
            "tokenizer": ["transformers", "CLIPTokenizer"], "tokenizer_2": ["transformers", "CLIPTokenizer"],
            "unet": ["diffusers", "UNet2DConditionModel"], "vae": ["diffusers", "AutoencoderKL"]
        });
        (files, index)
    }

    #[test]
    fn sdxl_repository_plans_only_fp16_component_files() {
        let (files, index) = ssd1b();
        let plan = plan(&REGISTRY[0], &files, Some(&index)).unwrap();
        let paths: Vec<_> = plan.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(plan.variant.as_deref(), Some("fp16"));
        assert!(
            !paths.contains(&"SSD-1B.safetensors"),
            "root single-file checkpoint skipped"
        );
        assert!(!paths.contains(&"README.md") && !paths.contains(&".gitattributes"));
        assert!(paths
            .iter()
            .filter(|p| p.ends_with(".safetensors"))
            .all(|p| p.ends_with(".fp16.safetensors")));
        assert_eq!(paths.len(), 18);
        assert_eq!(plan.total_bytes, 4_468_829_801);
    }

    #[test]
    fn wrong_pipeline_or_missing_index_is_unsupported() {
        let (files, mut index) = ssd1b();
        assert!(plan(&REGISTRY[0], &files, None)
            .unwrap_err()
            .reason
            .contains("model_index.json"));
        index["_class_name"] = json!("FluxPipeline");
        assert!(plan(&REGISTRY[0], &files, Some(&index))
            .unwrap_err()
            .reason
            .contains("FluxPipeline"));
    }

    #[test]
    fn repository_code_components_are_refused() {
        let (files, mut index) = ssd1b();
        index["unet"] = json!(["my_custom_unet", "FancyUNet"]);
        assert!(plan(&REGISTRY[0], &files, Some(&index))
            .unwrap_err()
            .reason
            .contains("repository code"));
    }

    #[test]
    fn pickle_only_weights_are_refused() {
        let (mut files, index) = ssd1b();
        files.retain(|f| !f.path.starts_with("unet/diffusion_pytorch_model"));
        files.push(f("unet/diffusion_pytorch_model.bin", 5_000_000_000, true));
        assert!(plan(&REGISTRY[0], &files, Some(&index))
            .unwrap_err()
            .reason
            .contains("safetensors"));
    }

    #[test]
    fn python_in_config_folder_is_refused() {
        let (mut files, index) = ssd1b();
        files.push(f("scheduler/custom_scheduler.py", 100, false));
        assert!(plan(&REGISTRY[0], &files, Some(&index))
            .unwrap_err()
            .reason
            .contains("executable"));
    }

    #[test]
    fn weights_without_published_hash_are_refused() {
        let (mut files, index) = ssd1b();
        for file in files
            .iter_mut()
            .filter(|f| f.path == "vae/diffusion_pytorch_model.fp16.safetensors")
        {
            file.lfs_sha256 = None;
        }
        assert!(plan(&REGISTRY[0], &files, Some(&index))
            .unwrap_err()
            .reason
            .contains("SHA-256"));
    }

    #[test]
    fn full_precision_only_repository_uses_plain_weights() {
        let (mut files, index) = ssd1b();
        files.retain(|f| !f.path.contains(".fp16."));
        let plan = plan(&REGISTRY[0], &files, Some(&index)).unwrap();
        assert_eq!(plan.variant, None);
        assert!(plan
            .files
            .iter()
            .any(|f| f.path == "unet/diffusion_pytorch_model.safetensors"));
    }

    #[test]
    fn capabilities_without_adapters_have_no_plans_but_remain_discoverable() {
        let (files, index) = ssd1b();
        let (plans, rejections) = evaluate(Capability::TextToVideo, &files, Some(&index));
        assert!(plans.is_empty() && rejections.is_empty());
        assert!(!Capability::TextToVideo.query().is_empty());
        assert_eq!(Capability::ALL.len(), 8);
    }
}
