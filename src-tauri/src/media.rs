//! Deterministic native media operations over a controlled FFmpeg subprocess.
//!
//! Scope (proposed owner: `docs/architecture/media-pipeline.md` in draft PR #11):
//! probe a file, enumerate video frames with their exact presentation timestamps, extract one
//! exact source frame as a lossless PNG, and render a still through an explicit crop/adjust
//! recipe to a new PNG or JPEG. No model is involved and nothing here runs on the UI thread.
//!
//! Safety rules: arguments are passed as an argv vector (never a shell string); every child
//! process has a deadline and bounded output; inputs are never written; outputs must be new
//! files, are written to a temporary sibling and renamed into place, and are decoded back to
//! validate them before success is reported.
//!
//! Not yet: project/asset lineage records, proxies and a bundled FFmpeg. `video_trim` builds the
//! first user-facing export on top of this module.

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Largest accepted frame edge. Bounds decompression-bomb style inputs.
pub const MAX_DIMENSION: u32 = 16_384;
/// Upper bound on frames returned by one enumeration request.
pub const MAX_FRAMES_PER_REQUEST: usize = 10_000;
const MAX_TOOL_OUTPUT: usize = 32 * 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);
const RENDER_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Tools {
    /// Locate FFmpeg. `OPTIC_RIFT_FFMPEG_DIR` overrides the search; otherwise `PATH` is used.
    /// Bundling and licensing a specific FFmpeg build is a separate, unresolved decision.
    pub fn discover() -> Result<Self, MediaError> {
        let exe = |name: &str| format!("{name}{}", std::env::consts::EXE_SUFFIX);
        let dirs: Vec<PathBuf> = match std::env::var_os("OPTIC_RIFT_FFMPEG_DIR") {
            Some(dir) => vec![PathBuf::from(dir)],
            None => std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
        };
        for dir in dirs {
            let ffmpeg = dir.join(exe("ffmpeg"));
            let ffprobe = dir.join(exe("ffprobe"));
            if ffmpeg.is_file() && ffprobe.is_file() {
                return Ok(Self { ffmpeg, ffprobe });
            }
        }
        Err(MediaError::ToolMissing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "category", content = "detail", rename_all = "snake_case")]
pub enum MediaError {
    ToolMissing,
    InvalidRequest(String),
    Unsupported(String),
    ToolFailed(String),
    TimedOut,
    Io(String),
    OutputInvalid(String),
}

impl std::fmt::Display for MediaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ToolMissing => write!(f, "FFmpeg and ffprobe were not found"),
            Self::InvalidRequest(m) => write!(f, "Invalid request: {m}"),
            Self::Unsupported(m) => write!(f, "Unsupported media: {m}"),
            Self::ToolFailed(m) => write!(f, "Media tool failed: {m}"),
            Self::TimedOut => write!(f, "Media operation timed out"),
            Self::Io(m) => write!(f, "File error: {m}"),
            Self::OutputInvalid(m) => write!(f, "Output failed validation: {m}"),
        }
    }
}

fn io_err(error: std::io::Error) -> MediaError {
    MediaError::Io(error.to_string())
}

/// Run a tool with a deadline and bounded stdout/stderr. Never invokes a shell.
pub(crate) fn run_bounded(
    program: &Path,
    args: &[OsString],
    timeout: Duration,
) -> Result<Vec<u8>, MediaError> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(io_err)?;
    let drain = |stream: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buffer = Vec::new();
            if let Some(stream) = stream {
                let _ = stream
                    .take(MAX_TOOL_OUTPUT as u64 + 1)
                    .read_to_end(&mut buffer);
            }
            buffer
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(io_err)? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(MediaError::TimedOut);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();
    if out.len() > MAX_TOOL_OUTPUT {
        return Err(MediaError::ToolFailed(
            "tool output exceeded the limit".into(),
        ));
    }
    if !status.success() {
        let text = String::from_utf8_lossy(&err);
        let last = text
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        return Err(MediaError::ToolFailed(format!(
            "exit {:?}: {}",
            status.code(),
            last.chars().take(300).collect::<String>()
        )));
    }
    Ok(out)
}

pub(crate) fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

/// A rational time base such as 1/15360.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rational {
    pub num: i64,
    pub den: i64,
}

