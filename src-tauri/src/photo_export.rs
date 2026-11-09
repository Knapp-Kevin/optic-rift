//! Deterministic photo export, independent of all inference models.
//! The webview renders the visible adjustments into a bounded PNG. The native
//! command decodes and verifies that PNG, then creates a *new* file and a
//! minimal export receipt in the user's Pictures/Optic Rift directory.
//! Export does not imply an editable saved project or a private vault.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_PNG_BYTES: usize = 32 * 1024 * 1024;
const MAX_PNG_EDGE: u32 = 8192;
const MAX_PNG_PIXELS: u64 = 20_000_000;
const MAX_DECODE_BYTES: usize = 80 * 1024 * 1024;

fn neutral_saturation_percent() -> u16 { 100 }

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct PhotoCrop {
    /// Insets in basis points of the rotated source image. 10000 = 100%.
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhotoExportRequest {
    pub png_base64: String,
    pub source_sha256: String,
    pub brightness: u16,
    pub contrast: u16,
    /// Defaults to neutral for pre-saturation clients.
    #[serde(default = "neutral_saturation_percent")]
    pub saturation_percent: u16,
    pub rotation: u16,
    /// Fine clockwise leveling in tenths of degrees; neutral for old clients.
    #[serde(default)]
    pub straighten_tenths: i16,
    /// WebView-requested longest output edge; absent for original-size exports.
    #[serde(default)]
    pub resize_long_edge: Option<u32>,
    pub crop: Option<PhotoCrop>,
    /// Optional for older callers; strict booleans prevent forged IPC shapes.
    #[serde(default)]
    pub flip_horizontal: bool,
    #[serde(default)]
    pub flip_vertical: bool,
    pub width: u32,
    pub height: u32,
    pub accept_pictures_export: bool,
}

#[derive(Debug, Serialize)]
pub struct PhotoExportReceipt {
    pub path: PathBuf,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub bytes: usize,
}

fn valid_source_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn validate_crop(c: &PhotoCrop) -> Result<(), String> {
    if [c.left, c.top, c.right, c.bottom]
        .iter()
        .any(|&edge| edge > 9500)
        || u32::from(c.left) + u32::from(c.right) >= 10_000
        || u32::from(c.top) + u32::from(c.bottom) >= 10_000
    {
        return Err("The requested crop is invalid or removes the full photo.".into());
    }
    Ok(())
}

fn validate_request(r: &PhotoExportRequest) -> Result<(), String> {
    if !r.accept_pictures_export {
        return Err("Approve export to Pictures before any file is written.".into());
    }
    if !(25..=175).contains(&r.brightness)
        || !(25..=175).contains(&r.contrast)
        || r.saturation_percent > 200
        || !matches!(r.rotation, 0 | 90 | 180 | 270)
        || !(-150..=150).contains(&r.straighten_tenths)
        || !valid_source_hash(&r.source_sha256)
    {
        return Err("Photo edit settings or source digest are invalid.".into());
    }
    if let Some(crop) = &r.crop {
        validate_crop(crop)?;
    }
    if r.width == 0
        || r.height == 0
        || r.width > MAX_PNG_EDGE
        || r.height > MAX_PNG_EDGE
        || u64::from(r.width) * u64::from(r.height) > MAX_PNG_PIXELS
    {
        return Err("Export dimensions exceed the supported limit.".into());
    }
    if let Some(longest) = r.resize_long_edge {
        // PNG dimensions are verified against bytes below. Resize intent is
        // WebView-reported and must not contradict the output bitmap.
        if !(32..=MAX_PNG_EDGE).contains(&longest) || r.width.max(r.height) > longest {
            return Err("The requested output resize is outside the supported range.".into());
        }
    }
    // Prevent allocating an unbounded decoded payload from a forged IPC call.
    if r.png_base64.len() > MAX_PNG_BYTES.div_ceil(3) * 4 + 4 {
        return Err("The encoded PNG exceeds the export limit.".into());
    }
    Ok(())
}

fn decode_verified_png(r: &PhotoExportRequest) -> Result<Vec<u8>, String> {
    validate_request(r)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&r.png_base64)
        .map_err(|_| "The photo export is not valid base64.".to_string())?;
    if bytes.is_empty() || bytes.len() > MAX_PNG_BYTES {
        return Err("The rendered PNG exceeds the export limit.".into());
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    decoder.set_ignore_text_chunk(true);
    decoder.set_ignore_iccp_chunk(true);
    let mut reader = decoder
        .read_info()
        .map_err(|_| "The exported photo has an invalid PNG header.".to_string())?;
    let info = reader.info();
    if info.width != r.width || info.height != r.height {
        return Err("The image dimensions do not match the export request.".into());
    }
    let capacity = reader
        .output_buffer_size()
        .ok_or("The PNG exceeds the decoded memory limit.")?;
    if capacity > MAX_DECODE_BYTES {
        return Err("The PNG exceeds the decoded memory limit.".into());
    }
    let mut buffer = vec![0; capacity];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|_| "The PNG data is truncated or corrupted.".to_string())?;
    if frame.width != r.width || frame.height != r.height {
        return Err("The PNG frame size differs from its header.".into());
    }
    Ok(bytes)
}

