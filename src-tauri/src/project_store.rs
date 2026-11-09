//! Small native project metadata store. Media linking and transformations are separate future work.
//! Originals are never accessed, copied, renamed, or deleted by these operations.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const VERSION: u32 = 1;
const MAX_RECORD_BYTES: u64 = 64 * 1024;
const MAX_NAME_CHARS: usize = 120;
const TEMP_SUFFIX: &str = ".tmp";

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

/// A project-shaped record that could not be read. Reported, never silently hidden.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectProblem {
    pub file_name: String,
    pub error: String,
}

/// Readable projects plus any unreadable records, so one corrupt file cannot hide the rest.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct ProjectListing {
    pub projects: Vec<ProjectSummary>,
    pub problems: Vec<ProjectProblem>,
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

/// Accept only the canonical lowercase hyphenated UUID form that this store writes.
/// `Uuid::parse_str` alone also accepts braced, URN and unhyphenated spellings.
fn checked_id(input: &str) -> Result<Uuid, String> {
    match Uuid::parse_str(input) {
        Ok(id) if id.hyphenated().to_string() == input => Ok(id),
        _ => Err("Invalid project identifier".into()),
    }
}

fn project_path(root: &Path, id: &Uuid) -> PathBuf {
    projects_dir(root).join(format!("{id}.json"))
}

/// Refuse a projects directory that is a symbolic link or junction (any name-surrogate
/// reparse point on Windows), so records cannot be redirected outside the app data root.
fn ensure_real_directory(dir: &Path) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(dir).map_err(|e| format!("Cannot inspect project directory: {e}"))?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err("Project directory is a link or reparse point; refusing to use it".into());
    }
    if !metadata.is_dir() {
        return Err("Project directory path is not a directory".into());
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

/// Persist an empty, named project without accepting arbitrary filesystem paths.
///
/// Crash consistency: the record is written to a `create_new` temporary file, flushed with
/// `sync_all`, then renamed into place. A kill before the rename leaves only an orphaned
/// `.<id>.tmp` file, which `list` never reports as a project. The destination name is a fresh
/// random v4 UUID, so the rename cannot clobber an existing record in practice; the
/// `create_new` temporary also fails rather than overwriting.
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
    ensure_real_directory(&dir)?;
    let bytes = serde_json::to_vec_pretty(&project).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("Project metadata exceeds storage limit".into());
    }

    let temporary = dir.join(format!(".{id}{TEMP_SUFFIX}"));
    let final_path = project_path(root, &id);
    if fs::symlink_metadata(&final_path).is_ok() {
        return Err("Project identifier collision".into());
    }
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
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
    ensure_real_directory(&projects_dir(root))?;
    let path = project_path(root, &id);
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("Cannot open project: {e}"))?;
    if !metadata.file_type().is_file() || is_reparse_point(&metadata) {
        return Err("Project record is not a regular file".into());
    }
    if metadata.len() > MAX_RECORD_BYTES {
        return Err("Project record exceeds the size limit".into());
    }
    // Bound the read itself too: the file may grow between the metadata check and the read.
    let mut bytes = Vec::new();
    File::open(&path)
        .and_then(|file| file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| format!("Cannot read project: {e}"))?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err("Project record exceeds the size limit".into());
    }
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