impl Rational {
    fn parse(text: &str) -> Option<Self> {
        let (n, d) = text.split_once('/')?;
        let (num, den) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
        (den != 0).then_some(Self { num, den })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoStreamInfo {
    pub index: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub time_base: Rational,
    pub average_frame_rate: Option<Rational>,
    pub rotation_degrees: i32,
    /// Transfer characteristic, e.g. `bt709`, `smpte2084` (PQ HDR) or `arib-std-b67` (HLG).
    pub color_transfer: Option<String>,
    pub color_space: Option<String>,
    pub color_primaries: Option<String>,
    pub pix_fmt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProbeResult {
    pub container: String,
    pub duration_seconds: Option<f64>,
    /// First video stream, if any. For still images this is the image itself.
    pub video: Option<VideoStreamInfo>,
    pub has_audio: bool,
    /// Number of audio streams and the first one's codec.
    pub audio_streams: u32,
    pub audio_codec: Option<String>,
}

#[derive(Deserialize)]
struct RawProbe {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
}

#[derive(Deserialize)]
struct RawFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Deserialize)]
struct RawStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    time_base: Option<String>,
    avg_frame_rate: Option<String>,
    color_transfer: Option<String>,
    color_space: Option<String>,
    color_primaries: Option<String>,
    pix_fmt: Option<String>,
    #[serde(default)]
    side_data_list: Vec<serde_json::Value>,
    #[serde(default)]
    tags: serde_json::Map<String, serde_json::Value>,
}

fn require_input_file(path: &Path) -> Result<(), MediaError> {
    let meta = fs::metadata(path).map_err(io_err)?;
    if !meta.is_file() {
        return Err(MediaError::InvalidRequest("input is not a file".into()));
    }
    Ok(())
}

/// Inspect container and streams. Rejects files without a decodable picture stream.
pub fn probe(tools: &Tools, input: &Path) -> Result<ProbeResult, MediaError> {
    require_input_file(input)?;
    let mut args = os(&[
        "-v",
        "error",
        "-print_format",
        "json",
        "-show_streams",
        "-show_format",
        "-i",
    ]);
    args.push(input.into());
    let raw = run_bounded(&tools.ffprobe, &args, PROBE_TIMEOUT).map_err(|e| match e {
        MediaError::ToolFailed(m) => MediaError::Unsupported(m),
        other => other,
    })?;
    let parsed: RawProbe = serde_json::from_slice(&raw)
        .map_err(|e| MediaError::Unsupported(format!("unreadable probe output: {e}")))?;
    let video = parsed
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .map(video_info)
        .transpose()?;
    let format = parsed.format.as_ref();
    Ok(ProbeResult {
        container: format
            .and_then(|f| f.format_name.clone())
            .unwrap_or_default(),
        duration_seconds: format
            .and_then(|f| f.duration.as_deref())
            .and_then(|d| d.parse().ok()),
        has_audio: parsed
            .streams
            .iter()
            .any(|s| s.codec_type.as_deref() == Some("audio")),
        audio_streams: parsed
            .streams
            .iter()
            .filter(|s| s.codec_type.as_deref() == Some("audio"))
            .count() as u32,
        audio_codec: parsed
            .streams
            .iter()
            .find(|s| s.codec_type.as_deref() == Some("audio"))
            .and_then(|s| s.codec_name.clone()),
        video,
    })
}

fn video_info(stream: &RawStream) -> Result<VideoStreamInfo, MediaError> {
    let (width, height) = match (stream.width, stream.height) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => {
            return Err(MediaError::Unsupported(
                "video stream has no dimensions".into(),
            ))
        }
    };
    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(MediaError::Unsupported(format!(
            "{width}x{height} exceeds the {MAX_DIMENSION}px limit"
        )));
    }
    let rotation = stream
        .side_data_list
        .iter()
        .find_map(|d| d.get("rotation").and_then(|r| r.as_f64()))
        .or_else(|| {
            stream
                .tags
                .get("rotate")
                .and_then(|r| r.as_str())
                .and_then(|r| r.parse().ok())
        })
        .unwrap_or(0.0);
    Ok(VideoStreamInfo {
        index: stream.index,
        codec: stream.codec_name.clone().unwrap_or_default(),
        width,
        height,
        time_base: stream
            .time_base
            .as_deref()
            .and_then(Rational::parse)
            .ok_or_else(|| MediaError::Unsupported("video stream has no time base".into()))?,
        average_frame_rate: stream
            .avg_frame_rate
            .as_deref()
            .and_then(Rational::parse)
            .filter(|r| r.num > 0),
        rotation_degrees: rotation.round() as i32,
        color_transfer: stream.color_transfer.clone(),
        color_space: stream.color_space.clone(),
        color_primaries: stream.color_primaries.clone(),
        pix_fmt: stream.pix_fmt.clone(),
    })
}

