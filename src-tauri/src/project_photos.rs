//! Experimental managed PNG project media and immutable photo-edit revisions.
//! Sources are stored under app data and never overwritten. Only explicit
//! project-photo-save commands can add or revise this data. This is not a
//! portable project bundle, an encrypted vault, or a general copy/link policy.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Cursor, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

const VERSION: u32 = 1;
const MAX_SOURCE_BYTES: usize = 32 * 1024 * 1024;
const MAX_DECODE_BYTES: usize = 80 * 1024 * 1024;
const MAX_EDGE: u32 = 8192;
const MAX_PIXELS: u64 = 20_000_000;
const MAX_DOCUMENT_BYTES: u64 = 16 * 1024;
const MAX_ASSETS: usize = 2048;
const MAX_REVISIONS: u32 = 1000;
static SAVE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhotoRecipe {
    pub schema_version: u32,
    pub brightness: u16,
    pub contrast: u16,
    pub saturation: u16,
    pub rotation: u16,
    pub straighten_tenths: i16,
    pub resize_long_edge: Option<u32>,
    pub crop: Option<crate::photo_export::PhotoCrop>,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavePhotoRequest {
    pub project_id: String,
    pub asset_id: Option<String>,
    pub expected_revision: Option<u32>,
    pub source_name: String,
    pub source_png_base64: String,
    pub edits: PhotoRecipe,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhotoDocument {
    pub schema_version: u32,
    pub project_id: String,
    pub asset_id: String,
    pub revision: u32,
    pub source_name: String,
    pub source_sha256: String,
    pub source_width: u32,
    pub source_height: u32,
    pub edits: PhotoRecipe,
}

#[derive(Debug, Serialize)]
pub struct PhotoProblem {
    pub asset_id: String,
    pub error: String,
}
#[derive(Debug, Serialize, Default)]
pub struct PhotoListing {
    pub photos: Vec<PhotoDocument>,
    pub problems: Vec<PhotoProblem>,
}
#[derive(Debug, Serialize)]
pub struct OpenedPhoto {
    pub document: PhotoDocument,
    pub source_png_base64: String,
}

fn checked_id(raw: &str) -> Result<Uuid, String> {
    match Uuid::parse_str(raw) {
        Ok(id) if id.hyphenated().to_string() == raw => Ok(id),
        _ => Err("Invalid canonical project or asset identifier".into()),
    }
}

fn media_root(root: &Path) -> PathBuf { root.join("project-media") }
fn project_dir(root: &Path, project: &Uuid) -> PathBuf {
    media_root(root).join(project.to_string())
}
fn asset_dir(root: &Path, project: &Uuid, asset: &Uuid) -> PathBuf {
    project_dir(root, project).join(asset.to_string())
}

#[cfg(windows)]
fn reparse(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_meta: &fs::Metadata) -> bool { false }

fn real_directory(dir: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(dir).map_err(|_| "Project media folder is unavailable.")?;
    if !meta.is_dir() || meta.file_type().is_symlink() || reparse(&meta) {
        return Err("Project media folder is redirected or not a directory.".into());
    }
    Ok(())
}
fn ensure_directory(dir: &Path) -> Result<(), String> {
    match fs::create_dir(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Could not create managed photo folder.".into()),
    }
    real_directory(dir)
}
fn verify_parents(root: &Path, project: &Uuid, create: bool) -> Result<bool, String> {
    // The root is supplied by Tauri's app_data_dir, never by the WebView.
    if create {
        fs::create_dir_all(root).map_err(|_| "App data folder is unavailable.")?;
        real_directory(root)?;
        ensure_directory(&media_root(root))?;
        ensure_directory(&project_dir(root, project))?;
        return Ok(true);
    }
    real_directory(root)?;
    match fs::symlink_metadata(media_root(root)) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("Cannot inspect managed photo storage.".into()),
        Ok(_) => real_directory(&media_root(root))?,
    }
    match fs::symlink_metadata(project_dir(root, project)) {
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err("Cannot inspect project photo folder.".into()),
        Ok(_) => { real_directory(&project_dir(root, project))?; Ok(true) }
    }
}

fn validate_recipe(r: &PhotoRecipe) -> Result<(), String> {
    if r.schema_version != VERSION || !(25..=175).contains(&r.brightness)
        || !(25..=175).contains(&r.contrast) || r.saturation > 200
        || !matches!(r.rotation, 0 | 90 | 180 | 270)
        || !(-150..=150).contains(&r.straighten_tenths)
        || r.resize_long_edge.is_some_and(|v| !(32..=MAX_EDGE).contains(&v))
    {
        return Err("Photo edit recipe version or values are invalid.".into());
    }
    if let Some(c) = &r.crop {
        let x = u32::from(c.left) + u32::from(c.right);
        let y = u32::from(c.top) + u32::from(c.bottom);
        if [c.left, c.top, c.right, c.bottom].iter().any(|v| *v > 9500)
            || x >= 10_000 || y >= 10_000 {
            return Err("Photo crop recipe is invalid.".into());
        }
    }
    Ok(())
}
fn validate_name(name: &str) -> Result<(), String> {
    if name.trim() != name || name.is_empty() || name.chars().count() > 120
        || name.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err("Photo name is missing or unsafe.".into());
    }
    Ok(())
}

