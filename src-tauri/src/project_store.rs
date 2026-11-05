//! Small native project metadata store. Media linking and transformations are separate future work.
//! Originals are never accessed, copied, renamed, or deleted by these operations.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const VERSION: u32 = 1;
const MAX_RECORD_BYTES: u64 = 64 * 1024;
const MAX_NAME_CHARS: usize = 120;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectRecord {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub created_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub created_unix_ms: u64,
}

fn projects_dir(root: &Path) -> PathBuf {
    root.join("projects")
}

fn normalized_name(input: &str) -> Result<String, String> {
    let name = input.trim();
    if name.is_empty()
        || name.chars().count() > MAX_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return Err("Project name must be 1–120 characters with no control characters".into());
    }
    Ok(name.to_owned())
}

fn checked_id(input: &str) -> Result<Uuid, String> {
    Uuid::parse_str(input).map_err(|_| "Invalid project identifier".into())
}

fn project_path(root: &Path, id: &Uuid) -> PathBuf {
    projects_dir(root).join(format!("{id}.json"))
}

/// Persist an empty, named project without accepting arbitrary filesystem paths.
pub fn create(root: &Path, input_name: &str) -> Result<ProjectRecord, String> {
    let name = normalized_name(input_name)?;
    let id = Uuid::new_v4();
    let created_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_millis()
        .try_into()
        .map_err(|_| "System clock is out of range".to_string())?;
    let project = ProjectRecord {
        schema_version: VERSION,
        id: id.to_string(),
        name,
        created_unix_ms,
    };

    let dir = projects_dir(root);
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create project directory: {e}"))?;
    let bytes = serde_json::to_vec_pretty(&project).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("Project metadata exceeds storage limit".into());
    }

    let temporary = dir.join(format!(".{id}.tmp"));
    let final_path = project_path(root, &id);
    if final_path.exists() {
        return Err("Project identifier collision".into());
    }
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &final_path)
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(format!("Could not save project: {error}"));
    }
    Ok(project)
}

/// Read one bounded project record; the ID never becomes an unvalidated path.
pub fn open(root: &Path, raw_id: &str) -> Result<ProjectRecord, String> {
    let id = checked_id(raw_id)?;
    let path = project_path(root, &id);
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("Cannot open project: {e}"))?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_RECORD_BYTES {
        return Err("Project record is not a regular file or exceeds the size limit".into());
    }
    let bytes = fs::read(path).map_err(|e| format!("Cannot read project: {e}"))?;
    let record: ProjectRecord =
        serde_json::from_slice(&bytes).map_err(|e| format!("Project record is corrupt: {e}"))?;
    if record.schema_version != VERSION || record.id != id.to_string() {
        return Err("Project schema version or identity does not match".into());
    }
    if normalized_name(&record.name)? != record.name {
        return Err("Project name is invalid".into());
    }
    Ok(record)
}

/// Enumerate only UUID-named JSON records. A corrupt matching record is an error, not hidden.
pub fn list(root: &Path) -> Result<Vec<ProjectSummary>, String> {
    let dir = projects_dir(root);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut projects = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(id) = checked_id(stem) else {
            continue;
        };
        let record = open(root, &id.to_string())?;
        projects.push(ProjectSummary {
            id: record.id,
            name: record.name,
            created_unix_ms: record.created_unix_ms,
        });
    }
    projects.sort_by(|a, b| b.created_unix_ms.cmp(&a.created_unix_ms).then(a.id.cmp(&b.id)));
    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        std::env::temp_dir().join(format!("optic-rift-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn create_open_list_and_reopen() {
        let dir = root();
        assert!(list(&dir).unwrap().is_empty());
        let created = create(&dir, "  City Film  ").unwrap();
        assert_eq!(created.name, "City Film");
        assert_eq!(open(&dir, &created.id).unwrap(), created);
        let rows = list(&dir).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, created.id);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reject_invalid_name_and_path_traversal() {
        let dir = root();
        assert!(create(&dir, "  ").is_err());
        assert!(create(&dir, "\u{0000}").is_err());
        assert!(create(&dir, &"x".repeat(121)).is_err());
        assert!(open(&dir, "../../other-project.json").is_err());
        assert!(!dir.exists());
    }

    #[test]
    fn corrupt_and_wrong_identity_are_not_valid_projects() {
        let dir = root();
        let project = create(&dir, "Test").unwrap();
        let path = project_path(&dir, &checked_id(&project.id).unwrap());
        fs::write(&path, b"{ broken json").unwrap();
        assert!(open(&dir, &project.id).is_err());
        assert!(list(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
