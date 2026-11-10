//! Privacy-bounded public disclosure receipts for locally generated images.
//!
//! These JSON receipts are UNSIGNED and NOT C2PA credentials. They are an
//! interim, explicitly unverified disclosure layer, never a substitute for
//! signed and content-bound credentials or encrypted forensic records.
//! Keep private prompts, paths and runtime identifiers in local-only history.
use crate::generation::Provenance;
use serde_json::{json, Value};
use std::collections::HashSet;

const MAX_STAGES: usize = 16;
const MAX_PUBLIC_RECORD_BYTES: usize = 32 * 1024;

fn canonical_job_id(raw: &str) -> bool {
    uuid::Uuid::parse_str(raw).is_ok_and(|id| id.hyphenated().to_string() == raw)
}

/// This is a strict, lossy disclosure whitelist, not a validation of external
/// model provenance. A local path, tokenized URL or suspicious value is omitted.
fn public_model_field(model: &Value, name: &str, max_len: usize) -> Option<String> {
    let value = model.get(name)?.as_str()?;
    if value.is_empty()
        || value.len() > max_len
        || value.starts_with('/')
        || value.contains("..")
        || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b))
    {
        return None;
    }
    Some(value.to_owned())
}

/// Build a deliberately untrusted, sanitized local generation disclosure.
///
/// `load_parent` MUST use the native verified-output path to check each
/// ancestor image against its local source hash and metadata before returning
/// its record. An unresolvable or conflicting ancestor makes the lineage
/// incomplete; no parent fields are invented and no prompt is serialized.
pub(crate) fn public_generation_receipt<F>(
    current: &Provenance,
    mut load_parent: F,
) -> Result<Vec<u8>, String>
where
    F: FnMut(&str) -> Result<Provenance, String>,
{
    if !canonical_job_id(&current.job_id) {
        return Err("Cannot disclose a generation with an invalid job ID.".into());
    }

    let original_output = &current.output;
    let output_sha = original_output["sha256"].as_str()
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("Cannot disclose output without a valid digest.")?;
    let width = original_output["width"].as_u64().ok_or("Missing output width.")?;
    let height = original_output["height"].as_u64().ok_or("Missing output height.")?;
    let bytes = original_output["bytes"].as_u64().ok_or("Missing output byte count.")?;

    let mut stages = Vec::new();
    let mut visited = HashSet::new();
    let mut entry = current.clone();
    let mut complete = true;
    loop {
        if stages.len() >= MAX_STAGES || !canonical_job_id(&entry.job_id)
            || !visited.insert(entry.job_id.clone())
        {
            complete = false;
            break;
        }
        let sha = entry.output["sha256"].as_str().unwrap_or_default();
        if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            complete = false;
            break;
        }
        stages.push(json!({
            "job_id": entry.job_id,
            "output_sha256": sha,
            "model_repo": public_model_field(&entry.model, "repo", 160),
            "model_revision": public_model_field(&entry.model, "revision", 120),
            "adapter": public_model_field(&entry.model, "adapter", 120),
            "adapter_version": public_model_field(&entry.model, "adapter_version", 80)
        }));
        let Some(source) = entry.source.as_ref() else { break };
        let Some(parent_id) = source.get("job_id").and_then(Value::as_str)
            .filter(|id| canonical_job_id(id))
            .map(str::to_owned)
        else {
            complete = false;
            break;
        };
        let Some(parent_digest) = source.get("sha256").and_then(Value::as_str)
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(str::to_owned)
        else {
            complete = false;
            break;
        };
        if visited.contains(&parent_id) {
            complete = false;
            break;
        }
        match load_parent(&parent_id) {
            Ok(parent)
                if parent.job_id == parent_id
                    && parent.output["sha256"].as_str() == Some(parent_digest.as_str()) =>
            {
                entry = parent;
            }
            _ => {
                complete = false;
                break;
            }
        }
    }
    // Display/serialize from earliest validated local stage to final output.
    stages.reverse();

    let record = json!({
        "record_version": 2,
        "kind": "optic-rift-ai-generation-disclosure",
        "ai_involvement": "ai_generated",
        "assertion_origin": "optic_rift_local_unsigned_record",
        "credential_status": "unsigned_unverified_not_c2pa",
        "lineage_status": if complete { "complete_within_available_local_history" } else { "incomplete" },
        "stage_order": "earliest_to_latest",
        "model_stages": stages,
        "output": { "sha256": output_sha, "width": width, "height": height, "bytes": bytes },
        "notice": "This redacted JSON is an unsigned local disclosure, not C2PA, an encrypted trace or proof of authorship. Missing stages are unknown."
    });
    let result = serde_json::to_vec_pretty(&record)
        .map_err(|_| "Could not encode a safe public generation disclosure.")?;
    if result.len() > MAX_PUBLIC_RECORD_BYTES {
        return Err("The safe generation disclosure exceeds its export size limit.".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, digest: &str, repo: &str) -> Provenance {
        serde_json::from_value(json!({
            "record_version": 1, "job_id": id, "created_unix_ms": 0,
            "prompt": "private prompt phrase", "negative_prompt": "private negative phrase",
            "parameters": {"seed": 31337, "secret": "do not publish"},
            "model": {"repo": repo, "revision": "abc123", "adapter": "local",
                "adapter_version": "1.0", "secret": "private model detail"},
            "runtime": {"device": "private host path C:\\Users\\Private"},
            "metrics": {}, "project_id": null,
            "output": {"sha256": digest, "width": 512, "height": 512, "bytes": 1024},
            "lineage_note": "private lineage"
        })).unwrap()
    }

    #[test]
    fn redacts_private_fields_and_discloses_multiple_verified_local_stages() {
        let parent_id = uuid::Uuid::new_v4().to_string();
        let child_id = uuid::Uuid::new_v4().to_string();
        let parent = record(&parent_id, &"a".repeat(64), "public/ModelA");
        let mut child = record(&child_id, &"b".repeat(64), "public/ModelB");
        child.source = Some(json!({
            "job_id": parent_id, "sha256": "a".repeat(64),
            "path": "C:\\Users\\Private\\secret.png"
        }));
        let exported = public_generation_receipt(&child, |id| {
            if id == parent.job_id { Ok(parent.clone()) }
            else { Err("Unknown ancestor".into()) }
        }).unwrap();
        let parsed: Value = serde_json::from_slice(&exported).unwrap();
        assert_eq!(parsed["model_stages"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["model_stages"][0]["model_repo"], "public/ModelA");
        assert_eq!(parsed["model_stages"][1]["model_repo"], "public/ModelB");
        assert_eq!(parsed["lineage_status"], "complete_within_available_local_history");
        assert_eq!(parsed["credential_status"], "unsigned_unverified_not_c2pa");
        let plain = String::from_utf8(exported).unwrap();
        for private in ["private prompt phrase", "private negative phrase",
            "Private", "do not publish", "31337", "private lineage", "secret.png"]
        {
            assert!(!plain.contains(private), "Leaked private field: {private}");
        }
    }

    #[test]
    fn missing_conflicting_or_cyclic_ancestor_is_not_claimed_verified() {
        let id = uuid::Uuid::new_v4().to_string();
        let mut record = record(&id, &"f".repeat(64), "public/Model");
        record.source = Some(json!({"job_id": uuid::Uuid::new_v4().to_string(),
            "sha256": "a".repeat(64)}));
        let raw = public_generation_receipt(&record, |_| Err("missing".into())).unwrap();
        let parsed: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(parsed["lineage_status"], "incomplete");
        assert_eq!(parsed["model_stages"].as_array().unwrap().len(), 1);
        record.source = Some(json!({"job_id": id, "sha256": "f".repeat(64)}));
        let raw = public_generation_receipt(&record, |_| panic!("cycle must not read files")).unwrap();
        let parsed: Value = serde_json::from_slice(&raw).unwrap();
        assert_eq!(parsed["lineage_status"], "incomplete");
    }

    #[test]
    fn ancestor_hash_mismatch_and_depth_limit_report_incomplete_lineage() {
        let id = uuid::Uuid::new_v4().to_string();
        let parent_id = uuid::Uuid::new_v4().to_string();
        let mut child = record(&id, &"f".repeat(64), "public/Child");
        let parent = record(&parent_id, &"a".repeat(64), "public/Parent");
        child.source = Some(json!({"job_id": parent_id, "sha256": "b".repeat(64)}));
        let public = public_generation_receipt(&child, |_| Ok(parent.clone())).unwrap();
        let parsed: Value = serde_json::from_slice(&public).unwrap();
        assert_eq!(parsed["lineage_status"], "incomplete");
        assert_eq!(parsed["model_stages"].as_array().unwrap().len(), 1);

        // A genuinely long chain is bounded and marked incomplete, even if
        // every intermediate source is present and digest-consistent.
        let mut chain: Vec<Provenance> = (0..MAX_STAGES + 2)
            .map(|_| record(&uuid::Uuid::new_v4().to_string(),
                &"e".repeat(64), "public/Other"))
            .collect();
        for index in 0..chain.len() - 1 {
            let parent_id = chain[index + 1].job_id.clone();
            chain[index].source = Some(json!({
                "job_id": parent_id, "sha256": "e".repeat(64)
            }));
        }
        let first = chain[0].clone();
        let ancestors: std::collections::HashMap<_, _> = chain.into_iter()
            .skip(1).map(|record| (record.job_id.clone(), record)).collect();
        let result = public_generation_receipt(&first, |id| {
            ancestors.get(id).cloned().ok_or_else(|| "missing".into())
        }).unwrap();
        let parsed: Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(parsed["model_stages"].as_array().unwrap().len(), MAX_STAGES);
        assert_eq!(parsed["lineage_status"], "incomplete");
    }

    #[test]
    fn untrusted_model_urls_paths_and_overlong_values_are_not_exported() {
        let id = uuid::Uuid::new_v4().to_string();
        let mut record = record(&id, &"f".repeat(64), "https://token@example.com/model");
        record.model["revision"] = json!("C:\\private\\weights");
        let raw = public_generation_receipt(&record, |_| unreachable!()).unwrap();
        let parsed: Value = serde_json::from_slice(&raw).unwrap();
        assert!(parsed["model_stages"][0]["model_repo"].is_null());
        assert!(parsed["model_stages"][0]["model_revision"].is_null());
        assert!(!String::from_utf8(raw).unwrap().contains("token@example.com"));
    }
}