fn checked_png(bytes: &[u8]) -> Result<(u32, u32), String> {
    if bytes.is_empty() || bytes.len() > MAX_SOURCE_BYTES
        || !bytes.starts_with(&[137, 80, 78, 71, 13, 10, 26, 10]) {
        return Err("Managed project imports currently support PNG files up to 32 MB.".into());
    }
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder.read_info().map_err(|_| "PNG header is invalid.")?;
    let w = reader.info().width;
    let h = reader.info().height;
    if w == 0 || h == 0 || w > MAX_EDGE || h > MAX_EDGE
        || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err("PNG dimensions exceed the supported project limit.".into());
    }
    let capacity = reader.output_buffer_size().ok_or("PNG cannot be decoded safely.")?;
    if capacity > MAX_DECODE_BYTES {
        return Err("PNG decoded image exceeds the memory limit.".into());
    }
    let mut buffer = vec![0; capacity];
    let frame = reader.next_frame(&mut buffer).map_err(|_| "PNG data is corrupt.")?;
    if frame.width != w || frame.height != h {
        return Err("PNG decoded dimensions do not match the source header.".into());
    }
    Ok((w, h))
}

fn bounded_base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    if encoded.len() > MAX_SOURCE_BYTES.div_ceil(3) * 4 + 4 {
        return Err("Project PNG encoded bytes exceed the 32 MB limit.".into());
    }
    base64::engine::general_purpose::STANDARD.decode(encoded)
        .map_err(|_| "PNG source bytes are not valid base64.".to_string())
}
fn source_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn revision_filename(revision: u32) -> String {
    format!("r{revision:06}.json")
}
fn checked_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let meta = fs::symlink_metadata(path).map_err(|_| "Project photo file is missing.")?;
    if !meta.is_file() || meta.file_type().is_symlink() || reparse(&meta) || meta.len() > limit {
        return Err("Project photo file is not regular or exceeds its storage limit.".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|f| f.take(limit + 1).read_to_end(&mut bytes))
        .map_err(|_| "Could not read project photo data.")?;
    if bytes.len() as u64 > limit {
        return Err("Project photo data grew beyond its storage limit.".into());
    }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_| "Cannot create a unique project photo file.")?;
    f.write_all(bytes).and_then(|_| f.sync_all())
        .map_err(|_| "Could not commit project photo bytes.")?;
    Ok(())
}
fn write_revision(dir: &Path, doc: &PhotoDocument) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(doc).map_err(|_| "Cannot encode edit document.")?;
    if bytes.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err("Photo edit document exceeds size limit.".into());
    }
    let destination = dir.join(revision_filename(doc.revision));
    if fs::symlink_metadata(&destination).is_ok() {
        return Err("A photo revision already exists; reload before saving.".into());
    }
    let temporary = dir.join(format!(".{}.{}.partial", doc.revision, Uuid::new_v4()));
    write_new(&temporary, &bytes)?;
    if let Err(_) = fs::rename(&temporary, &destination) {
        // A failed rename leaves only a hidden, ignored file; never a phantom revision.
        return Err("Could not commit a complete photo revision.".into());
    }
    Ok(())
}
fn latest_document(root: &Path, project: &Uuid, asset: &Uuid) -> Result<PhotoDocument, String> {
    let dir = asset_dir(root, project, asset);
    real_directory(&dir)?;
    let mut latest = 0;
    let mut scanned = 0usize;
    for entry in fs::read_dir(&dir).map_err(|_| "Cannot inspect photo revisions.")? {
        let entry = entry.map_err(|_| "Cannot inspect photo revision.")?;
        scanned += 1;
        if scanned > MAX_REVISIONS as usize + 12 {
            return Err("Too many photo revision records.".into());
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(raw) = name.strip_prefix('r').and_then(|s| s.strip_suffix(".json")) else {
            continue;
        };
        let Ok(revision) = raw.parse::<u32>() else { continue };
        if revision == 0 || revision > MAX_REVISIONS
            || revision_filename(revision) != name { continue }
        latest = latest.max(revision);
    }
    if latest == 0 {
        return Err("Photo has no committed edit revision.".into());
    }
    let bytes = checked_file(&dir.join(revision_filename(latest)), MAX_DOCUMENT_BYTES)?;
    let doc: PhotoDocument = serde_json::from_slice(&bytes)
        .map_err(|_| "Photo edit document is corrupt.")?;
    if doc.schema_version != VERSION || doc.project_id != project.to_string()
        || doc.asset_id != asset.to_string() || doc.revision != latest {
        return Err("Photo edit document identity or revision is invalid.".into());
    }
    validate_name(&doc.source_name)?;
    validate_recipe(&doc.edits)?;
    if doc.source_sha256.len() != 64
        || !doc.source_sha256.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || doc.source_width == 0 || doc.source_height == 0
        || doc.source_width > MAX_EDGE || doc.source_height > MAX_EDGE
        || u64::from(doc.source_width) * u64::from(doc.source_height) > MAX_PIXELS {
        return Err("Photo source metadata is invalid.".into());
    }
    Ok(doc)
}

fn checked_source(root: &Path, project: &Uuid, asset: &Uuid, doc: &PhotoDocument)
    -> Result<Vec<u8>, String> {
    let bytes = checked_file(&asset_dir(root, project, asset).join("original.png"), MAX_SOURCE_BYTES as u64)?;
    if source_digest(&bytes) != doc.source_sha256 {
        return Err("The project original has changed. Refusing to restore altered media.".into());
    }
    let (w, h) = checked_png(&bytes)?;
    if (w, h) != (doc.source_width, doc.source_height) {
        return Err("The project source dimensions do not match its saved document.".into());
    }
    Ok(bytes)
}

/// Explicitly save an immutable original PNG plus a versioned edit recipe.
pub fn save(root: &Path, req: SavePhotoRequest) -> Result<PhotoDocument, String> {
    let _guard = SAVE_LOCK.lock().map_err(|_| "Project save coordination failed.")?;
    let project = checked_id(&req.project_id)?;
    crate::project_store::open(root, &req.project_id)?;
    validate_name(&req.source_name)?;
    validate_recipe(&req.edits)?;
    let bytes = bounded_base64_decode(&req.source_png_base64)?;
    let (width, height) = checked_png(&bytes)?;
    let digest = source_digest(&bytes);
    verify_parents(root, &project, true)?;

    if let Some(raw_asset_id) = &req.asset_id {
        let asset = checked_id(raw_asset_id)?;
        let latest = latest_document(root, &project, &asset)?;
        if Some(latest.revision) != req.expected_revision {
            return Err("This photo has a newer saved revision; reopen it before saving.".into());
        }
        if latest.source_sha256 != digest || latest.source_name != req.source_name {
            return Err("Project photo source identity has changed; original will not be overwritten.".into());
        }
        let existing = checked_source(root, &project, &asset, &latest)?;
        if existing != bytes { return Err("Project original bytes differ from the saved source.".into()); }
        if latest.revision >= MAX_REVISIONS {
            return Err("Project photo has reached its revision limit.".into());
        }
        let doc = PhotoDocument {
            revision: latest.revision + 1, edits: req.edits, ..latest
        };
        write_revision(&asset_dir(root, &project, &asset), &doc)?;
        return Ok(doc);
    }
    if req.expected_revision.is_some() {
        return Err("A new photo cannot declare an existing revision.".into());
    }

    let asset = Uuid::new_v4();
    let parent = project_dir(root, &project);
    let staging = parent.join(format!(".{}.partial", asset));
    let final_dir = asset_dir(root, &project, &asset);
    if fs::symlink_metadata(&final_dir).is_ok() {
        return Err("Managed asset identifier collision.".into());
    }
    fs::create_dir(&staging).map_err(|_| "Cannot stage a new project photo.".to_string())?;
    real_directory(&staging)?;
    let doc = PhotoDocument {
        schema_version: VERSION,
        project_id: project.to_string(),
        asset_id: asset.to_string(),
        revision: 1,
        source_name: req.source_name,
        source_sha256: digest,
        source_width: width,
        source_height: height,
        edits: req.edits,
    };
    write_new(&staging.join("original.png"), &bytes)?;
    write_revision(&staging, &doc)?;
    // Atomic folder publication. A crash before rename leaves only hidden
    // .<uuid>.partial staging data, which list() never advertises as an asset.
    fs::rename(&staging, &final_dir)
        .map_err(|_| "Could not publish the staged project photo.".to_string())?;
    Ok(doc)
}

pub fn list(root: &Path, project_id: &str) -> Result<PhotoListing, String> {
    let project = checked_id(project_id)?;
    crate::project_store::open(root, project_id)?;
    if !verify_parents(root, &project, false)? {
        return Ok(PhotoListing::default());
    }
    let mut listing = PhotoListing::default();
    let mut count = 0usize;
    for entry in fs::read_dir(project_dir(root, &project))
        .map_err(|_| "Cannot list project photo assets.")? {
        let entry = entry.map_err(|_| "Cannot inspect project asset.")?;
        count += 1;
        if count > MAX_ASSETS {
            return Err("Project has too many media entries to list safely.".into());
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        let Ok(asset) = checked_id(&raw) else { continue };
        match latest_document(root, &project, &asset) {
            Ok(doc) => listing.photos.push(doc),
            Err(error) => listing.problems.push(PhotoProblem { asset_id: raw, error }),
        }
    }
    listing.photos.sort_by(|a, b| a.source_name.cmp(&b.source_name).then(a.asset_id.cmp(&b.asset_id)));
    listing.problems.sort_by(|a, b| a.asset_id.cmp(&b.asset_id));
    Ok(listing)
}

pub fn open(root: &Path, project_id: &str, asset_id: &str) -> Result<OpenedPhoto, String> {
    let project = checked_id(project_id)?;
    let asset = checked_id(asset_id)?;
    crate::project_store::open(root, project_id)?;
    if !verify_parents(root, &project, false)? {
        return Err("This project has no managed photos.".into());
    }
    let doc = latest_document(root, &project, &asset)?;
    let bytes = checked_source(root, &project, &asset, &doc)?;
    Ok(OpenedPhoto {
        document: doc,
        source_png_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestRoot(PathBuf);
    impl TestRoot {
        fn new() -> Self { Self(std::env::temp_dir().join(format!("optic-photo-{}", Uuid::new_v4()))) }
        fn path(&self) -> &Path { &self.0 }
    }
    impl Drop for TestRoot {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    fn png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255u8; 16]).unwrap();
        }
        bytes
    }
    fn recipe() -> PhotoRecipe {
        PhotoRecipe {
            schema_version: 1, brightness: 100, contrast: 100, saturation: 100,
            rotation: 0, straighten_tenths: 0, resize_long_edge: None,
            crop: None, flip_horizontal: false, flip_vertical: false,
        }
    }
    fn request(project: &str) -> SavePhotoRequest {
        SavePhotoRequest {
            project_id: project.into(), asset_id: None, expected_revision: None,
            source_name: "Lake.png".into(),
            source_png_base64: base64::engine::general_purpose::STANDARD.encode(png_bytes()),
            edits: recipe(),
        }
    }

    #[test]
    fn first_save_reopen_new_revision_and_preserve_immutable_original() {
        let root = TestRoot::new();
        let project = crate::project_store::create(root.path(), "Lake").unwrap();
        let initial = save(root.path(), request(&project.id)).unwrap();
        assert_eq!(initial.revision, 1);
        assert_eq!(list(root.path(), &project.id).unwrap().photos.len(), 1);
        let original = open(root.path(), &project.id, &initial.asset_id).unwrap();
        assert_eq!(original.source_png_base64,
            base64::engine::general_purpose::STANDARD.encode(png_bytes()));
        // Simulate restart by reading the same project root with no in-memory object.
        let mut next = request(&project.id);
        next.asset_id = Some(initial.asset_id.clone());
        next.expected_revision = Some(1);
        next.edits.saturation = 120;
        next.edits.rotation = 90;
        let second = save(root.path(), next).unwrap();
        assert_eq!(second.revision, 2);
        assert_eq!(second.edits.saturation, 120);
        assert_eq!(open(root.path(), &project.id, &initial.asset_id).unwrap().document.revision, 2);
        assert_eq!(fs::read(asset_dir(root.path(), &checked_id(&project.id).unwrap(),
            &checked_id(&initial.asset_id).unwrap()).join("original.png")).unwrap(), png_bytes());
        assert!(asset_dir(root.path(), &checked_id(&project.id).unwrap(),
            &checked_id(&initial.asset_id).unwrap()).join("r000001.json").exists());
    }

    #[test]
    fn reject_stale_revision_forged_path_changed_original_and_invalid_recipes() {
        let root = TestRoot::new();
        let project = crate::project_store::create(root.path(), "Lake").unwrap();
        let first = save(root.path(), request(&project.id)).unwrap();
        let mut stale = request(&project.id);
        stale.asset_id = Some(first.asset_id.clone());
        stale.expected_revision = Some(0);
        assert!(save(root.path(), stale).unwrap_err().contains("newer saved revision"));
        assert!(open(root.path(), &project.id, "../../evil").is_err());
        let mut invalid = request(&project.id);
        invalid.edits.brightness = 200;
        assert!(save(root.path(), invalid).is_err());
        let mut wrong_type = request(&project.id);
        wrong_type.source_png_base64 = base64::engine::general_purpose::STANDARD.encode(b"not png");
        assert!(save(root.path(), wrong_type).is_err());
        let folder = asset_dir(root.path(), &checked_id(&project.id).unwrap(),
            &checked_id(&first.asset_id).unwrap());
        fs::write(folder.join("original.png"), b"forged").unwrap();
        assert!(open(root.path(), &project.id, &first.asset_id).is_err());
        // A broken original cannot be silently overwritten by saving another revision.
        let mut retry = request(&project.id);
        retry.asset_id = Some(first.asset_id);
        retry.expected_revision = Some(1);
        assert!(save(root.path(), retry).is_err());
    }

    #[test]
    fn managed_assets_are_strictly_project_scoped_and_invalid_imports_do_not_write() {
        let root = TestRoot::new();
        let a = crate::project_store::create(root.path(), "A").unwrap();
        let b = crate::project_store::create(root.path(), "B").unwrap();
        let mut malformed = request(&a.id);
        malformed.source_png_base64 = base64::engine::general_purpose::STANDARD.encode(b"not png");
        assert!(save(root.path(), malformed).is_err());
        assert!(!project_dir(root.path(), &checked_id(&a.id).unwrap()).exists());

        let original = save(root.path(), request(&a.id)).unwrap();
        assert!(open(root.path(), &b.id, &original.asset_id).is_err());
        assert!(list(root.path(), &b.id).unwrap().photos.is_empty());
        assert_eq!(list(root.path(), &a.id).unwrap().photos.len(), 1);
        assert!(save(root.path(), SavePhotoRequest {
            project_id: a.id.clone(), asset_id: Some("../../evil".into()),
            expected_revision: Some(1), ..request(&a.id)
        }).is_err());
        assert_eq!(open(root.path(), &a.id, &original.asset_id).unwrap().document.revision, 1);
    }

    #[test]
    fn incomplete_staging_is_not_reported_as_a_saved_photo() {
        let root = TestRoot::new();
        let project = crate::project_store::create(root.path(), "Lake").unwrap();
        let saved = save(root.path(), request(&project.id)).unwrap();
        let folder = project_dir(root.path(), &checked_id(&project.id).unwrap());
        fs::create_dir(folder.join(format!(".{}.partial", Uuid::new_v4()))).unwrap();
        assert_eq!(list(root.path(), &project.id).unwrap().photos.len(), 1);
        assert_eq!(open(root.path(), &project.id, &saved.asset_id).unwrap().document.revision, 1);
    }

    #[test]
    fn corrupt_revision_visible_as_problem_not_hidden_or_trusted() {
        let root = TestRoot::new();
        let project = crate::project_store::create(root.path(), "Lake").unwrap();
        let saved = save(root.path(), request(&project.id)).unwrap();
        let folder = asset_dir(root.path(), &checked_id(&project.id).unwrap(),
            &checked_id(&saved.asset_id).unwrap());
        fs::write(folder.join("r000001.json"), b"{bad json").unwrap();
        let listing = list(root.path(), &project.id).unwrap();
        assert_eq!(listing.photos.len(), 0);
        assert_eq!(listing.problems.len(), 1);
        assert!(open(root.path(), &project.id, &saved.asset_id).is_err());
    }
}