/// One decoded frame's presentation timestamp in its stream time base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameStamp {
    /// Zero-based decode-order-independent index in presentation order.
    pub index: usize,
    pub pts: i64,
    pub time_base: Rational,
}

impl FrameStamp {
    pub fn seconds(&self) -> f64 {
        self.pts as f64 * self.time_base.num as f64 / self.time_base.den as f64
    }

    /// Microseconds rounded *down*, so an FFmpeg accurate seek lands on this frame, not the next.
    fn floor_micros(&self) -> Result<i64, MediaError> {
        let scaled = (self.pts as i128) * (self.time_base.num as i128) * 1_000_000;
        let den = self.time_base.den as i128;
        i64::try_from(scaled.div_euclid(den))
            .map_err(|_| MediaError::InvalidRequest("timestamp out of range".into()))
    }
}

#[derive(Deserialize)]
struct RawFrames {
    #[serde(default)]
    frames: Vec<RawFrame>,
}

#[derive(Deserialize)]
struct RawFrame {
    best_effort_timestamp: Option<i64>,
    pts: Option<i64>,
}

/// List the presentation timestamps of the first video stream's decoded frames (up to `limit`).
/// These are real frames, so variable frame rate footage is represented faithfully.
pub fn enumerate_frames(
    tools: &Tools,
    input: &Path,
    limit: usize,
) -> Result<Vec<FrameStamp>, MediaError> {
    if limit == 0 || limit > MAX_FRAMES_PER_REQUEST {
        return Err(MediaError::InvalidRequest(format!(
            "frame limit must be 1..={MAX_FRAMES_PER_REQUEST}"
        )));
    }
    let info = probe(tools, input)?
        .video
        .ok_or_else(|| MediaError::Unsupported("no video stream".into()))?;
    let mut args = os(&[
        "-v",
        "error",
        "-select_streams",
        "v:0",
        "-show_entries",
        "frame=best_effort_timestamp,pts",
        "-print_format",
        "json",
        "-read_intervals",
    ]);
    args.push(format!("%+#{limit}").into());
    args.push("-i".into());
    args.push(input.into());
    let raw = run_bounded(&tools.ffprobe, &args, RENDER_TIMEOUT)?;
    let parsed: RawFrames = serde_json::from_slice(&raw)
        .map_err(|e| MediaError::ToolFailed(format!("unreadable frame list: {e}")))?;
    let mut pts: Vec<i64> = parsed
        .frames
        .iter()
        .filter_map(|f| f.best_effort_timestamp.or(f.pts))
        .collect();
    pts.sort_unstable();
    pts.dedup();
    pts.truncate(limit);
    Ok(pts
        .into_iter()
        .enumerate()
        .map(|(index, pts)| FrameStamp {
            index,
            pts,
            time_base: info.time_base,
        })
        .collect())
}

/// Allowed still-image output formats, chosen by the output file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StillFormat {
    Png,
    Jpeg,
}

fn still_format(path: &Path) -> Result<StillFormat, MediaError> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Ok(StillFormat::Png),
        Some("jpg") | Some("jpeg") => Ok(StillFormat::Jpeg),
        _ => Err(MediaError::InvalidRequest(
            "output must end in .png, .jpg or .jpeg".into(),
        )),
    }
}

/// Validate an output path: a new file, in an existing directory, not the input.
fn prepare_output(input: &Path, output: &Path) -> Result<(StillFormat, PathBuf), MediaError> {
    let format = still_format(output)?;
    if fs::symlink_metadata(output).is_ok() {
        return Err(MediaError::InvalidRequest(
            "output already exists; refusing to overwrite".into(),
        ));
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)
        .map_err(|e| MediaError::InvalidRequest(format!("output folder unavailable: {e}")))?;
    let name = output
        .file_name()
        .ok_or_else(|| MediaError::InvalidRequest("output has no file name".into()))?;
    let resolved = parent.join(name);
    if fs::canonicalize(input).map_err(io_err)? == resolved {
        return Err(MediaError::InvalidRequest(
            "output must not be the input".into(),
        ));
    }
    Ok((format, resolved))
}

