//! Consent-gated native video trim/export (issue #36). The first executable native video
//! export: re-encode the frames between Video Studio's in/out markers into a new MP4.
//!
//! Trust boundary:
//! - The WebView never supplies a path. A source is granted only after the user picks it in the
//!   native Windows file dialog *and* it matches the clip open in Video Studio (name, size,
//!   modified time). The grant pins a SHA-256 that is checked again before and after encoding.
//! - FFmpeg/ffprobe come from `media::Tools`. Their SHA-256 is pinned on first use in a session
//!   and verified before every job, so a substituted binary is refused.
//! - The WebView's `currentTime` is only a hint. In/out points are resolved natively to real
//!   decoded frames (presentation order, VFR-faithful) and the plan the user confirmed is
//!   checked again when the job starts.
//! - One job at a time, in a Windows Job Object, with a wall-clock limit, a stall limit and an
//!   output-size limit. Cancel ends the process tree and removes staging before it reports.
//! - Output is staged in app data, decoded back and validated, then published into
//!   Pictures/Optic Rift with a non-overwriting hard link next to a provenance receipt.
//!   A journal lets a restart mark interrupted jobs and delete their partial files.
//!
//! Not in scope: timelines, audio editing, HDR, project asset records (issue #5).

use crate::media::{self, FrameStamp, ProbeResult, Tools};
use crate::photo_export::{check_directory, is_reparse};
use crate::process_tree::ProcessTree;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// Frame enumeration bound inherited from `media`. Longer clips are refused, not truncated.
pub const MAX_CLIP_FRAMES: usize = media::MAX_FRAMES_PER_REQUEST;
const ALLOWED_EXTENSIONS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv"];
const JOB_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const STALL_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_JOURNAL_ENTRIES: usize = 50;
/// Mean absolute luma difference (0..255) allowed between a source frame and the matching
/// re-encoded output frame, both downscaled to 64x36. Lossy re-encode, not bitwise identity.
const FRAME_MATCH_TOLERANCE: f64 = 6.0;
const TIME_EPSILON: f64 = 0.0005;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|_| "Could not read the file to verify it.".to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| "Could not read the file to verify it.".to_string())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn is_uuid(text: &str) -> bool {
    uuid::Uuid::parse_str(text).is_ok() && text.len() == 36
}

// ---------------------------------------------------------------------------------------------
// Links and redirected paths
// ---------------------------------------------------------------------------------------------

/// Windows cloud-file placeholders (OneDrive) are reparse points. A fully local one is an
/// ordinary file for our purposes; symlinks, junctions, mount points and anything offline or
/// recall-on-access are refused.
#[cfg(windows)]
fn reparse_is_acceptable(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{FindClose, FindFirstFileW, WIN32_FIND_DATAW};
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
    const IO_REPARSE_TAG_CLOUD_MASK: u32 = 0x0000_F000;
    const IO_REPARSE_TAG_CLOUD: u32 = 0x9000_001A;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: valid NUL-terminated path and an owned find-data buffer; the handle is closed.
    unsafe {
        let mut data: WIN32_FIND_DATAW = std::mem::zeroed();
        let handle = FindFirstFileW(wide.as_ptr(), &mut data);
        if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
            return false;
        }
        FindClose(handle);
        let tag = data.dwReserved0 & !IO_REPARSE_TAG_CLOUD_MASK;
        let offline = data.dwFileAttributes
            & (FILE_ATTRIBUTE_OFFLINE
                | FILE_ATTRIBUTE_RECALL_ON_OPEN
                | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
            != 0;
        tag == IO_REPARSE_TAG_CLOUD && !offline
    }
}
#[cfg(not(windows))]
fn reparse_is_acceptable(_path: &Path) -> bool {
    false
}