#[cfg(windows)]
pub(crate) fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
pub(crate) fn is_reparse(_metadata: &fs::Metadata) -> bool {
    false
}

pub(crate) fn check_directory(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|_| "The Pictures export directory is unavailable.".to_string())?;
    if !meta.is_dir() || meta.file_type().is_symlink() || is_reparse(&meta) {
        return Err(
            "The Pictures export path is linked or redirected; no files were written.".into(),
        );
    }
    Ok(())
}

fn write_stage(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Cannot create a unique export staging file.".to_string())?;
    let saved = file.write_all(bytes).and_then(|_| file.sync_all());
    drop(file);
    if saved.is_err() {
        let _ = fs::remove_file(path);
        return Err("Could not finish the photo export staging write.".into());
    }
    Ok(())
}

pub fn export_to_pictures(
    pictures: &Path,
    request: PhotoExportRequest,
) -> Result<PhotoExportReceipt, String> {
    let bytes = decode_verified_png(&request)?; // fail before any filesystem writes
    check_directory(pictures)?;
    let dest_dir = pictures.join("Optic Rift");
    match fs::create_dir(&dest_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Could not create the Optic Rift Pictures folder.".into()),
    }
    check_directory(&dest_dir)?;
    let id = uuid::Uuid::new_v4();
    let stem = format!("optic-rift-photo-{id}");
    let png_path = dest_dir.join(format!("{stem}.png"));
    let receipt_path = dest_dir.join(format!("{stem}.provenance.json"));
    let stage_png = dest_dir.join(format!(".{stem}.partial.png"));
    let stage_receipt = dest_dir.join(format!(".{stem}.partial.json"));

    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let receipt = PhotoExportReceipt {
        path: png_path.clone(),
        sha256,
        width: request.width,
        height: request.height,
        bytes: bytes.len(),
    };
    let record = serde_json::json!({
        "record_version": 1,
        "kind": "adjusted-photo-export",
        "source_sha256_webview_reported": request.source_sha256,
        "adjustments": {
            "brightness_percent": request.brightness,
            "contrast_percent": request.contrast,
            "saturation_percent": request.saturation_percent,
            "rotation_degrees_clockwise": request.rotation,
            "straighten_tenths_degrees_clockwise": request.straighten_tenths,
            "resize_long_edge_requested_webview_reported": request.resize_long_edge,
            "crop_insets_basis_points_of_rotated_image": &request.crop,
            "flip_display_horizontal": request.flip_horizontal,
            "flip_display_vertical": request.flip_vertical
        },
        "output": {
            "sha256": &receipt.sha256,
            "width": receipt.width,
            "height": receipt.height,
            "bytes": receipt.bytes
        },
        "note": "Exported copy only; not a project-owned asset. Source hash and transform settings are WebView-reported, not independently verified against the immutable original."
    });
    let metadata = serde_json::to_vec_pretty(&record)
        .map_err(|_| "Could not serialize the photo export receipt.".to_string())?;
    if metadata.len() > 8192 {
        return Err("The photo export receipt exceeds its limit.".into());
    }

    let mut linked_png = false;
    let result = (|| -> Result<(), String> {
        write_stage(&stage_png, &bytes)?;
        write_stage(&stage_receipt, &metadata)?;
        // Hard-link publication is non-overwriting on both Windows NTFS and
        // other supported filesystems. No unverified replacement of user files.
        fs::hard_link(&stage_png, &png_path).map_err(|_| {
            "Could not publish a new photo without overwriting another file.".to_string()
        })?;
        linked_png = true;
        fs::hard_link(&stage_receipt, &receipt_path)
            .map_err(|_| "Could not publish the matching photo export receipt.".to_string())?;
        Ok(())
    })();
    if result.is_err() && linked_png {
        // Best-effort cleanup of our unique ID on a failed two-file commit.
        let _ = fs::remove_file(&png_path);
    }
    let _ = fs::remove_file(&stage_png);
    let _ = fs::remove_file(&stage_receipt);
    result?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PhotoExportRequest {
        let mut png = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png, 2, 1);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 10, 20, 40, 50, 60]).unwrap();
        }
        PhotoExportRequest {
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            source_sha256: "a".repeat(64),
            brightness: 100,
            contrast: 100,
            saturation_percent: 100,
            rotation: 0,
            straighten_tenths: 0,
            resize_long_edge: None,
            crop: None,
            flip_horizontal: false,
            flip_vertical: false,
            width: 2,
            height: 1,
            accept_pictures_export: true,
        }
    }

    #[test]
    fn native_export_denies_invalid_consent_settings_and_data() {
        let mut request = fixture();
        assert!(decode_verified_png(&request).is_ok());
        request.accept_pictures_export = false;
        assert!(decode_verified_png(&request)
            .unwrap_err()
            .contains("Approve export"));
        request = fixture();
        request.brightness = 200;
        assert!(decode_verified_png(&request).is_err());
        request = fixture();
        request.source_sha256 = "bad".into();
        assert!(decode_verified_png(&request).is_err());
        request = fixture();
        request.width = 5;
        assert!(decode_verified_png(&request).is_err());
        request = fixture();
        request.png_base64 = "not-a-png".into();
        assert!(decode_verified_png(&request).is_err());
        request = fixture();
        request.crop = Some(PhotoCrop {
            left: 9501,
            top: 0,
            right: 0,
            bottom: 0,
        });
        assert!(decode_verified_png(&request).unwrap_err().contains("crop"));
        request = fixture();
        request.crop = Some(PhotoCrop {
            left: 9500,
            top: 0,
            right: 500,
            bottom: 0,
        });
        assert!(decode_verified_png(&request).unwrap_err().contains("crop"));
        request = fixture();
        request.crop = Some(PhotoCrop {
            left: 1000,
            top: 2500,
            right: 0,
            bottom: 0,
        });
        assert!(decode_verified_png(&request).is_ok());
    }

    #[test]
    fn export_creates_unique_pair_and_never_modifies_source() {
        let root = std::env::temp_dir().join(format!("optic-photo-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let original = root.join("original.png");
        let original_bytes = decode_verified_png(&fixture()).unwrap();
        fs::write(&original, &original_bytes).unwrap();
        let first = export_to_pictures(&root, fixture()).unwrap();
        let mut flipped = fixture();
        flipped.saturation_percent = 75;
        flipped.straighten_tenths = -75;
        flipped.resize_long_edge = Some(128);
        flipped.flip_horizontal = true;
        flipped.flip_vertical = true;
        let second = export_to_pictures(&root, flipped).unwrap();
        let receipt_json: serde_json::Value = serde_json::from_slice(
            &fs::read(second.path.with_extension("provenance.json")).unwrap()
        ).unwrap();
        assert_eq!(receipt_json["adjustments"]["flip_display_horizontal"].as_bool(), Some(true));
        assert_eq!(receipt_json["adjustments"]["flip_display_vertical"].as_bool(), Some(true));
        assert_eq!(receipt_json["adjustments"]["saturation_percent"].as_u64(), Some(75));
        assert_eq!(receipt_json["adjustments"]["straighten_tenths_degrees_clockwise"].as_i64(), Some(-75));
        assert_eq!(receipt_json["adjustments"]["resize_long_edge_requested_webview_reported"].as_u64(), Some(128));
        assert_ne!(first.path, second.path);
        assert_eq!(fs::read(&original).unwrap(), original_bytes);
        assert_eq!(fs::read(&first.path).unwrap(), original_bytes);
        assert!(first.path.with_extension("provenance.json").is_file());
        assert!(second.path.is_file());
        let mut bad = fixture();
        bad.accept_pictures_export = false;
        assert!(export_to_pictures(&root, bad).is_err());
        let entries = fs::read_dir(root.join("Optic Rift")).unwrap().count();
        assert_eq!(entries, 4);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refusals_happen_before_any_file_is_written() {
        let root =
            std::env::temp_dir().join(format!("optic-photo-refuse-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let good = fixture();
        let png = base64::engine::general_purpose::STANDARD
            .decode(&good.png_base64)
            .unwrap();
        let mut cases: Vec<(&str, PhotoExportRequest)> = Vec::new();
        let mut r = fixture();
        r.accept_pictures_export = false;
        cases.push(("no consent", r));
        let mut r = fixture();
        r.rotation = 45;
        cases.push(("rotation", r));
        let mut r = fixture();
        r.contrast = 24;
        cases.push(("contrast", r));
        let mut r = fixture();
        r.source_sha256 = "A".repeat(64);
        cases.push(("uppercase hash", r));
        let mut r = fixture();
        r.width = 9000;
        r.height = 1;
        cases.push(("edge limit", r));
        let mut r = fixture();
        r.width = 5000;
        r.height = 5000;
        cases.push(("pixel limit", r));
        let mut r = fixture();
        r.png_base64 = "A".repeat(MAX_PNG_BYTES.div_ceil(3) * 4 + 8);
        cases.push(("huge base64", r));
        let mut r = fixture();
        r.png_base64 = base64::engine::general_purpose::STANDARD.encode(&png[..png.len() - 20]);
        cases.push(("truncated pixel data", r));
        let mut r = fixture();
        r.png_base64 = base64::engine::general_purpose::STANDARD.encode(b"GIF89a not a png");
        cases.push(("not png", r));
        for (label, request) in cases {
            assert!(export_to_pictures(&root, request).is_err(), "{label}");
        }
        assert!(
            !root.join("Optic Rift").exists(),
            "no folder or file before validation passes"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resize_defaults_to_original_and_refuses_mismatched_or_forged_dimensions() {
        let payload = serde_json::json!({
            "png_base64": "", "source_sha256": "a".repeat(64),
            "brightness": 100, "contrast": 100, "rotation": 0,
            "width": 1, "height": 1, "accept_pictures_export": true
        });
        let old: PhotoExportRequest = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(old.resize_long_edge, None);
        let mut forged = payload.clone();
        forged["resize_long_edge"] = serde_json::json!("512");
        assert!(serde_json::from_value::<PhotoExportRequest>(forged).is_err());
        let mut invalid = fixture();
        invalid.resize_long_edge = Some(31);
        assert!(decode_verified_png(&invalid).is_err());
        invalid.resize_long_edge = Some(8193);
        assert!(decode_verified_png(&invalid).is_err());
        invalid.resize_long_edge = Some(32);
        assert!(decode_verified_png(&invalid).is_ok());
        // Claimed output must fit requested longest edge.
        invalid.resize_long_edge = Some(32);
        invalid.width = 64;
        assert!(decode_verified_png(&invalid).is_err());
    }

    #[test]
    fn straighten_defaults_neutral_and_refuses_out_of_range_or_forged_values() {
        let payload = serde_json::json!({
            "png_base64": "", "source_sha256": "a".repeat(64),
            "brightness": 100, "contrast": 100, "rotation": 0,
            "width": 1, "height": 1, "accept_pictures_export": true
        });
        let previous: PhotoExportRequest = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(previous.straighten_tenths, 0);
        let mut forged = payload;
        forged["straighten_tenths"] = serde_json::json!("5");
        assert!(serde_json::from_value::<PhotoExportRequest>(forged).is_err());
        for value in [-151, -150, 0, 150, 151] {
            let mut request = fixture();
            request.straighten_tenths = value;
            assert_eq!(decode_verified_png(&request).is_ok(), (-150..=150).contains(&value));
        }
    }

    #[test]
    fn saturation_defaults_neutral_and_refuses_invalid_input() {
        let payload = serde_json::json!({
            "png_base64": "",
            "source_sha256": "a".repeat(64),
            "brightness": 100, "contrast": 100, "rotation": 0,
            "width": 1, "height": 1, "accept_pictures_export": true
        });
        let old: PhotoExportRequest = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(old.saturation_percent, 100);
        let mut invalid = payload.clone();
        invalid["saturation_percent"] = serde_json::json!("200");
        assert!(serde_json::from_value::<PhotoExportRequest>(invalid).is_err());
        let mut invalid_range = fixture();
        invalid_range.saturation_percent = 201;
        assert!(decode_verified_png(&invalid_range).is_err());
        let mut valid = fixture();
        valid.saturation_percent = 0;
        assert!(decode_verified_png(&valid).is_ok());
        valid.saturation_percent = 200;
        assert!(decode_verified_png(&valid).is_ok());
    }

    #[test]
    fn flip_fields_default_false_and_refuse_wrong_type() {
        let payload = serde_json::json!({
            "png_base64": "",
            "source_sha256": "a".repeat(64),
            "brightness": 100, "contrast": 100, "rotation": 0,
            "width": 1, "height": 1, "accept_pictures_export": true
        });
        let old: PhotoExportRequest = serde_json::from_value(payload.clone()).unwrap();
        assert!(!old.flip_horizontal && !old.flip_vertical);
        let mut invalid = payload.clone();
        invalid["flip_horizontal"] = serde_json::json!("true");
        assert!(serde_json::from_value::<PhotoExportRequest>(invalid).is_err());
        let mut valid = payload;
        valid["flip_horizontal"] = serde_json::json!(true);
        valid["flip_vertical"] = serde_json::json!(false);
        let request: PhotoExportRequest = serde_json::from_value(valid).unwrap();
        assert!(request.flip_horizontal && !request.flip_vertical);
    }

    #[test]
    fn unknown_request_fields_are_rejected() {
        let json = serde_json::json!({
            "png_base64": "", "source_sha256": "a".repeat(64), "brightness": 100, "contrast": 100,
            "rotation": 0, "width": 1, "height": 1, "accept_pictures_export": true,
            "path": "C:\\Windows\\x.png"
        });
        assert!(serde_json::from_value::<PhotoExportRequest>(json).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn junctioned_optic_rift_folder_is_refused_without_writing() {
        let root =
            std::env::temp_dir().join(format!("optic-photo-junction-{}", uuid::Uuid::new_v4()));
        let elsewhere =
            std::env::temp_dir().join(format!("optic-photo-target-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        fs::create_dir(&elsewhere).unwrap();
        // Directory junctions need no elevation, unlike symbolic links.
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.join("Optic Rift"))
            .arg(&elsewhere)
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink /J failed: {status:?}");
        assert!(export_to_pictures(&root, fixture())
            .unwrap_err()
            .contains("redirected"));
        assert_eq!(
            fs::read_dir(&elsewhere).unwrap().count(),
            0,
            "nothing written through the junction"
        );
        // The Pictures root itself being a junction is refused too.
        let linked_root =
            std::env::temp_dir().join(format!("optic-photo-root-{}", uuid::Uuid::new_v4()));
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&linked_root)
            .arg(&elsewhere)
            .output()
            .unwrap();
        assert!(status.status.success());
        assert!(export_to_pictures(&linked_root, fixture()).is_err());
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
        fs::remove_dir(root.join("Optic Rift")).unwrap();
        fs::remove_dir(&linked_root).unwrap();
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(elsewhere).unwrap();
    }

    #[test]
    fn export_refuses_redirected_destination_without_writing() {
        let root =
            std::env::temp_dir().join(format!("optic-photo-invalid-{}", uuid::Uuid::new_v4()));
        assert!(export_to_pictures(&root, fixture()).is_err());
        assert!(!root.exists());
    }
}