/// Encode into a temporary sibling, validate by decoding it back, then rename into place.
fn render_still(
    tools: &Tools,
    input: &Path,
    output: &Path,
    pre_input: Vec<OsString>,
    filter: Option<String>,
    expect: (u32, u32),
) -> Result<PathBuf, MediaError> {
    let (format, final_path) = prepare_output(input, output)?;
    let extension = match format {
        StillFormat::Png => "png",
        StillFormat::Jpeg => "jpg",
    };
    let temporary = final_path.with_file_name(format!(
        ".{}.optic-rift-partial.{extension}",
        uuid::Uuid::new_v4()
    ));
    let mut args = os(&["-v", "error", "-nostdin", "-y"]);
    args.extend(pre_input);
    args.push("-i".into());
    args.push(input.into());
    args.extend(os(&["-map", "0:v:0", "-frames:v", "1"]));
    if let Some(filter) = filter {
        args.push("-vf".into());
        args.push(filter.into());
    }
    match format {
        StillFormat::Png => args.extend(os(&["-c:v", "png", "-pix_fmt", "rgb24"])),
        StillFormat::Jpeg => {
            args.extend(os(&["-c:v", "mjpeg", "-q:v", "2", "-pix_fmt", "yuvj444p"]))
        }
    }
    args.extend(os(&["-f", "image2", "-update", "1"]));
    args.push(temporary.clone().into());

    let result = run_bounded(&tools.ffmpeg, &args, RENDER_TIMEOUT).and_then(|_| {
        let produced = probe(tools, &temporary)
            .map_err(|e| MediaError::OutputInvalid(e.to_string()))?
            .video
            .ok_or_else(|| MediaError::OutputInvalid("no picture in output".into()))?;
        if (produced.width, produced.height) != expect {
            return Err(MediaError::OutputInvalid(format!(
                "expected {}x{}, got {}x{}",
                expect.0, expect.1, produced.width, produced.height
            )));
        }
        // Atomic on the same volume; refuses nothing by itself, so re-check for a racing file.
        if fs::symlink_metadata(&final_path).is_ok() {
            return Err(MediaError::InvalidRequest(
                "output appeared while rendering; not overwritten".into(),
            ));
        }
        fs::rename(&temporary, &final_path).map_err(io_err)
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map(|_| final_path)
}

/// Decode exactly the frame at `frame` from the source video and save it losslessly as PNG
/// (or JPEG if asked). The output has the stream's coded size; display rotation is applied.
pub fn extract_frame(
    tools: &Tools,
    input: &Path,
    frame: FrameStamp,
    output: &Path,
) -> Result<PathBuf, MediaError> {
    let info = probe(tools, input)?
        .video
        .ok_or_else(|| MediaError::Unsupported("no video stream".into()))?;
    if info.time_base != frame.time_base || frame.pts < 0 {
        return Err(MediaError::InvalidRequest(
            "frame stamp does not belong to this video".into(),
        ));
    }
    let micros = frame.floor_micros()?;
    // Input-side -ss decodes from the preceding keyframe and discards frames before the
    // target, so the first emitted frame is the first with pts >= target (accurate seek).
    let pre = vec![OsString::from("-ss"), OsString::from(format!("{micros}us"))];
    let expect = if info.rotation_degrees.rem_euclid(180) == 90 {
        (info.height, info.width)
    } else {
        (info.width, info.height)
    };
    render_still(tools, input, output, pre, None, expect)
}

/// A non-destructive still-image recipe, applied in source pixel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ImageRecipe {
    /// Crop rectangle (x, y, width, height) in the upright source image; `None` keeps all.
    pub crop: Option<(u32, u32, u32, u32)>,
    /// Clockwise quarter turns applied after cropping (0..=3).
    pub quarter_turns: u8,
    /// Additive brightness in -1.0..=1.0 (0 = unchanged).
    pub brightness: f32,
    /// Contrast multiplier in 0.0..=4.0 (1 = unchanged).
    pub contrast: f32,
}

impl Default for ImageRecipe {
    fn default() -> Self {
        Self {
            crop: None,
            quarter_turns: 0,
            brightness: 0.0,
            contrast: 1.0,
        }
    }
}

/// Render a still image through `recipe` into a new file. The source is only read.
pub fn render_image(
    tools: &Tools,
    input: &Path,
    recipe: &ImageRecipe,
    output: &Path,
) -> Result<PathBuf, MediaError> {
    let info = probe(tools, input)?
        .video
        .ok_or_else(|| MediaError::Unsupported("no picture stream".into()))?;
    let (src_w, src_h) = if info.rotation_degrees.rem_euclid(180) == 90 {
        (info.height, info.width)
    } else {
        (info.width, info.height)
    };
    if recipe.quarter_turns > 3 {
        return Err(MediaError::InvalidRequest(
            "quarter_turns must be 0..=3".into(),
        ));
    }
    if !(-1.0..=1.0).contains(&recipe.brightness) || !(0.0..=4.0).contains(&recipe.contrast) {
        return Err(MediaError::InvalidRequest(
            "brightness or contrast out of range".into(),
        ));
    }
    let mut filters = Vec::new();
    let (mut w, mut h) = (src_w, src_h);
    if let Some((x, y, cw, ch)) = recipe.crop {
        let fits = cw > 0
            && ch > 0
            && x.checked_add(cw).is_some_and(|r| r <= src_w)
            && y.checked_add(ch).is_some_and(|b| b <= src_h);
        if !fits {
            return Err(MediaError::InvalidRequest(format!(
                "crop {cw}x{ch}+{x}+{y} is outside the {src_w}x{src_h} image"
            )));
        }
        filters.push(format!("crop={cw}:{ch}:{x}:{y}:exact=1"));
        (w, h) = (cw, ch);
    }
    if recipe.brightness != 0.0 || recipe.contrast != 1.0 {
        filters.push(format!(
            "eq=brightness={:.4}:contrast={:.4}",
            recipe.brightness, recipe.contrast
        ));
    }
    match recipe.quarter_turns {
        1 => filters.push("transpose=clock".into()),
        2 => filters.push("hflip,vflip".into()),
        3 => filters.push("transpose=cclock".into()),
        _ => {}
    }
    if recipe.quarter_turns % 2 == 1 {
        (w, h) = (h, w);
    }
    let filter = (!filters.is_empty()).then(|| filters.join(","));
    render_still(tools, input, output, Vec::new(), filter, (w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("optic-rift-media-{}", uuid::Uuid::new_v4()));
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

    /// FFmpeg is a host prerequisite for these tests; fail loudly rather than skip silently.
    fn tools() -> Tools {
        Tools::discover().expect("ffmpeg and ffprobe must be on PATH or in OPTIC_RIFT_FFMPEG_DIR")
    }

    fn ffmpeg(tools: &Tools, args: &[&str]) {
        run_bounded(&tools.ffmpeg, &os(args), RENDER_TIMEOUT).unwrap();
    }

    /// 60 frames at 30 fps whose luma encodes the frame number (N*4), lossless H.264 with
    /// B-frames and a keyframe every 7 frames so seeks must decode across GOPs.
    fn counter_video(tools: &Tools, dir: &Scratch, name: &str, rate: &str) -> PathBuf {
        let out = dir.join(name);
        let src = format!("color=c=black:s=64x48:r={rate}:d=2,format=gray,geq=lum='N*4'");
        ffmpeg(
            tools,
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                &src,
                "-c:v",
                "libx264",
                "-qp",
                "0",
                "-bf",
                "2",
                "-g",
                "7",
                "-pix_fmt",
                "yuv444p",
                out.to_str().unwrap(),
            ],
        );
        out
    }

    /// Decode an image and return its mean luma.
    fn mean_luma(tools: &Tools, image: &Path) -> f64 {
        let mut args = os(&["-v", "error", "-i"]);
        args.push(image.into());
        args.extend(os(&["-f", "rawvideo", "-pix_fmt", "gray", "-"]));
        let bytes = run_bounded(&tools.ffmpeg, &args, RENDER_TIMEOUT).unwrap();
        bytes.iter().map(|&b| b as f64).sum::<f64>() / bytes.len() as f64
    }

    /// Map a counter frame's mean luma back to its source frame number. Frames are 4 luma
    /// levels apart; full/limited range conversion moves values by at most ~1 level.
    fn source_frame(luma: f64) -> i64 {
        (luma / 4.0).round() as i64
    }

    #[test]
    fn probe_reports_video_audio_and_rejects_non_media() {
        let t = tools();
        let dir = Scratch::new();
        let video = dir.join("av.mp4");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=s=320x240:r=25:d=1",
                "-f",
                "lavfi",
                "-i",
                "sine=d=1",
                "-c:v",
                "libx264",
                "-c:a",
                "aac",
                "-shortest",
                video.to_str().unwrap(),
            ],
        );
        let probed = probe(&t, &video).unwrap();
        let v = probed.video.unwrap();
        assert_eq!((v.width, v.height, v.codec.as_str()), (320, 240, "h264"));
        assert_eq!(v.average_frame_rate, Some(Rational { num: 25, den: 1 }));
        assert!(probed.has_audio);
        assert!(probed.duration_seconds.unwrap() > 0.9);

        let text = dir.join("fake.png");
        fs::write(&text, b"not an image").unwrap();
        assert!(matches!(probe(&t, &text), Err(MediaError::Unsupported(_))));
        let noise = dir.join("noise.mp4");
        fs::write(
            &noise,
            (0..200_000u32)
                .map(|n| (n.wrapping_mul(2654435761) >> 24) as u8)
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(matches!(probe(&t, &noise), Err(MediaError::Unsupported(_))));
        assert!(probe(&t, &dir.join("missing.mp4")).is_err());
    }

    #[test]
    fn every_extracted_frame_is_the_requested_source_frame() {
        let t = tools();
        let dir = Scratch::new();
        let video = counter_video(&t, &dir, "counter.mp4", "30");
        let frames = enumerate_frames(&t, &video, 1000).unwrap();
        assert_eq!(frames.len(), 60);
        assert!(frames.windows(2).all(|w| w[0].pts < w[1].pts));
        // Every frame, including those right after keyframes and B-frames.
        let mut seen = Vec::new();
        for frame in &frames {
            let out = dir.join(&format!("f{}.png", frame.index));
            extract_frame(&t, &video, *frame, &out).unwrap();
            seen.push(source_frame(mean_luma(&t, &out)));
        }
        let wanted: Vec<i64> = (0..60).collect();
        assert_eq!(seen, wanted, "extracted source frame per requested index");
    }

    #[test]
    fn variable_frame_rate_frames_are_real_timestamps_not_a_nominal_grid() {
        let t = tools();
        let dir = Scratch::new();
        let cfr = counter_video(&t, &dir, "cfr.mp4", "30");
        // Drop every other frame in the first second only: a genuinely variable cadence.
        let vfr = dir.join("vfr.mkv");
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
                "yuv444p",
                vfr.to_str().unwrap(),
            ],
        );
        let frames = enumerate_frames(&t, &vfr, 1000).unwrap();
        assert_eq!(frames.len(), 45);
        let gaps: Vec<f64> = frames
            .windows(2)
            .map(|w| w[1].seconds() - w[0].seconds())
            .collect();
        assert!(gaps[0] > 0.06 && gaps[40] < 0.04, "gaps {gaps:?}");
        // VFR index 20 is source frame 35: indices 0..=14 are even source frames 0..=28,
        // then every source frame from 30 onward.
        let out = dir.join("vfr20.png");
        extract_frame(&t, &vfr, frames[20], &out).unwrap();
        assert_eq!(source_frame(mean_luma(&t, &out)), 35);
    }

    #[test]
    fn extraction_validates_requests_and_never_overwrites() {
        let t = tools();
        let dir = Scratch::new();
        let video = counter_video(&t, &dir, "v.mp4", "30");
        let frames = enumerate_frames(&t, &video, 5).unwrap();
        assert_eq!(frames.len(), 5);
        let original = fs::read(&video).unwrap();

        let existing = dir.join("taken.png");
        fs::write(&existing, b"keep me").unwrap();
        assert!(extract_frame(&t, &video, frames[1], &existing).is_err());
        assert_eq!(fs::read(&existing).unwrap(), b"keep me");
        assert!(extract_frame(&t, &video, frames[1], &dir.join("out.gif")).is_err());
        assert!(extract_frame(&t, &video, frames[1], &dir.join("no-such-dir/out.png")).is_err());
        let foreign = FrameStamp {
            time_base: Rational { num: 1, den: 7 },
            ..frames[1]
        };
        assert!(extract_frame(&t, &video, foreign, &dir.join("x.png")).is_err());
        assert!(enumerate_frames(&t, &video, 0).is_err());
        assert!(enumerate_frames(&t, &video, MAX_FRAMES_PER_REQUEST + 1).is_err());

        // A frame beyond the end produces no output and no leftover partial file.
        let beyond = FrameStamp {
            pts: frames[4].pts * 1000,
            ..frames[4]
        };
        assert!(extract_frame(&t, &video, beyond, &dir.join("beyond.png")).is_err());
        let leftovers = fs::read_dir(&dir.0)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("partial")
            })
            .count();
        assert_eq!(leftovers, 0);
        assert!(!dir.join("beyond.png").exists());
        assert_eq!(fs::read(&video).unwrap(), original, "source untouched");
    }

    #[test]
    fn frame_to_cropped_adjusted_photo_journey() {
        let t = tools();
        let dir = Scratch::new();
        // 160x90 video: left half luma 40, right half luma 200.
        let video = dir.join("halves.mp4");
        ffmpeg(
            &t,
            &[
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=160x90:r=24:d=1,format=gray,geq=lum='if(lt(X\\,80)\\,40\\,200)'",
                "-c:v",
                "libx264",
                "-qp",
                "0",
                "-pix_fmt",
                "yuv444p",
                video.to_str().unwrap(),
            ],
        );
        let frames = enumerate_frames(&t, &video, 100).unwrap();
        let still = dir.join("frame.png");
        extract_frame(&t, &video, frames[12], &still).unwrap();

        let recipe = ImageRecipe {
            crop: Some((90, 10, 60, 40)),
            quarter_turns: 1,
            brightness: 0.0,
            contrast: 1.0,
        };
        let cropped = dir.join("crop.png");
        render_image(&t, &still, &recipe, &cropped).unwrap();
        let v = probe(&t, &cropped).unwrap().video.unwrap();
        assert_eq!((v.width, v.height), (40, 60), "rotated after crop");
        assert!(
            (mean_luma(&t, &cropped) - 200.0).abs() < 2.0,
            "crop came from the right half"
        );

        let darker = dir.join("darker.jpg");
        let recipe = ImageRecipe {
            crop: Some((90, 10, 60, 40)),
            brightness: -0.3,
            ..Default::default()
        };
        render_image(&t, &still, &recipe, &darker).unwrap();
        let luma = mean_luma(&t, &darker);
        assert!(
            luma < 150.0 && luma > 80.0,
            "brightness applied, got {luma}"
        );

        let bad = ImageRecipe {
            crop: Some((150, 0, 20, 20)),
            ..Default::default()
        };
        assert!(matches!(
            render_image(&t, &still, &bad, &dir.join("bad.png")),
            Err(MediaError::InvalidRequest(_))
        ));
        let bad = ImageRecipe {
            crop: Some((u32::MAX, 0, 2, 2)),
            ..Default::default()
        };
        assert!(render_image(&t, &still, &bad, &dir.join("bad2.png")).is_err());
        assert!(render_image(
            &t,
            &still,
            &ImageRecipe {
                quarter_turns: 4,
                ..Default::default()
            },
            &dir.join("bad3.png")
        )
        .is_err());
        assert!(
            render_image(&t, &still, &ImageRecipe::default(), &still).is_err(),
            "never overwrite the input"
        );
    }

    /// Manual measurement: `OPTIC_RIFT_BENCH_VIDEO=<file> cargo test -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_exact_frame_extraction_latency() {
        let t = tools();
        let video = PathBuf::from(
            std::env::var("OPTIC_RIFT_BENCH_VIDEO").expect("set OPTIC_RIFT_BENCH_VIDEO"),
        );
        let dir = Scratch::new();
        let started = Instant::now();
        let frames = enumerate_frames(&t, &video, MAX_FRAMES_PER_REQUEST).unwrap();
        println!(
            "enumerated {} frames in {:?}",
            frames.len(),
            started.elapsed()
        );
        let step = (frames.len() / 40).max(1);
        let mut samples: Vec<Duration> = frames
            .iter()
            .step_by(step)
            .map(|frame| {
                let started = Instant::now();
                extract_frame(
                    &t,
                    &video,
                    *frame,
                    &dir.join(&format!("{}.png", frame.index)),
                )
                .unwrap();
                started.elapsed()
            })
            .collect();
        samples.sort();
        let pick = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
        println!(
            "exact-frame PNG extraction n={} p50={:?} p95={:?} max={:?}",
            samples.len(),
            pick(0.5),
            pick(0.95),
            samples[samples.len() - 1]
        );
    }
}