/// Enumerate canonical `<uuid>.json` records. Unreadable records are returned as problems.
pub fn list(root: &Path) -> Result<ProjectListing, String> {
    let dir = projects_dir(root);
    match fs::symlink_metadata(&dir) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(ProjectListing::default()),
        Err(error) => return Err(format!("Cannot inspect project directory: {error}")),
        Ok(_) => ensure_real_directory(&dir)?,
    }
    let mut listing = ProjectListing::default();
    for entry in fs::read_dir(&dir).map_err(|e| format!("Cannot list projects: {e}"))? {
        let entry = entry.map_err(|e| format!("Cannot list projects: {e}"))?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let Some(stem) = file_name.strip_suffix(".json") else {
            continue;
        };
        let Ok(id) = checked_id(stem) else {
            continue;
        };
        match open(root, &id.to_string()) {
            Ok(record) => listing.projects.push(ProjectSummary {
                id: record.id,
                name: record.name,
                created_unix_ms: record.created_unix_ms,
            }),
            Err(error) => listing.problems.push(ProjectProblem { file_name, error }),
        }
    }
    listing.projects.sort_by(|a, b| {
        b.created_unix_ms
            .cmp(&a.created_unix_ms)
            .then(a.id.cmp(&b.id))
    });
    listing
        .problems
        .sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(listing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::{Arc, Barrier};
    use std::thread;

    /// Temporary root removed on drop, even when an assertion fails.
    struct TestRoot(PathBuf);
    impl TestRoot {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("optic-rift-test-{}", Uuid::new_v4())))
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn record_path(root: &Path, id: &str) -> PathBuf {
        project_path(root, &checked_id(id).unwrap())
    }

    #[test]
    fn create_open_list_and_reopen() {
        let root = TestRoot::new();
        let dir = root.path();
        assert!(list(dir).unwrap().projects.is_empty());
        let created = create(dir, "  City Film  ").unwrap();
        assert_eq!(created.name, "City Film");
        assert_eq!(open(dir, &created.id).unwrap(), created);
        let rows = list(dir).unwrap();
        assert_eq!(rows.projects.len(), 1);
        assert_eq!(rows.projects[0].id, created.id);
        assert!(rows.problems.is_empty());
    }

    #[test]
    fn reopen_from_fresh_state_after_simulated_restart() {
        // No in-memory state exists in the store; a "restart" is a fresh read of the same root.
        let root = TestRoot::new();
        let a = create(root.path(), "First").unwrap();
        let b = create(root.path(), "Second").unwrap();
        let path_copy = root.path().to_path_buf();
        let listing = thread::spawn(move || list(&path_copy).unwrap())
            .join()
            .unwrap();
        let ids: Vec<_> = listing.projects.iter().map(|p| p.id.clone()).collect();
        assert!(ids.contains(&a.id) && ids.contains(&b.id));
        assert_eq!(open(root.path(), &b.id).unwrap().name, "Second");
    }

    #[test]
    fn reject_invalid_name_and_path_traversal() {
        let root = TestRoot::new();
        let dir = root.path();
        assert!(create(dir, "  ").is_err());
        assert!(create(dir, "\u{0000}").is_err());
        assert!(create(dir, "line\nbreak").is_err());
        assert!(create(dir, &"x".repeat(121)).is_err());
        assert!(
            create(dir, &"é".repeat(120)).is_ok(),
            "limit counts characters, not bytes"
        );
        assert!(open(dir, "../../other-project.json").is_err());
        assert!(open(dir, "..\\..\\Windows\\win.ini").is_err());
        assert!(open(dir, "C:\\Windows\\win.ini").is_err());
    }

    #[test]
    fn only_canonical_identifiers_are_accepted() {
        let root = TestRoot::new();
        let created = create(root.path(), "Canon").unwrap();
        let id = Uuid::parse_str(&created.id).unwrap();
        for alias in [
            id.simple().to_string(),
            id.braced().to_string(),
            id.urn().to_string(),
            created.id.to_uppercase(),
        ] {
            assert!(open(root.path(), &alias).is_err(), "accepted alias {alias}");
        }
        // A stray file using a non-canonical spelling is ignored rather than breaking listing.
        fs::write(
            projects_dir(root.path()).join(format!("{}.json", id.simple())),
            b"{}",
        )
        .unwrap();
        let listing = list(root.path()).unwrap();
        assert_eq!(listing.projects.len(), 1);
        assert!(listing.problems.is_empty());
    }

    #[test]
    fn corrupt_and_wrong_identity_are_reported_without_hiding_valid_projects() {
        let root = TestRoot::new();
        let dir = root.path();
        let good = create(dir, "Good").unwrap();
        let corrupt = create(dir, "Corrupt").unwrap();
        let mismatched = create(dir, "Mismatched").unwrap();
        let future = create(dir, "Future").unwrap();

        fs::write(record_path(dir, &corrupt.id), b"{ broken json").unwrap();
        let mut other = mismatched.clone();
        other.id = Uuid::new_v4().to_string();
        fs::write(
            record_path(dir, &mismatched.id),
            serde_json::to_vec(&other).unwrap(),
        )
        .unwrap();
        let mut newer = future.clone();
        newer.schema_version = 99;
        fs::write(
            record_path(dir, &future.id),
            serde_json::to_vec(&newer).unwrap(),
        )
        .unwrap();

        assert!(open(dir, &corrupt.id).is_err());
        assert!(open(dir, &mismatched.id).is_err());
        assert!(open(dir, &future.id).is_err());
        let listing = list(dir).unwrap();
        assert_eq!(listing.projects.len(), 1);
        assert_eq!(listing.projects[0].id, good.id);
        assert_eq!(listing.problems.len(), 3);
    }

    #[test]
    fn invalid_name_stored_on_disk_is_rejected() {
        let root = TestRoot::new();
        let project = create(root.path(), "Fine").unwrap();
        let mut tampered = project.clone();
        tampered.name = "  padded  ".into();
        fs::write(
            record_path(root.path(), &project.id),
            serde_json::to_vec(&tampered).unwrap(),
        )
        .unwrap();
        assert!(open(root.path(), &project.id).is_err());
    }

    #[test]
    fn oversize_record_is_rejected() {
        let root = TestRoot::new();
        let project = create(root.path(), "Big").unwrap();
        let big = vec![b' '; (MAX_RECORD_BYTES + 1) as usize];
        fs::write(record_path(root.path(), &project.id), big).unwrap();
        assert!(open(root.path(), &project.id)
            .unwrap_err()
            .contains("size limit"));
    }

    #[test]
    fn directory_in_place_of_record_is_rejected() {
        let root = TestRoot::new();
        let project = create(root.path(), "Dir").unwrap();
        let path = record_path(root.path(), &project.id);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(open(root.path(), &project.id).is_err());
        assert_eq!(list(root.path()).unwrap().problems.len(), 1);
    }

    #[test]
    fn interrupted_write_leaves_no_false_project() {
        let root = TestRoot::new();
        let project = create(root.path(), "Real").unwrap();
        // Simulate a process killed after writing its temporary file but before rename.
        let orphan = projects_dir(root.path()).join(format!(".{}{TEMP_SUFFIX}", Uuid::new_v4()));
        fs::write(&orphan, b"{\"partial\":").unwrap();
        let listing = list(root.path()).unwrap();
        assert_eq!(listing.projects.len(), 1);
        assert_eq!(listing.projects[0].id, project.id);
        assert!(listing.problems.is_empty());
        // Subsequent creates are unaffected.
        assert!(create(root.path(), "After crash").is_ok());
    }

    #[test]
    fn failed_create_leaves_no_project_or_temporary() {
        let root = TestRoot::new();
        fs::create_dir_all(root.path()).unwrap();
        // A file where the projects directory should be makes directory creation fail.
        fs::write(projects_dir(root.path()), b"not a directory").unwrap();
        assert!(create(root.path(), "Blocked").is_err());
        assert!(list(root.path()).is_err());
        let names: Vec<_> = fs::read_dir(root.path()).unwrap().collect();
        assert_eq!(names.len(), 1, "no temporary or record was left behind");
    }

    #[test]
    fn concurrent_creates_all_persist_with_unique_ids() {
        let root = TestRoot::new();
        let threads = 16;
        let barrier = Arc::new(Barrier::new(threads));
        let handles: Vec<_> = (0..threads)
            .map(|i| {
                let barrier = barrier.clone();
                let dir = root.path().to_path_buf();
                thread::spawn(move || {
                    barrier.wait();
                    create(&dir, &format!("Concurrent {i}")).unwrap()
                })
            })
            .collect();
        let mut ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap().id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), threads);
        let listing = list(root.path()).unwrap();
        assert_eq!(listing.projects.len(), threads);
        assert!(listing.problems.is_empty());
        let leftovers = fs::read_dir(projects_dir(root.path()))
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(TEMP_SUFFIX)
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn unrelated_files_in_store_are_never_modified() {
        let root = TestRoot::new();
        let dir = projects_dir(root.path());
        fs::create_dir_all(&dir).unwrap();
        let original = dir.join("holiday.mp4");
        let bytes: Vec<u8> = (0..4096u32).map(|n| (n % 251) as u8).collect();
        fs::write(&original, &bytes).unwrap();
        let before = fs::metadata(&original).unwrap().modified().unwrap();
        let p = create(root.path(), "Keeps media").unwrap();
        list(root.path()).unwrap();
        open(root.path(), &p.id).unwrap();
        assert_eq!(fs::read(&original).unwrap(), bytes);
        assert_eq!(fs::metadata(&original).unwrap().modified().unwrap(), before);
    }

    #[cfg(windows)]
    #[test]
    fn junction_projects_directory_is_refused() {
        let root = TestRoot::new();
        let elsewhere = TestRoot::new();
        fs::create_dir_all(root.path()).unwrap();
        create(elsewhere.path(), "Outside").unwrap();
        // Directory junctions need no elevation, unlike symbolic links.
        let status = Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(projects_dir(root.path()))
            .arg(projects_dir(elsewhere.path()))
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed: {status:?}");
        assert!(list(root.path()).is_err());
        assert!(create(root.path(), "Redirected").is_err());
        let outside = list(elsewhere.path()).unwrap();
        assert_eq!(
            outside.projects.len(),
            1,
            "nothing was written through the junction"
        );
        // Remove the junction itself (not its target) before cleanup.
        fs::remove_dir(projects_dir(root.path())).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn symlinked_record_is_refused_when_symlinks_are_available() {
        let root = TestRoot::new();
        let project = create(root.path(), "Linked").unwrap();
        let target = root.path().join("target.json");
        let path = record_path(root.path(), &project.id);
        fs::rename(&path, &target).unwrap();
        if std::os::windows::fs::symlink_file(&target, &path).is_err() {
            eprintln!("symlink creation needs Developer Mode or elevation; skipped");
            return;
        }
        assert!(open(root.path(), &project.id).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn permission_denied_is_reported_and_creates_nothing() {
        let root = TestRoot::new();
        let dir = projects_dir(root.path());
        fs::create_dir_all(&dir).unwrap();
        let user = std::env::var("USERNAME").unwrap();
        let deny = Command::new("icacls")
            .arg(&dir)
            .args(["/deny", &format!("{user}:(WD,AD)")])
            .output()
            .unwrap();
        assert!(deny.status.success(), "icacls deny failed: {deny:?}");
        let result = create(root.path(), "No access");
        let _ = Command::new("icacls")
            .arg(&dir)
            .args(["/remove:d", &user])
            .output();
        let error = result.unwrap_err();
        assert!(error.contains("Could not save project"), "{error}");
        assert!(list(root.path()).unwrap().projects.is_empty());
    }
}