/// Refuse a path if it, or any ancestor below the volume root, is a symlink, junction or other
/// redirecting reparse point.
pub(crate) fn path_is_linked(path: &Path) -> bool {
    for ancestor in path.ancestors() {
        if ancestor.parent().is_none() || ancestor.as_os_str().is_empty() {
            break; // volume root
        }
        let Ok(meta) = fs::symlink_metadata(ancestor) else {
            return true;
        };
        if meta.file_type().is_symlink() {
            return true;
        }
        if is_reparse(&meta) && !reparse_is_acceptable(ancestor) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------
// Trusted tools
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolIdentity {
    /// Where the binaries came from. A PATH install is a developer setup, not a packaged
    /// trust boundary; bundling and licensing an FFmpeg build is still open.
    pub source: String,
    pub ffmpeg_version: String,
    pub ffmpeg_sha256: String,
    pub ffprobe_sha256: String,
}

#[derive(Clone)]
pub struct TrustedTools {
    pub tools: Tools,
    pub identity: ToolIdentity,
}

fn pinned_tools() -> &'static Mutex<Option<TrustedTools>> {
    static PINNED: OnceLock<Mutex<Option<TrustedTools>>> = OnceLock::new();
    PINNED.get_or_init(|| Mutex::new(None))
}

fn identify_tools(tools: Tools, source: &str) -> Result<TrustedTools, String> {
    for exe in [&tools.ffmpeg, &tools.ffprobe] {
        if path_is_linked(exe) {
            return Err(
                "FFmpeg is reached through a link or redirected folder; export refused.".into(),
            );
        }
    }
    let version = media::run_bounded(
        &tools.ffmpeg,
        &media::os(&["-hide_banner", "-version"]),
        Duration::from_secs(20),
    )
    .map_err(|e| format!("FFmpeg could not start: {e}"))?;
    let version = String::from_utf8_lossy(&version)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .take(120)
        .collect::<String>();
    if !version.starts_with("ffmpeg version") {
        return Err("The FFmpeg tool did not identify itself; export refused.".into());
    }
    let encoders = media::run_bounded(
        &tools.ffmpeg,
        &media::os(&["-hide_banner", "-encoders"]),
        Duration::from_secs(20),
    )
    .map_err(|e| format!("FFmpeg could not list encoders: {e}"))?;
    let encoders = String::from_utf8_lossy(&encoders);
    let has = |name: &str| {
        encoders
            .lines()
            .any(|l| l.split_whitespace().nth(1) == Some(name))
    };
    if !has("libx264") || !has("aac") {
        return Err(
            "This FFmpeg build has no libx264 or AAC encoder, which trimming needs.".into(),
        );
    }
    Ok(TrustedTools {
        identity: ToolIdentity {
            source: source.into(),
            ffmpeg_version: version,
            ffmpeg_sha256: sha256_file(&tools.ffmpeg)?,
            ffprobe_sha256: sha256_file(&tools.ffprobe)?,
        },
        tools,
    })
}

/// Discover FFmpeg once per session (bundled resource folder first), then on every later call
/// verify that the same binaries are still there, byte for byte.
pub fn trusted_tools(resource_dir: Option<&Path>) -> Result<TrustedTools, String> {
    let mut pinned = pinned_tools().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(trusted) = pinned.as_ref() {
        let ffmpeg = sha256_file(&trusted.tools.ffmpeg)?;
        let ffprobe = sha256_file(&trusted.tools.ffprobe)?;
        if ffmpeg != trusted.identity.ffmpeg_sha256 || ffprobe != trusted.identity.ffprobe_sha256 {
            return Err(
                "FFmpeg changed on disk since this session started; export refused. Restart Optic Rift to trust the new build."
                    .into(),
            );
        }
        return Ok(trusted.clone());
    }
    let exe = |name: &str| format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let bundled = resource_dir
        .map(|dir| dir.join("ffmpeg"))
        .filter(|dir| dir.join(exe("ffmpeg")).is_file() && dir.join(exe("ffprobe")).is_file());
    let (tools, source) = match bundled {
        Some(dir) => (
            Tools {
                ffmpeg: dir.join(exe("ffmpeg")),
                ffprobe: dir.join(exe("ffprobe")),
            },
            "bundled with Optic Rift",
        ),
        None => (
            Tools::discover().map_err(|_| {
                "FFmpeg is not installed for Optic Rift, so videos cannot be trimmed yet."
                    .to_string()
            })?,
            if std::env::var_os("OPTIC_RIFT_FFMPEG_DIR").is_some() {
                "OPTIC_RIFT_FFMPEG_DIR (developer setting)"
            } else {
                "PATH (developer setup, not a packaged build)"
            },
        ),
    };
    let trusted = identify_tools(tools, source)?;
    *pinned = Some(trusted.clone());
    Ok(trusted)
}

// ---------------------------------------------------------------------------------------------
// Source grants
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GrantRequest {
    /// The Video Studio session asset this grant is for.
    pub asset_id: String,
    /// What the WebView knows about that asset's `File`: name, byte size, modified time.
    pub name: String,
    pub size: u64,
    pub last_modified_ms: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AudioPolicy {
    /// The source has no audio track; the export has none either.
    None,
    /// Exactly one audio track: re-encoded to AAC over the same span.
    KeepAac,
}

#[derive(Debug, Clone)]
struct Grant {
    id: String,
    asset_id: String,
    path: PathBuf,
    name: String,
    size: u64,
    modified_ms: u64,
    sha256: String,
    probe: ProbeResult,
    frames: Vec<FrameStamp>,
    audio: AudioPolicy,
    display: (u32, u32),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantInfo {
    pub grant_id: String,
    pub asset_id: String,
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    pub rotation_degrees: i32,
    pub codec: String,
    pub frame_count: usize,
    pub duration_seconds: f64,
    pub audio: AudioPolicy,
    pub tool: ToolIdentity,
}

fn grants() -> &'static Mutex<HashMap<String, Grant>> {
    static GRANTS: OnceLock<Mutex<HashMap<String, Grant>>> = OnceLock::new();
    GRANTS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn modified_ms(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn validate_grant_request(request: &GrantRequest) -> Result<(), String> {
    let bad_name = request.name.is_empty()
        || request.name.chars().count() > 255
        || request.name.contains(['/', '\\', ':', '\0'])
        || request.name == "."
        || request.name == "..";
    if !is_uuid(&request.asset_id) || bad_name || request.size == 0 {
        return Err("The video export request is invalid.".into());
    }
    if request.size > MAX_SOURCE_BYTES {
        return Err("Videos larger than 8 GB can't be trimmed in this version.".into());
    }
    Ok(())
}

fn extension_allowed(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .is_some_and(|e| ALLOWED_EXTENSIONS.contains(&e.as_str()))
}

fn display_size(probe: &ProbeResult) -> Result<(u32, u32, i32), String> {
    let video = probe
        .video
        .as_ref()
        .ok_or("This file has no video stream.")?;
    let rotation = video.rotation_degrees.rem_euclid(360);
    if rotation % 90 != 0 {
        return Err("This video has an unusual rotation that can't be exported yet.".into());
    }
    let (w, h) = if rotation % 180 == 90 {
        (video.height, video.width)
    } else {
        (video.width, video.height)
    };
    Ok((w, h, rotation))
}

/// Everything about a picked file that must hold before a grant exists.
fn admit_source(
    trusted: &TrustedTools,
    request: &GrantRequest,
    picked: &Path,
) -> Result<Grant, String> {
    validate_grant_request(request)?;
    let meta = fs::symlink_metadata(picked)
        .map_err(|_| "The chosen file is not available.".to_string())?;
    if !meta.is_file() || path_is_linked(picked) {
        return Err(
            "Choose a regular video file, not a link or shortcut. Nothing was granted.".into(),
        );
    }
    if !extension_allowed(picked) {
        return Err("Choose an MP4, MOV, M4V, WebM or MKV video. Nothing was granted.".into());
    }
    let picked_name = picked
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let modified = modified_ms(&meta);
    if picked_name != request.name
        || meta.len() != request.size
        || modified.abs_diff(request.last_modified_ms) > 2
    {
        return Err(
            "The file you chose is not the clip open in Video Studio (its name, size or modified time differ). Nothing was granted."
                .into(),
        );
    }
    let sha256 = sha256_file(picked)?;
    let probe = media::probe(&trusted.tools, picked)
        .map_err(|e| format!("This video can't be read for export: {e}"))?;
    let video = probe
        .video
        .clone()
        .ok_or("This file has no video stream.")?;
    // No color management yet: HDR transfer curves and BT.2020 wide-gamut primaries would be
    // re-encoded as if they were ordinary SDR video and change color.
    if matches!(
        video.color_transfer.as_deref(),
        Some("smpte2084") | Some("arib-std-b67")
    ) || video.color_primaries.as_deref() == Some("bt2020")
    {
        return Err(
            "HDR or wide-gamut (BT.2020) video can't be trimmed yet; the export would change its colors. Nothing was granted."
                .into(),
        );
    }
    let (w, h, _) = display_size(&probe)?;
    if w % 2 == 1 || h % 2 == 1 {
        return Err(format!(
            "This {w}×{h} video has an odd width or height, which the MP4 export can't keep exactly yet."
        ));
    }
    let audio = match probe.audio_streams {
        0 => AudioPolicy::None,
        1 => AudioPolicy::KeepAac,
        n => {
            return Err(format!(
                "This video has {n} audio tracks. This version keeps exactly one track and will not drop the others silently."
            ))
        }
    };
    let frames = media::enumerate_frames(&trusted.tools, picked, MAX_CLIP_FRAMES)
        .map_err(|e| format!("The video's frames could not be listed: {e}"))?;
    if frames.len() >= MAX_CLIP_FRAMES {
        return Err(format!(
            "Clips with {MAX_CLIP_FRAMES} or more frames can't be trimmed in this version."
        ));
    }
    if frames.len() < 2 {
        return Err("This video has fewer than two decodable frames.".into());
    }
    Ok(Grant {
        id: uuid::Uuid::new_v4().to_string(),
        asset_id: request.asset_id.clone(),
        path: picked.to_path_buf(),
        name: picked_name.to_string(),
        size: meta.len(),
        modified_ms: modified,
        sha256,
        probe,
        frames,
        audio,
        display: (w, h),
    })
}

fn grant_info(grant: &Grant, tool: &ToolIdentity) -> GrantInfo {
    let video = grant
        .probe
        .video
        .as_ref()
        .expect("admitted grants have video");
    let first = grant.frames[0].seconds();
    let last = grant.frames[grant.frames.len() - 1];
    GrantInfo {
        grant_id: grant.id.clone(),
        asset_id: grant.asset_id.clone(),
        file_name: grant.name.clone(),
        bytes: grant.size,
        sha256: grant.sha256.clone(),
        width: grant.display.0,
        height: grant.display.1,
        rotation_degrees: video.rotation_degrees,
        codec: video.codec.clone(),
        frame_count: grant.frames.len(),
        duration_seconds: last.seconds() - first
            + frame_duration(&grant.frames, grant.frames.len() - 1, &grant.probe),
        audio: grant.audio,
        tool: tool.clone(),
    }
}

/// Register a grant for a file the user picked. `picked` must come from the native dialog.
pub fn grant_picked(
    trusted: &TrustedTools,
    request: &GrantRequest,
    picked: &Path,
) -> Result<GrantInfo, String> {
    let grant = admit_source(trusted, request, picked)?;
    let info = grant_info(&grant, &trusted.identity);
    let mut all = grants().lock().unwrap_or_else(|p| p.into_inner());
    // One live grant per session asset: a new pick replaces the old one.
    all.retain(|_, g| g.asset_id != grant.asset_id);
    all.insert(grant.id.clone(), grant);
    Ok(info)
}

pub fn revoke(grant_id: &str) -> bool {
    grants()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(grant_id)
        .is_some()
}

fn live_grant(grant_id: &str, asset_id: &str) -> Result<Grant, String> {
    let all = grants().lock().unwrap_or_else(|p| p.into_inner());
    let grant = all
        .get(grant_id)
        .filter(|g| g.asset_id == asset_id)
        .cloned()
        .ok_or(
            "This video is not authorized for export. Choose it again with Save trimmed video.",
        )?;
    Ok(grant)
}

/// The grant is only usable while the file is the same one the user picked.
fn source_unchanged(grant: &Grant, check_hash: bool) -> Result<(), String> {
    let changed = "The source video changed on disk after you chose it. Nothing was exported; choose it again.";
    let meta = fs::symlink_metadata(&grant.path).map_err(|_| changed.to_string())?;
    if !meta.is_file()
        || path_is_linked(&grant.path)
        || meta.len() != grant.size
        || modified_ms(&meta) != grant.modified_ms
    {
        return Err(changed.into());
    }
    if check_hash && sha256_file(&grant.path)? != grant.sha256 {
        return Err(changed.into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Frame resolution
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TrimRequest {
    pub grant_id: String,
    pub asset_id: String,
    /// WebView `currentTime` hints, in seconds from the start of the clip.
    pub in_seconds: f64,
    /// `None` means "to the end of the clip".
    pub out_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FrameRef {
    pub index: usize,
    pub pts: i64,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TrimPlan {
    pub grant_id: String,
    pub source_name: String,
    pub first: FrameRef,
    pub last: FrameRef,
    pub frame_count: usize,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub audio: AudioPolicy,
}

fn frame_duration(frames: &[FrameStamp], index: usize, probe: &ProbeResult) -> f64 {
    if let Some(next) = frames.get(index + 1) {
        return next.seconds() - frames[index].seconds();
    }
    if let Some(rate) = probe.video.as_ref().and_then(|v| v.average_frame_rate) {
        if rate.num > 0 {
            return rate.den as f64 / rate.num as f64;
        }
    }
    if index > 0 {
        return frames[index].seconds() - frames[index - 1].seconds();
    }
    0.0
}

/// The frame shown at `hint` seconds: the last frame whose presentation time is at or before
/// it (the WebView's clock starts at the first frame).
fn frame_at(frames: &[FrameStamp], hint: f64) -> usize {
    let origin = frames[0].seconds();
    let target = origin + hint + TIME_EPSILON;
    frames
        .iter()
        .rposition(|f| f.seconds() <= target)
        .unwrap_or(0)
}

fn plan_for(grant: &Grant, request: &TrimRequest) -> Result<TrimPlan, String> {
    let duration = grant.frames[grant.frames.len() - 1].seconds() - grant.frames[0].seconds()
        + frame_duration(&grant.frames, grant.frames.len() - 1, &grant.probe);
    let valid = |t: f64| t.is_finite() && t >= 0.0 && t <= duration + 1.0;
    if !valid(request.in_seconds) || request.out_seconds.is_some_and(|t| !valid(t)) {
        return Err("The in or out point is outside this clip.".into());
    }
    if request
        .out_seconds
        .is_some_and(|out| out <= request.in_seconds)
    {
        return Err("Set the out point after the in point.".into());
    }
    let start = frame_at(&grant.frames, request.in_seconds);
    let end = match request.out_seconds {
        Some(out) => frame_at(&grant.frames, out),
        None => grant.frames.len() - 1,
    };
    if end <= start {
        return Err("The in and out points select the same frame. Choose a longer range.".into());
    }
    let to_ref = |i: usize| FrameRef {
        index: i,
        pts: grant.frames[i].pts,
        seconds: grant.frames[i].seconds() - grant.frames[0].seconds(),
    };
    let first = to_ref(start);
    let last = to_ref(end);
    Ok(TrimPlan {
        grant_id: grant.id.clone(),
        source_name: grant.name.clone(),
        first,
        last,
        frame_count: end - start + 1,
        duration_seconds: last.seconds - first.seconds
            + frame_duration(&grant.frames, end, &grant.probe),
        width: grant.display.0,
        height: grant.display.1,
        audio: grant.audio,
    })
}

pub fn resolve(request: &TrimRequest) -> Result<TrimPlan, String> {
    let grant = live_grant(&request.grant_id, &request.asset_id)?;
    source_unchanged(&grant, false)?;
    plan_for(&grant, request)
}

// ---------------------------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartRequest {
    pub grant_id: String,
    pub asset_id: String,
    pub in_seconds: f64,
    pub out_seconds: Option<f64>,
    /// The plan the user confirmed. The job refuses to start if it no longer resolves the same.
    pub expected_first_index: usize,
    pub expected_last_index: usize,
    pub accept_pictures_export: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JobState {
    Encoding,
    Validating,
    Publishing,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
    /// The app stopped while this job ran. Its partial files were removed on the next start.
    Interrupted,
}

impl JobState {
    fn is_final(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Interrupted
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportedVideo {
    pub file_name: String,
    pub sha256: String,
    pub bytes: u64,
    pub width: u32,
    pub height: u32,
    pub frame_count: usize,
    pub duration_seconds: f64,
    pub has_audio: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job_id: String,
    pub state: JobState,
    pub source_name: String,
    pub frames_done: usize,
    pub frame_count: usize,
    pub message: String,
    pub output: Option<ExportedVideo>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: Option<u64>,
}

/// On-disk journal entry: what a restart needs to clean up.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Journal {
    status: JobStatus,
    staging_dir: PathBuf,
    /// Files created inside the destination folder before the final link.
    dest_partials: Vec<PathBuf>,
    /// Final published files (video, receipt) once linked.
    published: Vec<PathBuf>,
}

struct Job {
    journal: Mutex<Journal>,
    cancel: AtomicBool,
    journal_path: PathBuf,
}

impl Job {
    fn status(&self) -> JobStatus {
        self.journal
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .status
            .clone()
    }

    fn update(&self, change: impl FnOnce(&mut Journal)) {
        let mut journal = self.journal.lock().unwrap_or_else(|p| p.into_inner());
        change(&mut journal);
        if journal.status.state.is_final() && journal.status.finished_unix_ms.is_none() {
            journal.status.finished_unix_ms = Some(now_ms());
        }
        let _ = write_journal(&self.journal_path, &journal);
    }
}

fn jobs() -> &'static Mutex<Vec<Arc<Job>>> {
    static JOBS: OnceLock<Mutex<Vec<Arc<Job>>>> = OnceLock::new();
    JOBS.get_or_init(|| Mutex::new(Vec::new()))
}

fn write_journal(path: &Path, journal: &Journal) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(journal).map_err(std::io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    {
        let mut file = File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)
}

/// Directories under app data that this module owns.
pub struct ExportRoots {
    pub app_data: PathBuf,
    pub pictures: PathBuf,
    pub resource_dir: Option<PathBuf>,
}

impl ExportRoots {
    fn base(&self) -> PathBuf {
        self.app_data.join("video-exports")
    }
    fn journal_dir(&self) -> PathBuf {
        self.base().join("jobs")
    }
    fn staging_root(&self) -> PathBuf {
        self.base().join("staging")
    }
}

fn ensure_owned_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|_| "Could not prepare the export work folder.".to_string())?;
    if path_is_linked(path) {
        return Err("The export work folder is linked or redirected; nothing was written.".into());
    }
    Ok(())
}

pub fn start(roots: &ExportRoots, request: StartRequest) -> Result<JobStatus, String> {
    if !request.accept_pictures_export {
        return Err("Approve saving to Pictures before any file is written.".into());
    }
    // One writer: the running-job check and the registration of the new job happen under one
    // lock, so two simultaneous requests can never both start.
    static STARTING: Mutex<()> = Mutex::new(());
    let _starting = STARTING.lock().unwrap_or_else(|p| p.into_inner());
    {
        let all = jobs().lock().unwrap_or_else(|p| p.into_inner());
        if all.iter().any(|job| !job.status().state.is_final()) {
            return Err("Another video export is running. Wait for it or cancel it first.".into());
        }
    }
    let trusted = trusted_tools(roots.resource_dir.as_deref())?;
    let grant = live_grant(&request.grant_id, &request.asset_id)?;
    let plan = plan_for(
        &grant,
        &TrimRequest {
            grant_id: request.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: request.in_seconds,
            out_seconds: request.out_seconds,
        },
    )?;
    if plan.first.index != request.expected_first_index
        || plan.last.index != request.expected_last_index
    {
        return Err(
            "The trim range changed since you confirmed it. Review it and try again.".into(),
        );
    }
    source_unchanged(&grant, true)?;
    check_directory(&roots.pictures)?;

    let job_id = uuid::Uuid::new_v4().to_string();
    ensure_owned_dir(&roots.journal_dir())?;
    ensure_owned_dir(&roots.staging_root())?;
    let staging_dir = roots.staging_root().join(&job_id);
    fs::create_dir(&staging_dir)
        .map_err(|_| "Could not create the export work folder.".to_string())?;
    let status = JobStatus {
        job_id: job_id.clone(),
        state: JobState::Encoding,
        source_name: grant.name.clone(),
        frames_done: 0,
        frame_count: plan.frame_count,
        message: "Encoding the trimmed video…".into(),
        output: None,
        started_unix_ms: now_ms(),
        finished_unix_ms: None,
    };
    let job = Arc::new(Job {
        journal: Mutex::new(Journal {
            status: status.clone(),
            staging_dir: staging_dir.clone(),
            dest_partials: Vec::new(),
            published: Vec::new(),
        }),
        cancel: AtomicBool::new(false),
        journal_path: roots.journal_dir().join(format!("{job_id}.json")),
    });
    job.update(|_| {});
    {
        let mut all = jobs().lock().unwrap_or_else(|p| p.into_inner());
        all.push(job.clone());
        let excess = all.len().saturating_sub(MAX_JOURNAL_ENTRIES);
        all.drain(..excess);
    }
    let pictures = roots.pictures.clone();
    thread::spawn(move || run_job(job, trusted, grant, plan, pictures));
    Ok(status)
}

pub fn status(job_id: &str) -> Option<JobStatus> {
    find_job(job_id).map(|job| job.status())
}

fn find_job(job_id: &str) -> Option<Arc<Job>> {
    jobs()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find(|job| job.status().job_id == job_id)
        .cloned()
}

/// Ask a running job to stop. It reports `cancelled` only after the process tree has ended and
/// its files are gone.
pub fn cancel(job_id: &str) -> Result<JobStatus, String> {
    let job = find_job(job_id).ok_or("No such video export.")?;
    job.cancel.store(true, Ordering::SeqCst);
    job.update(|j| {
        if !j.status.state.is_final() {
            j.status.state = JobState::Cancelling;
            j.status.message = "Stopping the export…".into();
        }
    });
    Ok(job.status())
}

pub fn list() -> Vec<JobStatus> {
    jobs()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .map(|job| job.status())
        .collect()
}

/// The published file of a finished job, for "Show in folder". Never a WebView-supplied path.
pub fn published_video(job_id: &str) -> Option<PathBuf> {
    let job = find_job(job_id)?;
    let journal = job.journal.lock().unwrap_or_else(|p| p.into_inner());
    (journal.status.state == JobState::Succeeded)
        .then(|| journal.published.first().cloned())
        .flatten()
}

// ---------------------------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------------------------

fn micros(frame: FrameStamp) -> i64 {
    let scaled = (frame.pts as i128) * (frame.time_base.num as i128) * 1_000_000;
    scaled.div_euclid(frame.time_base.den as i128) as i64
}

fn encode_args(grant: &Grant, plan: &TrimPlan, output: &Path) -> Vec<OsString> {
    let first = grant.frames[plan.first.index];
    let mut args: Vec<OsString> = media::os(&["-hide_banner", "-v", "error", "-nostdin", "-y"]);
    // Input-side accurate seek: decode from the preceding keyframe and drop everything before
    // the first chosen frame. Never stream copy, so non-keyframe cuts are exact.
    args.push("-ss".into());
    args.push(format!("{}us", micros(first)).into());
    args.push("-i".into());
    args.push(grant.path.clone().into());
    args.extend(media::os(&["-map", "0:v:0"]));
    if grant.audio == AudioPolicy::KeepAac {
        args.extend(media::os(&["-map", "0:a:0"]));
    }
    args.extend(media::os(&["-map_metadata", "-1", "-map_chapters", "-1"]));
    args.push("-frames:v".into());
    args.push(plan.frame_count.to_string().into());
    // Keep the audio span equal to the video span.
    args.push("-t".into());
    args.push(format!("{:.6}", plan.duration_seconds).into());
    // Keep the source's real frame timing (variable frame rate stays variable).
    args.extend(media::os(&[
        "-fps_mode",
        "passthrough",
        "-c:v",
        "libx264",
        "-preset",
        "medium",
        "-crf",
        "16",
        "-pix_fmt",
        "yuv420p",
        "-threads",
        "0",
    ]));
    // Carry the source's color description so players show the same colors.
    let video = grant
        .probe
        .video
        .as_ref()
        .expect("admitted grants have video");
    for (flag, value) in [
        ("-color_primaries", &video.color_primaries),
        ("-color_trc", &video.color_transfer),
        ("-colorspace", &video.color_space),
    ] {
        if let Some(value) = value.as_deref().filter(|v| *v != "unknown" && v.len() < 32) {
            args.push(flag.into());
            args.push(value.into());
        }
    }
    if grant.audio == AudioPolicy::KeepAac {
        args.extend(media::os(&["-c:a", "aac", "-b:a", "192k"]));
    }
    args.extend(media::os(&[
        "-movflags",
        "+faststart",
        "-f",
        "mp4",
        "-progress",
        "pipe:1",
        "-nostats",
    ]));
    args.push(output.into());
    args
}

/// A user-facing reason for an FFmpeg exit. No FFmpeg message means the tool was stopped from
/// outside (crash, kill), which is not a problem with the video.
fn failure_text(code: Option<i32>, last_line: &str) -> String {
    let code = code.map_or("no exit code".to_string(), |c| format!("exit code {c}"));
    let line = last_line.trim().trim_end_matches('.');
    if line.is_empty() {
        format!("FFmpeg stopped unexpectedly before the export finished ({code}).")
    } else {
        format!("FFmpeg could not encode this video: {line} ({code}).")
    }
}

fn damaged_text(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let first = lines
        .first()
        .map(|l| l.split("] ").last().unwrap_or(l).trim())
        .unwrap_or("")
        .chars()
        .take(120)
        .collect::<String>();
    format!(
        "The source video has damaged frames in this range: FFmpeg reported {} decoding problem(s), starting with \"{first}\".",
        lines.len()
    )
}

enum EncodeEnd {
    Done,
    Cancelled,
    Failed(String),
}

fn encode(
    job: &Job,
    tools: &Tools,
    args: Vec<OsString>,
    output: &Path,
    size_limit: u64,
) -> EncodeEnd {
    let mut command = Command::new(&tools.ffmpeg);
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => return EncodeEnd::Failed("FFmpeg could not be started.".into()),
    };
    let tree = match ProcessTree::adopt(&child) {
        Ok(tree) => tree,
        Err(e) => {
            let _ = child.kill();
            let _ = child.wait();
            return EncodeEnd::Failed(e);
        }
    };
    let progress = Arc::new(Mutex::new((0usize, Instant::now())));
    let reader = child.stdout.take().map(|stdout| {
        let progress = progress.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(frames) = line
                    .strip_prefix("frame=")
                    .and_then(|v| v.trim().parse().ok())
                {
                    let mut p = progress.lock().unwrap_or_else(|p| p.into_inner());
                    if frames != p.0 {
                        *p = (frames, Instant::now());
                    }
                }
            }
        })
    });
    let errors = child.stderr.take().map(|mut stderr| {
        thread::spawn(move || {
            let mut text = Vec::new();
            let _ = (&mut stderr).take(64 * 1024).read_to_end(&mut text);
            let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            String::from_utf8_lossy(&text).to_string()
        })
    });
    let started = Instant::now();
    let stop = |reason: Option<String>| {
        tree.terminate();
        reason
    };
    let outcome: Result<std::process::ExitStatus, Option<String>> = loop {
        if job.cancel.load(Ordering::SeqCst) {
            break Err(stop(None));
        }
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(_) => break Err(stop(Some("Lost track of the FFmpeg process.".into()))),
        }
        if started.elapsed() > JOB_TIMEOUT {
            break Err(stop(Some(
                "The export took longer than 30 minutes and was stopped.".into(),
            )));
        }
        let (frames, last_change) = *progress.lock().unwrap_or_else(|p| p.into_inner());
        if last_change.elapsed() > STALL_TIMEOUT {
            break Err(stop(Some(
                "FFmpeg stopped making progress and was stopped.".into(),
            )));
        }
        if fs::metadata(output).map(|m| m.len()).unwrap_or(0) > size_limit {
            break Err(stop(Some(
                "The export grew past its size limit and was stopped.".into(),
            )));
        }
        job.update(|j| j.status.frames_done = frames.min(j.status.frame_count));
        thread::sleep(Duration::from_millis(100));
    };
    // Whatever happened, the process tree is gone before we report.
    let _ = child.wait();
    drop(tree);
    if let Some(reader) = reader {
        let _ = reader.join();
    }
    let stderr = errors.and_then(|e| e.join().ok()).unwrap_or_default();
    match outcome {
        Err(None) => EncodeEnd::Cancelled,
        Err(Some(reason)) => EncodeEnd::Failed(reason),
        // FFmpeg runs at `-v error`: a clean encode prints nothing. Decoder errors mean damaged
        // source frames that FFmpeg concealed; exporting them as a verified copy would be false.
        Ok(status) if status.success() && !stderr.trim().is_empty() => {
            EncodeEnd::Failed(damaged_text(&stderr))
        }
        Ok(status) if status.success() => EncodeEnd::Done,
        Ok(status) => {
            let last = stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .chars()
                .take(200)
                .collect::<String>();
            EncodeEnd::Failed(failure_text(status.code(), &last))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------------------------

/// Decode one frame (seeking to `at` when given) to a 64x36 grey thumbnail.
fn thumbnail(tools: &Tools, input: &Path, at: Option<i64>) -> Result<Vec<u8>, String> {
    let mut args = media::os(&["-v", "error", "-nostdin"]);
    if let Some(us) = at {
        args.push("-ss".into());
        args.push(format!("{us}us").into());
    }
    args.push("-i".into());
    args.push(input.into());
    args.extend(media::os(&[
        "-map",
        "0:v:0",
        "-frames:v",
        "1",
        "-vf",
        "scale=64:36:flags=area,format=gray",
        "-f",
        "rawvideo",
        "-",
    ]));
    let bytes = media::run_bounded(&tools.ffmpeg, &args, Duration::from_secs(120))
        .map_err(|e| format!("A frame could not be decoded for checking: {e}"))?;
    if bytes.len() != 64 * 36 {
        return Err("A frame could not be decoded for checking.".into());
    }
    Ok(bytes)
}

fn mean_difference(a: &[u8], b: &[u8]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (*x as f64 - *y as f64).abs())
        .sum::<f64>()
        / a.len() as f64
}

#[derive(Deserialize)]
struct RawStreams {
    #[serde(default)]
    streams: Vec<RawStreamDuration>,
}
#[derive(Deserialize)]
struct RawStreamDuration {
    codec_type: Option<String>,
    duration: Option<String>,
}

fn stream_durations(tools: &Tools, file: &Path) -> Result<(Option<f64>, Option<f64>), String> {
    let mut args = media::os(&[
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_streams",
        "-i",
    ]);
    args.push(file.into());
    let raw = media::run_bounded(&tools.ffprobe, &args, Duration::from_secs(60))
        .map_err(|e| e.to_string())?;
    let parsed: RawStreams = serde_json::from_slice(&raw).map_err(|e| e.to_string())?;
    let find = |kind: &str| {
        parsed
            .streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some(kind))
            .and_then(|s| s.duration.as_deref())
            .and_then(|d| d.parse::<f64>().ok())
    };
    Ok((find("video"), find("audio")))
}

/// Check the staged output before anything is published.
fn validate_output(
    tools: &Tools,
    grant: &Grant,
    plan: &TrimPlan,
    output: &Path,
) -> Result<ExportedVideo, String> {
    let fail = |m: String| format!("The exported video failed its check: {m}. Nothing was saved.");
    let probe = media::probe(tools, output).map_err(|e| fail(e.to_string()))?;
    let video = probe
        .video
        .as_ref()
        .ok_or_else(|| fail("no video stream".into()))?;
    if video.codec != "h264" || !probe.container.contains("mp4") {
        return Err(fail(format!(
            "unexpected {} in {}",
            video.codec, probe.container
        )));
    }
    if (video.width, video.height) != (plan.width, plan.height) || video.rotation_degrees != 0 {
        return Err(fail(format!(
            "expected {}×{} upright, got {}×{} rotated {}°",
            plan.width, plan.height, video.width, video.height, video.rotation_degrees
        )));
    }
    let want_audio = plan.audio == AudioPolicy::KeepAac;
    if probe.audio_streams != u32::from(want_audio) {
        return Err(fail(format!(
            "expected {} audio track(s), found {}",
            u32::from(want_audio),
            probe.audio_streams
        )));
    }
    let frames =
        media::enumerate_frames(tools, output, MAX_CLIP_FRAMES).map_err(|e| fail(e.to_string()))?;
    if frames.len() != plan.frame_count {
        return Err(fail(format!(
            "expected {} frames, found {}",
            plan.frame_count,
            frames.len()
        )));
    }
    let (video_seconds, audio_seconds) = stream_durations(tools, output).map_err(fail)?;
    let frame = plan.duration_seconds / plan.frame_count as f64;
    if let Some(v) = video_seconds {
        if (v - plan.duration_seconds).abs() > frame * 1.5 + 0.002 {
            return Err(fail(format!(
                "video lasts {v:.3} s, expected {:.3} s",
                plan.duration_seconds
            )));
        }
    }
    if want_audio {
        let a = audio_seconds.ok_or_else(|| fail("the audio track has no duration".into()))?;
        if (a - plan.duration_seconds).abs() > (frame * 2.0).max(0.1) {
            return Err(fail(format!(
                "audio lasts {a:.3} s but the video lasts {:.3} s",
                plan.duration_seconds
            )));
        }
    }
    // The first and last output frames must look like the chosen source frames.
    let source_first = thumbnail(
        tools,
        &grant.path,
        Some(micros(grant.frames[plan.first.index])),
    )?;
    let source_last = thumbnail(
        tools,
        &grant.path,
        Some(micros(grant.frames[plan.last.index])),
    )?;
    let out_first = thumbnail(tools, output, None)?;
    let out_last = thumbnail(tools, output, Some(micros(frames[frames.len() - 1])))?;
    let first_diff = mean_difference(&source_first, &out_first);
    let last_diff = mean_difference(&source_last, &out_last);
    if first_diff > FRAME_MATCH_TOLERANCE || last_diff > FRAME_MATCH_TOLERANCE {
        return Err(fail(format!(
            "first/last frames differ from the source (mean {first_diff:.1}/{last_diff:.1})"
        )));
    }
    let bytes = fs::metadata(output).map_err(|e| fail(e.to_string()))?.len();
    Ok(ExportedVideo {
        file_name: String::new(),
        sha256: sha256_file(output)?,
        bytes,
        width: video.width,
        height: video.height,
        frame_count: frames.len(),
        duration_seconds: video_seconds.unwrap_or(plan.duration_seconds),
        has_audio: want_audio,
    })
}

// ---------------------------------------------------------------------------------------------
// Publishing
// ---------------------------------------------------------------------------------------------

fn copy_new(from: &Path, to: &Path) -> Result<(), String> {
    let mut source =
        File::open(from).map_err(|_| "Could not read the staged export.".to_string())?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .map_err(|_| "Could not create a new file in the Pictures folder.".to_string())?;
    let copied = std::io::copy(&mut source, &mut target).and_then(|_| target.sync_all());
    drop(target);
    copied.map_err(|_| "Could not finish writing the export (is the disk full?).".to_string())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Could not create the export receipt.".to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "Could not finish the export receipt.".to_string())
}

fn publish(
    job: &Job,
    trusted: &TrustedTools,
    grant: &Grant,
    plan: &TrimPlan,
    staged: &Path,
    exported: &mut ExportedVideo,
    pictures: &Path,
) -> Result<(), String> {
    check_directory(pictures)?;
    let dest_dir = pictures.join("Optic Rift");
    match fs::create_dir(&dest_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err("Could not create the Optic Rift Pictures folder.".into()),
    }
    check_directory(&dest_dir)?;
    let stem = format!("optic-rift-trim-{}", uuid::Uuid::new_v4());
    let video_path = dest_dir.join(format!("{stem}.mp4"));
    let receipt_path = dest_dir.join(format!("{stem}.provenance.json"));
    let partial_video = dest_dir.join(format!(".{stem}.partial.mp4"));
    let partial_receipt = dest_dir.join(format!(".{stem}.partial.json"));
    exported.file_name = format!("{stem}.mp4");
    let video = grant
        .probe
        .video
        .as_ref()
        .expect("admitted grants have video");
    let record = serde_json::json!({
        "record_version": 1,
        "kind": "video-trim-export",
        "source": {
            "file_name": grant.name,
            "sha256": grant.sha256,
            "bytes": grant.size,
            "codec": video.codec,
            "rotation_degrees": video.rotation_degrees,
            "verified": "native SHA-256 of the user-chosen file, checked before and after encoding"
        },
        "range": {
            "first_frame_index": plan.first.index,
            "first_frame_pts": plan.first.pts,
            "first_frame_seconds": plan.first.seconds,
            "last_frame_index": plan.last.index,
            "last_frame_pts": plan.last.pts,
            "last_frame_seconds": plan.last.seconds,
            "time_base": format!("{}/{}", video.time_base.num, video.time_base.den),
            "frame_count": plan.frame_count
        },
        "output": {
            "file_name": exported.file_name,
            "sha256": exported.sha256,
            "bytes": exported.bytes,
            "width": exported.width,
            "height": exported.height,
            "frame_count": exported.frame_count,
            "duration_seconds": exported.duration_seconds,
            "video": "H.264 (libx264, CRF 16, yuv420p), MP4, rotation applied to pixels",
            "audio": match plan.audio {
                AudioPolicy::None => "none (the source has no audio track)",
                AudioPolicy::KeepAac => "the source's one audio track, re-encoded to AAC 192 kb/s over the same span",
            },
            "not_copied": "metadata, chapters, subtitles and data tracks"
        },
        "tool": trusted.identity,
        "created_unix_ms": now_ms(),
        "note": "Exported copy only; not an editable project asset. The source file was not modified."
    });
    let receipt = serde_json::to_vec_pretty(&record)
        .map_err(|_| "Could not write the export receipt.".to_string())?;
    job.update(|j| j.dest_partials = vec![partial_video.clone(), partial_receipt.clone()]);
    let result = (|| -> Result<(), String> {
        copy_new(staged, &partial_video)?;
        if sha256_file(&partial_video)? != exported.sha256 {
            return Err("The copied export did not match the checked file.".into());
        }
        write_new(&partial_receipt, &receipt)?;
        if job.cancel.load(Ordering::SeqCst) {
            return Err(String::new());
        }
        // Hard links never replace an existing file.
        fs::hard_link(&partial_video, &video_path)
            .map_err(|_| "Could not save without overwriting another file.".to_string())?;
        job.update(|j| j.published = vec![video_path.clone()]);
        fs::hard_link(&partial_receipt, &receipt_path)
            .map_err(|_| "Could not save the export receipt.".to_string())?;
        job.update(|j| j.published = vec![video_path.clone(), receipt_path.clone()]);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&video_path);
        let _ = fs::remove_file(&receipt_path);
        job.update(|j| j.published.clear());
    }
    let _ = fs::remove_file(&partial_video);
    let _ = fs::remove_file(&partial_receipt);
    job.update(|j| j.dest_partials.clear());
    result
}

fn run_job(job: Arc<Job>, trusted: TrustedTools, grant: Grant, plan: TrimPlan, pictures: PathBuf) {
    let staging_dir = job
        .journal
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .staging_dir
        .clone();
    let staged = staging_dir.join("trim.mp4");
    let size_limit = (grant.size.saturating_mul(4)).clamp(256 * 1024 * 1024, MAX_OUTPUT_BYTES);
    let finish = |state: JobState, message: String, output: Option<ExportedVideo>| {
        let _ = fs::remove_dir_all(&staging_dir);
        job.update(|j| {
            j.status.state = state;
            j.status.message = message;
            j.status.output = output;
        });
    };
    let cancelled = || "Export cancelled. FFmpeg was stopped and no file was saved.".to_string();
    match encode(
        &job,
        &trusted.tools,
        encode_args(&grant, &plan, &staged),
        &staged,
        size_limit,
    ) {
        EncodeEnd::Cancelled => return finish(JobState::Cancelled, cancelled(), None),
        EncodeEnd::Failed(reason) => {
            return finish(
                JobState::Failed,
                format!("{reason} No file was saved."),
                None,
            )
        }
        EncodeEnd::Done => {}
    }
    job.update(|j| {
        j.status.state = JobState::Validating;
        j.status.frames_done = j.status.frame_count;
        j.status.message = "Checking the exported frames…".into();
    });
    let checked = validate_output(&trusted.tools, &grant, &plan, &staged)
        .and_then(|exported| source_unchanged(&grant, true).map(|_| exported));
    let mut exported = match checked {
        Ok(exported) => exported,
        Err(reason) => return finish(JobState::Failed, reason, None),
    };
    if job.cancel.load(Ordering::SeqCst) {
        return finish(JobState::Cancelled, cancelled(), None);
    }
    job.update(|j| {
        j.status.state = JobState::Publishing;
        j.status.message = "Saving to Pictures…".into();
    });
    match publish(
        &job,
        &trusted,
        &grant,
        &plan,
        &staged,
        &mut exported,
        &pictures,
    ) {
        Ok(()) => finish(
            JobState::Succeeded,
            format!(
                "Saved {} to Pictures/Optic Rift. Your original video was not changed.",
                exported.file_name
            ),
            Some(exported),
        ),
        Err(reason) if reason.is_empty() => finish(JobState::Cancelled, cancelled(), None),
        Err(reason) => finish(
            JobState::Failed,
            format!("{reason} No file was saved."),
            None,
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// Restart reconciliation
// ---------------------------------------------------------------------------------------------

/// Run once when the app starts. Any job that was not finished when the app stopped is marked
/// interrupted and its staging and partial files are deleted. A published file from such a job
/// is removed too: it was never confirmed as a completed export.
pub fn reconcile(roots: &ExportRoots) -> Vec<JobStatus> {
    let mut statuses = Vec::new();
    let Ok(entries) = fs::read_dir(roots.journal_dir()) else {
        return statuses;
    };
    let mut journals: Vec<(PathBuf, Journal)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .filter_map(|p| {
            let bytes = fs::read(&p).ok()?;
            serde_json::from_slice::<Journal>(&bytes)
                .ok()
                .map(|j| (p, j))
        })
        .collect();
    journals.sort_by_key(|(_, j)| j.status.started_unix_ms);
    let excess = journals.len().saturating_sub(MAX_JOURNAL_ENTRIES);
    for (path, _) in journals.drain(..excess) {
        let _ = fs::remove_file(path);
    }
    for (path, mut journal) in journals {
        if !journal.status.state.is_final() {
            for file in journal.dest_partials.iter().chain(journal.published.iter()) {
                let _ = fs::remove_file(file);
            }
            journal.dest_partials.clear();
            journal.published.clear();
            journal.status.state = JobState::Interrupted;
            journal.status.message =
                "Optic Rift closed while this export was running. Its partial files were removed; no video was saved."
                    .into();
            journal.status.output = None;
            journal.status.finished_unix_ms = Some(now_ms());
            let _ = write_journal(&path, &journal);
        }
        let _ = fs::remove_dir_all(&journal.staging_dir);
        let job = Arc::new(Job {
            journal: Mutex::new(journal.clone()),
            cancel: AtomicBool::new(false),
            journal_path: path,
        });
        statuses.push(journal.status.clone());
        jobs().lock().unwrap_or_else(|p| p.into_inner()).push(job);
    }
    // Staging folders without a journal (a crash before the first write) are removed as well.
    if let Ok(entries) = fs::read_dir(roots.staging_root()) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().to_string();
            if !roots.journal_dir().join(format!("{name}.json")).exists() {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
    statuses
}

// ---------------------------------------------------------------------------------------------
// Native file dialog
// ---------------------------------------------------------------------------------------------

/// Show the Windows "open file" dialog, pre-filled with the clip's name. `Ok(None)` means the
/// user cancelled. Must run on a thread that may enter a COM single-threaded apartment.
#[cfg(windows)]
pub fn pick_video_file(owner: isize, suggested_name: &str) -> Result<Option<PathBuf>, String> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST,
        FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, SIGDN_FILESYSPATH,
    };
    const ERROR_CANCELLED_HRESULT: i32 = 0x8007_04C7_u32 as i32;
    // SAFETY: COM is initialised for this thread and uninitialised on every path; all interface
    // pointers are owned by `windows` smart wrappers; the returned string is freed once.
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let result = (|| -> windows::core::Result<Option<PathBuf>> {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            let options = dialog.GetOptions()?;
            dialog.SetOptions(
                options
                    | FOS_FILEMUSTEXIST
                    | FOS_FORCEFILESYSTEM
                    | FOS_NOCHANGEDIR
                    | FOS_DONTADDTORECENT,
            )?;
            let label = HSTRING::from("Video files (MP4, MOV, M4V, WebM, MKV)");
            let pattern = HSTRING::from("*.mp4;*.m4v;*.mov;*.webm;*.mkv");
            let filters = [COMDLG_FILTERSPEC {
                pszName: PCWSTR(label.as_ptr()),
                pszSpec: PCWSTR(pattern.as_ptr()),
            }];
            dialog.SetFileTypes(&filters)?;
            dialog.SetTitle(&HSTRING::from(format!(
                "Optic Rift: choose {suggested_name} to allow a trimmed copy"
            )))?;
            dialog.SetOkButtonLabel(&HSTRING::from("Allow trimmed copy"))?;
            dialog.SetFileName(&HSTRING::from(suggested_name))?;
            if let Err(error) = dialog.Show(Some(HWND(owner as *mut core::ffi::c_void))) {
                if error.code().0 == ERROR_CANCELLED_HRESULT {
                    return Ok(None);
                }
                return Err(error);
            }
            let item = dialog.GetResult()?;
            let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = name.to_string();
            CoTaskMemFree(Some(name.0 as *const core::ffi::c_void));
            let path =
                path.map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
            Ok(Some(PathBuf::from(path)))
        })();
        if init.is_ok() {
            CoUninitialize();
        }
        result.map_err(|e| format!("The Windows file dialog failed: {e}"))
    }
}

#[cfg(not(windows))]
pub fn pick_video_file(_owner: isize, _suggested_name: &str) -> Result<Option<PathBuf>, String> {
    Err("Choosing a source video is only implemented on Windows.".into())
}

// ---------------------------------------------------------------------------------------------
// Tests (FFmpeg is a host prerequisite, as for `media`)
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("optic-rift-trim-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn trusted() -> TrustedTools {
        trusted_tools(None)
            .expect("ffmpeg with libx264 and aac must be on PATH or in OPTIC_RIFT_FFMPEG_DIR")
    }

    fn ffmpeg(t: &TrustedTools, args: &[&str]) {
        media::run_bounded(&t.tools.ffmpeg, &media::os(args), Duration::from_secs(120)).unwrap();
    }

    /// 60 frames at 30 fps; luma = 4 × frame number. B-frames, a keyframe every 7 frames.
    fn counter(t: &TrustedTools, dir: &Scratch, name: &str, extra: &[&str]) -> PathBuf {
        let out = dir.join(name);
        let mut args = vec![
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=64x48:r=30:d=2,format=gray,geq=lum='N*4'",
        ];
        args.extend_from_slice(extra);
        args.extend_from_slice(&[
            "-c:v", "libx264", "-qp", "0", "-bf", "2", "-g", "7", "-pix_fmt", "yuv420p",
        ]);
        let path = out.to_str().unwrap().to_string();
        args.push(&path);
        ffmpeg(t, &args);
        out
    }

    fn request_for(path: &Path) -> GrantRequest {
        let meta = fs::metadata(path).unwrap();
        GrantRequest {
            asset_id: uuid::Uuid::new_v4().to_string(),
            name: path.file_name().unwrap().to_str().unwrap().into(),
            size: meta.len(),
            last_modified_ms: modified_ms(&meta),
        }
    }

    /// Mean luma of every decoded frame of `file`, mapped back to counter numbers.
    fn counter_frames(t: &TrustedTools, file: &Path) -> Vec<i64> {
        let mut args = media::os(&["-v", "error", "-i"]);
        args.push(file.into());
        args.extend(media::os(&[
            // One raw frame per stored frame: no duplicates filling VFR timing gaps.
            "-map",
            "0:v:0",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "-",
        ]));
        let bytes = media::run_bounded(&t.tools.ffmpeg, &args, Duration::from_secs(120)).unwrap();
        let probe = media::probe(&t.tools, file).unwrap().video.unwrap();
        let size = (probe.width * probe.height) as usize;
        bytes
            .chunks(size)
            .map(|f| {
                (f.iter().map(|&b| b as f64).sum::<f64>() / f.len() as f64 / 4.0).round() as i64
            })
            .collect()
    }

    fn roots(dir: &Scratch) -> ExportRoots {
        let pictures = dir.join("Pictures");
        fs::create_dir_all(&pictures).unwrap();
        ExportRoots {
            app_data: dir.join("AppData"),
            pictures,
            resource_dir: None,
        }
    }

    fn wait(job_id: &str) -> JobStatus {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let s = status(job_id).unwrap();
            if s.state.is_final() {
                return s;
            }
            assert!(Instant::now() < deadline, "job never finished: {s:?}");
            thread::sleep(Duration::from_millis(50));
        }
    }

    /// Jobs are global (one at a time), so tests that start jobs run one after another.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn published(dir: &Scratch) -> Vec<String> {
        fs::read_dir(dir.join("Pictures").join("Optic Rift"))
            .map(|r| {
                r.filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn non_keyframe_cut_exports_exactly_the_chosen_frames() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let source = counter(&t, &dir, "counter.mp4", &[]);
        let before = sha256_file(&source).unwrap();
        let request = request_for(&source);
        let grant = grant_picked(&t, &request, &source).unwrap();
        assert_eq!(grant.frame_count, 60);
        assert_eq!(grant.audio, AudioPolicy::None);
        // Frames 10..=40 (10 and 40 are not keyframes with a 7-frame GOP).
        let trim = TrimRequest {
            grant_id: grant.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: 10.0 / 30.0,
            out_seconds: Some(40.0 / 30.0),
        };
        let plan = resolve(&trim).unwrap();
        assert_eq!(
            (plan.first.index, plan.last.index, plan.frame_count),
            (10, 40, 31)
        );
        let roots = roots(&dir);
        let job = start(
            &roots,
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: trim.in_seconds,
                out_seconds: trim.out_seconds,
                expected_first_index: 10,
                expected_last_index: 40,
                accept_pictures_export: true,
            },
        )
        .unwrap();
        let done = wait(&job.job_id);
        assert_eq!(done.state, JobState::Succeeded, "{}", done.message);
        let output = done.output.unwrap();
        let file = dir
            .join("Pictures")
            .join("Optic Rift")
            .join(&output.file_name);
        assert_eq!(counter_frames(&t, &file), (10..=40).collect::<Vec<_>>());
        assert_eq!(sha256_file(&file).unwrap(), output.sha256);
        assert_eq!(sha256_file(&source).unwrap(), before, "source untouched");
        let receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(file.with_extension("provenance.json")).unwrap())
                .unwrap();
        assert_eq!(receipt["source"]["sha256"], before);
        assert_eq!(receipt["output"]["sha256"], output.sha256);
        assert_eq!(receipt["range"]["first_frame_index"], 10);
        assert_eq!(receipt["range"]["last_frame_index"], 40);
        assert_eq!(published(&dir).len(), 2, "video + receipt, no partials");
        // A second export of the same range gets a new name and leaves the first alone.
        let again = start(
            &roots,
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: trim.in_seconds,
                out_seconds: trim.out_seconds,
                expected_first_index: 10,
                expected_last_index: 40,
                accept_pictures_export: true,
            },
        )
        .unwrap();
        let second = wait(&again.job_id);
        assert_eq!(second.state, JobState::Succeeded);
        assert_ne!(second.output.unwrap().file_name, output.file_name);
        assert_eq!(
            sha256_file(&file).unwrap(),
            output.sha256,
            "first export unchanged"
        );
        assert_eq!(published(&dir).len(), 4);
    }

    #[test]
    fn rotation_and_audio_follow_the_declared_policy() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let plain = dir.join("plain.mp4");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x48:r=30:d=2,format=gray,geq=lum='N*4'",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:d=2",
                "-c:v",
                "libx264",
                "-qp",
                "0",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-shortest",
                plain.to_str().unwrap(),
            ],
        );
        let rotated = dir.join("rotated.mp4");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-display_rotation:v",
                "90",
                "-i",
                plain.to_str().unwrap(),
                "-c",
                "copy",
                rotated.to_str().unwrap(),
            ],
        );
        let request = request_for(&rotated);
        let grant = grant_picked(&t, &request, &rotated).unwrap();
        assert_eq!((grant.width, grant.height), (48, 64));
        assert_eq!(grant.audio, AudioPolicy::KeepAac);
        let plan = resolve(&TrimRequest {
            grant_id: grant.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: 0.5,
            out_seconds: Some(1.5),
        })
        .unwrap();
        let job = start(
            &roots(&dir),
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: 0.5,
                out_seconds: Some(1.5),
                expected_first_index: plan.first.index,
                expected_last_index: plan.last.index,
                accept_pictures_export: true,
            },
        )
        .unwrap();
        let done = wait(&job.job_id);
        assert_eq!(done.state, JobState::Succeeded, "{}", done.message);
        let output = done.output.unwrap();
        assert!(output.has_audio);
        assert_eq!(
            (output.width, output.height),
            (48, 64),
            "rotation applied to pixels"
        );
        let file = dir
            .join("Pictures")
            .join("Optic Rift")
            .join(&output.file_name);
        assert_eq!(counter_frames(&t, &file), (15..=45).collect::<Vec<_>>());

        // Two audio tracks are refused rather than silently reduced to one.
        let two = dir.join("two.mkv");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-i",
                plain.to_str().unwrap(),
                "-f",
                "lavfi",
                "-i",
                "sine=f=880:d=2",
                "-map",
                "0",
                "-map",
                "1",
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                two.to_str().unwrap(),
            ],
        );
        let error = grant_picked(&t, &request_for(&two), &two).unwrap_err();
        assert!(error.contains("2 audio tracks"), "{error}");
    }

    #[test]
    fn variable_frame_rate_cut_uses_real_frames() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let cfr = counter(&t, &dir, "cfr.mp4", &[]);
        let vfr = dir.join("vfr.mkv");
        // Even frames only in the first second, every frame after.
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-i",
                cfr.to_str().unwrap(),
                "-vf",
                "select='gte(n\\,30)+not(mod(n\\,2))'",
                "-fps_mode",
                "vfr",
                "-c:v",
                "libx264",
                "-qp",
                "0",
                "-pix_fmt",
                "yuv420p",
                vfr.to_str().unwrap(),
            ],
        );
        let request = request_for(&vfr);
        let grant = grant_picked(&t, &request, &vfr).unwrap();
        assert_eq!(grant.frame_count, 45);
        // 0.70 s shows source frame 20 (index 10); 1.30 s shows source frame 39 (index 24).
        let trim = TrimRequest {
            grant_id: grant.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: 0.70,
            out_seconds: Some(1.30),
        };
        let plan = resolve(&trim).unwrap();
        assert_eq!((plan.first.index, plan.last.index), (10, 24));
        let job = start(
            &roots(&dir),
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: 0.70,
                out_seconds: Some(1.30),
                expected_first_index: 10,
                expected_last_index: 24,
                accept_pictures_export: true,
            },
        )
        .unwrap();
        let done = wait(&job.job_id);
        assert_eq!(done.state, JobState::Succeeded, "{}", done.message);
        let file = dir
            .join("Pictures")
            .join("Optic Rift")
            .join(done.output.unwrap().file_name);
        let mut wanted: Vec<i64> = vec![20, 22, 24, 26, 28];
        wanted.extend(30..=39);
        assert_eq!(counter_frames(&t, &file), wanted);
    }

    #[test]
    fn refusals_write_nothing() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let source = counter(&t, &dir, "c.mp4", &[]);
        let request = request_for(&source);
        // Picked file must match the clip open in Video Studio.
        let mut wrong = request_for(&source);
        wrong.size += 1;
        assert!(grant_picked(&t, &wrong, &source)
            .unwrap_err()
            .contains("not the clip"));
        wrong = request_for(&source);
        wrong.name = "other.mp4".into();
        assert!(grant_picked(&t, &wrong, &source).is_err());
        wrong = request_for(&source);
        wrong.name = "..\\c.mp4".into();
        assert!(grant_picked(&t, &wrong, &source).is_err());
        let text = dir.join("notes.txt");
        fs::write(&text, b"x").unwrap();
        assert!(grant_picked(&t, &request_for(&text), &text).is_err());
        let fake = dir.join("fake.mp4");
        fs::write(&fake, vec![7u8; 4096]).unwrap();
        assert!(grant_picked(&t, &request_for(&fake), &fake).is_err());
        // HDR (PQ transfer, BT.2020 primaries) is refused until color management exists.
        let hdr = dir.join("hdr.mp4");
        ffmpeg(&t, &["-v", "error", "-f", "lavfi", "-i", "testsrc2=s=64x48:r=30:d=1", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-bsf:v", "h264_metadata=colour_primaries=9:transfer_characteristics=16:matrix_coefficients=9", hdr.to_str().unwrap()]);
        assert!(grant_picked(&t, &request_for(&hdr), &hdr)
            .unwrap_err()
            .contains("HDR"));

        let grant = grant_picked(&t, &request, &source).unwrap();
        let ok = |first, last| StartRequest {
            grant_id: grant.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: 0.2,
            out_seconds: Some(1.0),
            expected_first_index: first,
            expected_last_index: last,
            accept_pictures_export: true,
        };
        let r = roots(&dir);
        assert!(start(
            &r,
            StartRequest {
                accept_pictures_export: false,
                ..ok(6, 30)
            }
        )
        .unwrap_err()
        .contains("Approve"));
        assert!(start(&r, ok(5, 30))
            .unwrap_err()
            .contains("changed since you confirmed"));
        assert!(start(
            &r,
            StartRequest {
                asset_id: uuid::Uuid::new_v4().to_string(),
                ..ok(6, 30)
            }
        )
        .unwrap_err()
        .contains("not authorized"));
        assert!(start(
            &r,
            StartRequest {
                grant_id: uuid::Uuid::new_v4().to_string(),
                ..ok(6, 30)
            }
        )
        .is_err());
        for (a, b) in [
            (1.0, Some(0.5)),
            (f64::NAN, None),
            (-1.0, None),
            (0.0, Some(99.0)),
            (0.5, Some(0.51)),
        ] {
            assert!(
                resolve(&TrimRequest {
                    grant_id: grant.grant_id.clone(),
                    asset_id: request.asset_id.clone(),
                    in_seconds: a,
                    out_seconds: b
                })
                .is_err(),
                "{a} {b:?}"
            );
        }
        // Unknown request fields (for example a path) are rejected by the IPC types.
        assert!(serde_json::from_str::<StartRequest>(r#"{"grantId":"x","assetId":"y","inSeconds":0,"outSeconds":null,"expectedFirstIndex":0,"expectedLastIndex":1,"acceptPicturesExport":true,"path":"C:\\x.mp4"}"#).is_err());
        // Source mutation after the grant: refused before encoding.
        let mut bytes = fs::read(&source).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF;
        fs::write(&source, &bytes).unwrap();
        assert!(start(&r, ok(6, 30))
            .unwrap_err()
            .contains("changed on disk"));
        assert!(published(&dir).is_empty());
        assert!(revoke(&grant.grant_id));
        assert!(!revoke(&grant.grant_id));
    }

    #[test]
    fn damaged_source_frames_fail_instead_of_being_concealed() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let clean = dir.join("clean.mp4");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=320x240:r=30:d=4",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "30",
                "-pix_fmt",
                "yuv420p",
                clean.to_str().unwrap(),
            ],
        );
        let mut bytes = fs::read(&clean).unwrap();
        let len = bytes.len();
        let mut x: u64 = 7;
        for _ in 0..300 {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let at = len * 2 / 5 + (x >> 33) as usize % (len / 5);
            bytes[at] = (x >> 17) as u8;
        }
        let damaged = dir.join("damaged.mp4");
        fs::write(&damaged, &bytes).unwrap();
        let request = request_for(&damaged);
        let grant = grant_picked(&t, &request, &damaged).unwrap();
        let plan = resolve(&TrimRequest {
            grant_id: grant.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: 0.0,
            out_seconds: None,
        })
        .unwrap();
        let job = start(
            &roots(&dir),
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: 0.0,
                out_seconds: None,
                expected_first_index: plan.first.index,
                expected_last_index: plan.last.index,
                accept_pictures_export: true,
            },
        )
        .unwrap();
        let done = wait(&job.job_id);
        assert_eq!(done.state, JobState::Failed, "{}", done.message);
        assert!(done.message.contains("damaged frames"), "{}", done.message);
        assert!(published(&dir).is_empty());
    }

    #[test]
    fn failure_messages_are_plain() {
        assert_eq!(
            failure_text(Some(-1), "  "),
            "FFmpeg stopped unexpectedly before the export finished (exit code -1)."
        );
        assert_eq!(
            failure_text(None, ""),
            "FFmpeg stopped unexpectedly before the export finished (no exit code)."
        );
        assert_eq!(
            failure_text(Some(1), "Invalid data found."),
            "FFmpeg could not encode this video: Invalid data found (exit code 1)."
        );
        assert!(!failure_text(Some(-1), "").contains("Some("));
    }

    #[test]
    fn simultaneous_starts_run_only_one_job() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let source = counter(&t, &dir, "c.mp4", &[]);
        let request = request_for(&source);
        let grant = grant_picked(&t, &request, &source).unwrap();
        let r = Arc::new(roots(&dir));
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let (r, b, g, a) = (
                    r.clone(),
                    barrier.clone(),
                    grant.grant_id.clone(),
                    request.asset_id.clone(),
                );
                thread::spawn(move || {
                    b.wait();
                    start(
                        &r,
                        StartRequest {
                            grant_id: g,
                            asset_id: a,
                            in_seconds: 0.0,
                            out_seconds: Some(1.0),
                            expected_first_index: 0,
                            expected_last_index: 30,
                            accept_pictures_export: true,
                        },
                    )
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let started: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
        assert_eq!(started.len(), 1, "{results:?}");
        assert!(results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .all(|e| e.contains("Another video export")));
        assert_eq!(wait(&started[0].job_id).state, JobState::Succeeded);
        assert_eq!(published(&dir).len(), 2);
    }

    #[test]
    fn cancel_stops_the_encoder_and_leaves_no_files() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        // Big enough to still be encoding when Cancel arrives.
        let source = dir.join("long.mp4");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=1280x720:r=30:d=60",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
                source.to_str().unwrap(),
            ],
        );
        let request = request_for(&source);
        let grant = grant_picked(&t, &request, &source).unwrap();
        let plan = resolve(&TrimRequest {
            grant_id: grant.grant_id.clone(),
            asset_id: request.asset_id.clone(),
            in_seconds: 0.0,
            out_seconds: None,
        })
        .unwrap();
        let r = roots(&dir);
        let job = start(
            &r,
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: 0.0,
                out_seconds: None,
                expected_first_index: plan.first.index,
                expected_last_index: plan.last.index,
                accept_pictures_export: true,
            },
        )
        .unwrap();
        // A second job is refused while one runs.
        assert!(start(
            &r,
            StartRequest {
                grant_id: grant.grant_id.clone(),
                asset_id: request.asset_id.clone(),
                in_seconds: 0.0,
                out_seconds: None,
                expected_first_index: plan.first.index,
                expected_last_index: plan.last.index,
                accept_pictures_export: true
            }
        )
        .unwrap_err()
        .contains("Another video export"));
        thread::sleep(Duration::from_millis(1500));
        cancel(&job.job_id).unwrap();
        let done = wait(&job.job_id);
        assert_eq!(done.state, JobState::Cancelled, "{}", done.message);
        assert!(published(&dir).is_empty());
        assert!(!r.staging_root().join(&job.job_id).exists());
    }

    #[test]
    fn restart_marks_unfinished_jobs_interrupted_and_removes_partials() {
        let dir = Scratch::new();
        let r = roots(&dir);
        fs::create_dir_all(r.journal_dir()).unwrap();
        let staging = r.staging_root().join("a1");
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("trim.mp4"), b"partial").unwrap();
        let dest = dir.join("Pictures").join("Optic Rift");
        fs::create_dir_all(&dest).unwrap();
        let partial = dest.join(".optic-rift-trim-a1.partial.mp4");
        fs::write(&partial, b"partial").unwrap();
        let orphan = r.staging_root().join("no-journal");
        fs::create_dir_all(&orphan).unwrap();
        let journal = Journal {
            status: JobStatus {
                job_id: "a1".into(),
                state: JobState::Publishing,
                source_name: "x.mp4".into(),
                frames_done: 5,
                frame_count: 9,
                message: String::new(),
                output: None,
                started_unix_ms: 1,
                finished_unix_ms: None,
            },
            staging_dir: staging.clone(),
            dest_partials: vec![partial.clone()],
            published: Vec::new(),
        };
        write_journal(&r.journal_dir().join("a1.json"), &journal).unwrap();
        let statuses = reconcile(&r);
        let a1 = statuses.iter().find(|s| s.job_id == "a1").unwrap();
        assert_eq!(a1.state, JobState::Interrupted);
        assert!(a1.output.is_none());
        assert!(!partial.exists() && !staging.exists() && !orphan.exists());
        assert!(
            fs::read_dir(&dest).unwrap().next().is_none(),
            "nothing published"
        );
    }

    #[test]
    fn substituted_ffmpeg_is_refused() {
        // Swaps the session-wide pin, so it must not overlap a test that starts jobs.
        let _serial = serial();
        let t = trusted();
        let mut pinned = pinned_tools().lock().unwrap();
        let saved = pinned.clone();
        let mut forged = t.clone();
        forged.identity.ffmpeg_sha256 = "0".repeat(64);
        *pinned = Some(forged);
        drop(pinned);
        let error = trusted_tools(None).err().unwrap();
        *pinned_tools().lock().unwrap() = saved;
        assert!(error.contains("changed on disk"), "{error}");
    }

    #[cfg(windows)]
    #[test]
    fn linked_sources_are_refused() {
        let _serial = serial();
        let t = trusted();
        let dir = Scratch::new();
        let real = dir.join("real");
        fs::create_dir_all(&real).unwrap();
        let source = counter(&t, &dir, "real/c.mp4", &[]);
        let junction = dir.join("jn");
        let made = Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                junction.to_str().unwrap(),
                real.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(made.status.success());
        let via = junction.join("c.mp4");
        let error = grant_picked(&t, &request_for(&via), &via).unwrap_err();
        assert!(error.contains("link"), "{error}");
        assert!(grant_picked(&t, &request_for(&source), &source).is_ok());
    }
}
