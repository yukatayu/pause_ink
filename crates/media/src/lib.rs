use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use pauseink_domain::{MediaDuration, MediaTime, Point2};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRuntime {
    pub ffmpeg_path: PathBuf,
    pub ffprobe_path: PathBuf,
    pub origin: RuntimeOrigin,
    pub manifest_path: Option<PathBuf>,
    pub build_summary: Option<String>,
    pub license_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeOrigin {
    Sidecar,
    SystemHost,
    TestFixture,
}

impl MediaRuntime {
    pub fn from_paths(ffmpeg_path: PathBuf, ffprobe_path: PathBuf, origin: RuntimeOrigin) -> Self {
        Self {
            ffmpeg_path,
            ffprobe_path,
            origin,
            manifest_path: None,
            build_summary: None,
            license_summary: None,
        }
    }

    pub fn from_system_path() -> Self {
        Self::from_paths(
            PathBuf::from(ffmpeg_binary_name()),
            PathBuf::from(ffprobe_binary_name()),
            RuntimeOrigin::SystemHost,
        )
    }
}

pub fn ffmpeg_binary_name() -> &'static str {
    if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    }
}

pub fn ffprobe_binary_name() -> &'static str {
    if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    }
}

pub fn default_platform_id() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn sidecar_runtime_dir(runtime_root: &Path, platform_id: &str) -> PathBuf {
    runtime_root.join("ffmpeg").join(platform_id)
}

pub fn discover_sidecar_runtime(
    runtime_root: &Path,
    platform_id: &str,
) -> Result<MediaRuntime, MediaError> {
    let runtime_dir = sidecar_runtime_dir(runtime_root, platform_id);
    let ffmpeg_path = runtime_dir.join(ffmpeg_binary_name());
    let ffprobe_path = runtime_dir.join(ffprobe_binary_name());
    let manifest_path = runtime_dir.join("manifest.json");

    ensure_file_exists(&ffmpeg_path, "ffmpeg binary")?;
    ensure_file_exists(&ffprobe_path, "ffprobe binary")?;
    ensure_file_exists(&manifest_path, "runtime manifest")?;

    let manifest_raw = fs::read_to_string(&manifest_path)?;
    let manifest: RuntimeManifest = serde_json::from_str(&manifest_raw)
        .map_err(MediaError::ManifestParse)?;

    Ok(MediaRuntime {
        ffmpeg_path,
        ffprobe_path,
        origin: RuntimeOrigin::Sidecar,
        manifest_path: Some(manifest_path),
        build_summary: manifest.build_summary(),
        license_summary: manifest.license_summary,
    })
}

pub fn discover_system_runtime() -> Result<MediaRuntime, MediaError> {
    let ffmpeg_version = capture_version_output(Path::new(ffmpeg_binary_name()))?;
    let ffprobe_version = capture_version_output(Path::new(ffprobe_binary_name()))?;

    Ok(MediaRuntime {
        ffmpeg_path: PathBuf::from(ffmpeg_binary_name()),
        ffprobe_path: PathBuf::from(ffprobe_binary_name()),
        origin: RuntimeOrigin::SystemHost,
        manifest_path: None,
        build_summary: Some(format!(
            "{} | {}",
            ffmpeg_version.first_line, ffprobe_version.first_line
        )),
        license_summary: Some(system_license_summary(&ffmpeg_version.full_output)),
    })
}

pub fn discover_runtime(
    runtime_root: &Path,
    platform_id: &str,
    allow_system_fallback: bool,
) -> Result<MediaRuntime, MediaError> {
    match discover_sidecar_runtime(runtime_root, platform_id) {
        Ok(runtime) => Ok(runtime),
        Err(sidecar_error) if allow_system_fallback => {
            discover_system_runtime().map_err(|system_error| {
                MediaError::RuntimeUnavailable(format!(
                    "sidecar discovery failed: {sidecar_error}; system fallback failed: {system_error}"
                ))
            })
        }
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaProbe {
    pub format_name: Option<String>,
    pub duration_seconds: Option<f64>,
    pub duration_raw: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub frame_rate: Option<f64>,
    pub avg_frame_rate_raw: Option<String>,
    pub r_frame_rate_raw: Option<String>,
    pub pix_fmt: Option<String>,
    pub has_alpha: bool,
    pub has_audio: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub support: MediaSupport,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MediaSupport {
    Supported,
    SupportedWithCaveats(Vec<String>),
    Unsupported(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub video_encoders: Vec<String>,
    pub audio_encoders: Vec<String>,
    pub muxers: Vec<String>,
    pub hwaccels: Vec<String>,
}

impl RuntimeCapabilities {
    pub fn from_outputs(encoders_output: &str, muxers_output: &str, hwaccels_output: &str) -> Self {
        Self {
            video_encoders: parse_encoder_names(encoders_output, 'V'),
            audio_encoders: parse_encoder_names(encoders_output, 'A'),
            muxers: parse_name_list(muxers_output, " E "),
            hwaccels: parse_hwaccels(hwaccels_output),
        }
    }
}

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("ffprobe execution failed: {0}")]
    CommandFailed(String),
    #[error("ffprobe output parse failed: {0}")]
    ProbeParse(serde_json::Error),
    #[error("runtime manifest parse failed: {0}")]
    ManifestParse(serde_json::Error),
    #[error("media runtime unavailable: {0}")]
    RuntimeUnavailable(String),
    #[error("ffprobe I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub trait MediaProvider {
    fn probe(&self, source_path: &Path) -> Result<MediaProbe, MediaError>;
    fn capabilities(&self) -> Result<RuntimeCapabilities, MediaError>;
    fn diagnostics(&self) -> MediaRuntime;
}

pub struct FfprobeMediaProvider {
    runtime: MediaRuntime,
}

impl FfprobeMediaProvider {
    pub fn new(runtime: MediaRuntime) -> Self {
        Self { runtime }
    }
}

impl MediaProvider for FfprobeMediaProvider {
    fn probe(&self, source_path: &Path) -> Result<MediaProbe, MediaError> {
        let output = Command::new(&self.runtime.ffprobe_path)
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-of",
                "json",
            ])
            .arg(source_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(MediaError::CommandFailed(stderr));
        }

        parse_ffprobe_output(&String::from_utf8_lossy(&output.stdout))
    }

    fn capabilities(&self) -> Result<RuntimeCapabilities, MediaError> {
        let encoders = run_ffmpeg_query(&self.runtime.ffmpeg_path, "-encoders")?;
        let muxers = run_ffmpeg_query(&self.runtime.ffmpeg_path, "-muxers")?;
        let hwaccels = run_ffmpeg_query(&self.runtime.ffmpeg_path, "-hwaccels")?;

        Ok(RuntimeCapabilities::from_outputs(
            &encoders, &muxers, &hwaccels,
        ))
    }

    fn diagnostics(&self) -> MediaRuntime {
        self.runtime.clone()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedMedia {
    pub source_path: PathBuf,
    pub probe: MediaProbe,
}

impl ImportedMedia {
    pub fn duration(&self) -> Option<MediaDuration> {
        self.probe.duration_seconds.map(|seconds| {
            MediaDuration::from_millis((seconds * 1_000.0).round() as i64)
        })
    }
}

pub fn import_media(
    provider: &dyn MediaProvider,
    source_path: &Path,
) -> Result<ImportedMedia, MediaError> {
    Ok(ImportedMedia {
        source_path: source_path.to_path_buf(),
        probe: provider.probe(source_path)?,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackState {
    pub media: ImportedMedia,
    pub current_time: MediaTime,
    pub is_playing: bool,
}

impl PlaybackState {
    pub fn new(media: ImportedMedia) -> Self {
        Self {
            media,
            current_time: MediaTime::from_millis(0),
            is_playing: false,
        }
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn seek(&mut self, time: MediaTime) {
        let clamped = if time.ticks < 0 {
            MediaTime::new(0, time.time_base)
        } else if let Some(duration) = self.media.duration() {
            let duration_time = MediaTime::new(duration.ticks, duration.time_base);
            if time > duration_time {
                duration_time
            } else {
                time
            }
        } else {
            time
        };

        self.current_time = clamped;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasSize {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub fn fit_frame_to_canvas(
    frame_width: u32,
    frame_height: u32,
    canvas: CanvasSize,
) -> Option<CanvasRect> {
    if frame_width == 0 || frame_height == 0 || canvas.width <= 0.0 || canvas.height <= 0.0 {
        return None;
    }

    let frame_width = frame_width as f32;
    let frame_height = frame_height as f32;
    let scale = (canvas.width / frame_width).min(canvas.height / frame_height);
    let width = frame_width * scale;
    let height = frame_height * scale;

    Some(CanvasRect {
        x: (canvas.width - width) / 2.0,
        y: (canvas.height - height) / 2.0,
        width,
        height,
    })
}

pub fn canvas_point_to_frame(
    point: Point2,
    frame_rect: CanvasRect,
    frame_width: u32,
    frame_height: u32,
) -> Option<Point2> {
    if point.x < frame_rect.x
        || point.y < frame_rect.y
        || point.x > frame_rect.x + frame_rect.width
        || point.y > frame_rect.y + frame_rect.height
        || frame_rect.width <= 0.0
        || frame_rect.height <= 0.0
    {
        return None;
    }

    let normalized_x = (point.x - frame_rect.x) / frame_rect.width;
    let normalized_y = (point.y - frame_rect.y) / frame_rect.height;

    Some(Point2 {
        x: normalized_x * frame_width as f32,
        y: normalized_y * frame_height as f32,
    })
}

pub fn frame_point_to_canvas(
    point: Point2,
    frame_rect: CanvasRect,
    frame_width: u32,
    frame_height: u32,
) -> Option<Point2> {
    if frame_width == 0
        || frame_height == 0
        || point.x < 0.0
        || point.y < 0.0
        || point.x > frame_width as f32
        || point.y > frame_height as f32
    {
        return None;
    }

    Some(Point2 {
        x: frame_rect.x + (point.x / frame_width as f32) * frame_rect.width,
        y: frame_rect.y + (point.y / frame_height as f32) * frame_rect.height,
    })
}

pub fn parse_ffprobe_output(json: &str) -> Result<MediaProbe, MediaError> {
    let payload: FfprobePayload =
        serde_json::from_str(json).map_err(MediaError::ProbeParse)?;
    let video_stream = payload.streams.iter().find(|stream| stream.codec_type == "video");
    let audio_stream = payload.streams.iter().find(|stream| stream.codec_type == "audio");

    let frame_rate = video_stream
        .and_then(|stream| stream.avg_frame_rate.as_deref())
        .and_then(parse_rational);

    let support = match video_stream {
        None => MediaSupport::Unsupported("video stream missing".to_owned()),
        Some(_) if frame_rate.is_none() => {
            MediaSupport::SupportedWithCaveats(vec!["unknown_frame_rate".to_owned()])
        }
        Some(_) => MediaSupport::Supported,
    };

    Ok(MediaProbe {
        format_name: payload.format.format_name,
        duration_seconds: payload
            .format
            .duration
            .as_deref()
            .and_then(|value| value.parse::<f64>().ok()),
        duration_raw: payload.format.duration,
        width: video_stream.and_then(|stream| stream.width),
        height: video_stream.and_then(|stream| stream.height),
        frame_rate,
        avg_frame_rate_raw: video_stream.and_then(|stream| stream.avg_frame_rate.clone()),
        r_frame_rate_raw: video_stream.and_then(|stream| stream.r_frame_rate.clone()),
        pix_fmt: video_stream.and_then(|stream| stream.pix_fmt.clone()),
        has_alpha: video_stream
            .and_then(|stream| stream.pix_fmt.as_deref())
            .map(pix_fmt_has_alpha)
            .unwrap_or(false),
        has_audio: audio_stream.is_some(),
        video_codec: video_stream.and_then(|stream| stream.codec_name.clone()),
        audio_codec: audio_stream.and_then(|stream| stream.codec_name.clone()),
        support,
    })
}

fn parse_rational(raw: &str) -> Option<f64> {
    let (numerator, denominator) = raw.split_once('/')?;
    let numerator = numerator.parse::<f64>().ok()?;
    let denominator = denominator.parse::<f64>().ok()?;

    if denominator == 0.0 {
        return None;
    }

    Some(numerator / denominator)
}

fn pix_fmt_has_alpha(pix_fmt: &str) -> bool {
    pix_fmt.contains("rgba") || pix_fmt.contains("yuva") || pix_fmt.contains("argb")
}

fn run_ffmpeg_query(binary_path: &Path, flag: &str) -> Result<String, MediaError> {
    let output = Command::new(binary_path).arg(flag).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(MediaError::CommandFailed(stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_encoder_names(output: &str, media_kind: char) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let prefix = trimmed.chars().next()?;
            if prefix != media_kind {
                return None;
            }
            trimmed.split_whitespace().nth(1).map(str::to_owned)
        })
        .collect()
}

fn parse_name_list(output: &str, marker: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if !trimmed.starts_with(marker.trim_start()) {
                return None;
            }
            trimmed.split_whitespace().nth(1).map(str::to_owned)
        })
        .collect()
}

fn parse_hwaccels(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("Hardware acceleration methods")
                && !line.starts_with("ffmpeg")
        })
        .map(str::to_owned)
        .collect()
}

fn ensure_file_exists(path: &Path, label: &str) -> Result<(), MediaError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(MediaError::RuntimeUnavailable(format!(
            "{label} not found at {}",
            path.display()
        )))
    }
}

fn capture_version_output(binary_path: &Path) -> Result<VersionOutput, MediaError> {
    let output = Command::new(binary_path).arg("-version").output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(MediaError::CommandFailed(stderr));
    }

    let full_output = String::from_utf8_lossy(&output.stdout).into_owned();
    let first_line = full_output
        .lines()
        .next()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or("unknown version")
        .to_owned();

    Ok(VersionOutput {
        first_line,
        full_output,
    })
}

fn system_license_summary(version_output: &str) -> String {
    if version_output.contains("--enable-gpl") {
        "host system runtime; ffmpeg build reports --enable-gpl".to_owned()
    } else {
        "host system runtime; packaging/license review still required".to_owned()
    }
}

#[derive(Debug)]
struct VersionOutput {
    first_line: String,
    full_output: String,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeManifest {
    build_summary: Option<String>,
    license_summary: Option<String>,
    version: Option<String>,
    source: Option<String>,
}

impl RuntimeManifest {
    fn build_summary(&self) -> Option<String> {
        self.build_summary
            .clone()
            .or_else(|| self.version.as_ref().map(|version| format!("sidecar runtime {version}")))
            .or_else(|| self.source.clone())
    }
}

#[derive(Debug, Deserialize)]
struct FfprobePayload {
    #[serde(default)]
    format: FfprobeFormat,
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeStream {
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    pix_fmt: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    use pauseink_domain::MediaTime;
    use super::*;
    use tempfile::tempdir;

    struct MockMediaProvider {
        probe: MediaProbe,
        capabilities: RuntimeCapabilities,
        diagnostics: MediaRuntime,
    }

    impl MediaProvider for MockMediaProvider {
        fn probe(&self, _source_path: &Path) -> Result<MediaProbe, MediaError> {
            Ok(self.probe.clone())
        }

        fn capabilities(&self) -> Result<RuntimeCapabilities, MediaError> {
            Ok(self.capabilities.clone())
        }

        fn diagnostics(&self) -> MediaRuntime {
            self.diagnostics.clone()
        }
    }

    #[test]
    fn probe_parser_extracts_video_summary() {
        let probe = parse_ffprobe_output(
            r#"{
              "format": {
                "filename": "demo.mp4",
                "duration": "5.000000",
                "format_name": "mov,mp4,m4a,3gp,3g2,mj2"
              },
              "streams": [
                {
                  "index": 0,
                  "codec_type": "video",
                  "codec_name": "h264",
                  "width": 1920,
                  "height": 1080,
                  "avg_frame_rate": "30000/1001"
                },
                {
                  "index": 1,
                  "codec_type": "audio",
                  "codec_name": "aac"
                }
              ]
            }"#,
        )
        .expect("valid ffprobe json should parse");

        assert_eq!(probe.width, Some(1920));
        assert_eq!(probe.height, Some(1080));
        assert_eq!(probe.video_codec.as_deref(), Some("h264"));
        assert_eq!(probe.audio_codec.as_deref(), Some("aac"));
        assert_eq!(probe.support, MediaSupport::Supported);
        assert!(probe.frame_rate.expect("fps should exist") > 29.9);
    }

    #[test]
    fn probe_parser_marks_audio_only_files_as_unsupported_for_annotation_video_flow() {
        let probe = parse_ffprobe_output(
            r#"{
              "format": {
                "filename": "audio.wav",
                "duration": "2.000000",
                "format_name": "wav"
              },
              "streams": [
                {
                  "index": 0,
                  "codec_type": "audio",
                  "codec_name": "pcm_s16le"
                }
              ]
            }"#,
        )
        .expect("audio-only json should still parse");

        assert_eq!(
            probe.support,
            MediaSupport::Unsupported("video stream missing".into())
        );
    }

    #[test]
    fn runtime_discovery_prefers_explicit_paths() {
        let runtime = MediaRuntime::from_paths(
            PathBuf::from("/tmp/custom-ffmpeg"),
            PathBuf::from("/tmp/custom-ffprobe"),
            RuntimeOrigin::SystemHost,
        );

        assert_eq!(runtime.ffmpeg_path, PathBuf::from("/tmp/custom-ffmpeg"));
        assert_eq!(runtime.ffprobe_path, PathBuf::from("/tmp/custom-ffprobe"));
        assert_eq!(runtime.origin, RuntimeOrigin::SystemHost);
    }

    #[test]
    fn sidecar_runtime_discovery_reads_manifest_and_layout() {
        let temp_dir = tempdir().expect("temp dir");
        let platform_id = "linux-x86_64";
        let runtime_dir = sidecar_runtime_dir(temp_dir.path(), platform_id);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        fs::write(runtime_dir.join(ffmpeg_binary_name()), b"").expect("ffmpeg placeholder");
        fs::write(runtime_dir.join(ffprobe_binary_name()), b"").expect("ffprobe placeholder");
        fs::write(
            runtime_dir.join("manifest.json"),
            r#"{
              "build_summary": "PauseInk sidecar test runtime",
              "license_summary": "MIT-friendly test runtime"
            }"#,
        )
        .expect("manifest");

        let runtime =
            discover_sidecar_runtime(temp_dir.path(), platform_id).expect("sidecar should resolve");

        assert_eq!(runtime.origin, RuntimeOrigin::Sidecar);
        assert_eq!(
            runtime.build_summary.as_deref(),
            Some("PauseInk sidecar test runtime")
        );
        assert_eq!(
            runtime.license_summary.as_deref(),
            Some("MIT-friendly test runtime")
        );
        assert_eq!(
            runtime.manifest_path.as_deref(),
            Some(runtime_dir.join("manifest.json").as_path())
        );
    }

    #[test]
    fn runtime_discovery_prefers_sidecar_before_system_fallback() {
        let temp_dir = tempdir().expect("temp dir");
        let platform_id = "linux-x86_64";
        let runtime_dir = sidecar_runtime_dir(temp_dir.path(), platform_id);
        fs::create_dir_all(&runtime_dir).expect("runtime dir");
        fs::write(runtime_dir.join(ffmpeg_binary_name()), b"").expect("ffmpeg placeholder");
        fs::write(runtime_dir.join(ffprobe_binary_name()), b"").expect("ffprobe placeholder");
        fs::write(
            runtime_dir.join("manifest.json"),
            r#"{
              "version": "1.0.0",
              "source": "https://example.invalid/runtime"
            }"#,
        )
        .expect("manifest");

        let runtime =
            discover_runtime(temp_dir.path(), platform_id, true).expect("runtime should resolve");

        assert_eq!(runtime.origin, RuntimeOrigin::Sidecar);
        assert_eq!(runtime.build_summary.as_deref(), Some("sidecar runtime 1.0.0"));
    }

    #[test]
    fn probe_parser_keeps_raw_timing_and_alpha_metadata() {
        let probe = parse_ffprobe_output(
            r#"{
              "format": {
                "duration": "3.500000",
                "format_name": "mov"
              },
              "streams": [
                {
                  "codec_type": "video",
                  "codec_name": "prores",
                  "width": 1280,
                  "height": 720,
                  "pix_fmt": "yuva444p10le",
                  "avg_frame_rate": "0/0",
                  "r_frame_rate": "24000/1001"
                }
              ]
            }"#,
        )
        .expect("valid ffprobe json should parse");

        assert_eq!(probe.avg_frame_rate_raw.as_deref(), Some("0/0"));
        assert_eq!(probe.r_frame_rate_raw.as_deref(), Some("24000/1001"));
        assert!(probe.has_alpha);
        assert_eq!(
            probe.support,
            MediaSupport::SupportedWithCaveats(vec!["unknown_frame_rate".into()])
        );
    }

    #[test]
    fn capability_parsers_extract_encoders_muxers_and_hwaccels() {
        let capabilities = RuntimeCapabilities::from_outputs(
            r#"
Encoders:
 V....D libvpx-vp9           libvpx VP9
 V....D libx264              libx264 H.264
 A....D libopus              libopus Opus
"#,
            r#"
Muxers:
 E webm            WebM
 E mov             QuickTime / MOV
"#,
            r#"
Hardware acceleration methods:
vaapi
cuda
"#,
        );

        assert!(capabilities.video_encoders.contains(&"libvpx-vp9".to_owned()));
        assert!(capabilities.video_encoders.contains(&"libx264".to_owned()));
        assert!(capabilities.audio_encoders.contains(&"libopus".to_owned()));
        assert!(capabilities.muxers.contains(&"webm".to_owned()));
        assert!(capabilities.hwaccels.contains(&"vaapi".to_owned()));
    }

    #[test]
    fn import_media_keeps_probe_and_support_classification() {
        let provider = MockMediaProvider {
            probe: MediaProbe {
                format_name: Some("mov,mp4,m4a,3gp,3g2,mj2".into()),
                duration_seconds: Some(12.0),
                duration_raw: Some("12.000000".into()),
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(30.0),
                avg_frame_rate_raw: Some("30/1".into()),
                r_frame_rate_raw: Some("30/1".into()),
                pix_fmt: Some("yuv420p".into()),
                has_alpha: false,
                has_audio: true,
                video_codec: Some("h264".into()),
                audio_codec: Some("aac".into()),
                support: MediaSupport::SupportedWithCaveats(vec!["vfr".into()]),
            },
            capabilities: RuntimeCapabilities::default(),
            diagnostics: MediaRuntime::from_system_path(),
        };

        let imported =
            import_media(&provider, Path::new("sample.mp4")).expect("import should succeed");

        assert_eq!(imported.source_path, PathBuf::from("sample.mp4"));
        assert_eq!(
            imported.probe.support,
            MediaSupport::SupportedWithCaveats(vec!["vfr".into()])
        );
        assert_eq!(imported.duration(), Some(MediaDuration::from_millis(12_000)));
    }

    #[test]
    fn playback_state_clamps_seek_and_toggles_play_pause() {
        let imported = ImportedMedia {
            source_path: PathBuf::from("sample.mp4"),
            probe: MediaProbe {
                format_name: Some("mp4".into()),
                duration_seconds: Some(5.0),
                duration_raw: Some("5.000000".into()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                avg_frame_rate_raw: Some("30/1".into()),
                r_frame_rate_raw: Some("30/1".into()),
                pix_fmt: Some("yuv420p".into()),
                has_alpha: false,
                has_audio: true,
                video_codec: Some("h264".into()),
                audio_codec: Some("aac".into()),
                support: MediaSupport::Supported,
            },
        };
        let mut playback = PlaybackState::new(imported);

        playback.play();
        assert!(playback.is_playing);

        playback.seek(MediaTime::from_millis(7_000));
        assert_eq!(playback.current_time, MediaTime::from_millis(5_000));

        playback.seek(MediaTime::from_millis(-100));
        assert_eq!(playback.current_time, MediaTime::from_millis(0));

        playback.pause();
        assert!(!playback.is_playing);
    }

    #[test]
    fn frame_canvas_mapping_letterboxes_and_roundtrips_points() {
        let frame_rect = fit_frame_to_canvas(
            1920,
            1080,
            CanvasSize {
                width: 1000.0,
                height: 1000.0,
            },
        )
        .expect("mapping should exist");

        assert!(frame_rect.x.abs() < 0.01);
        assert!((frame_rect.y - 218.75).abs() < 0.01);
        assert!((frame_rect.width - 1000.0).abs() < 0.01);
        assert!((frame_rect.height - 562.5).abs() < 0.01);

        let canvas_point = frame_point_to_canvas(
            Point2 { x: 960.0, y: 540.0 },
            frame_rect,
            1920,
            1080,
        )
        .expect("frame point should map");
        let roundtrip = canvas_point_to_frame(canvas_point, frame_rect, 1920, 1080)
            .expect("canvas point should roundtrip");

        assert!((roundtrip.x - 960.0).abs() < 0.01);
        assert!((roundtrip.y - 540.0).abs() < 0.01);
    }

    #[test]
    fn host_ffprobe_smoke_if_host_runtime_exists() {
        let runtime = match discover_system_runtime() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };

        let temp_dir = tempdir().expect("temp dir");
        let sample_path = temp_dir.path().join("probe-smoke.avi");
        let output = Command::new(&runtime.ffmpeg_path)
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x180:d=1:r=30",
                "-an",
                "-c:v",
                "mjpeg",
            ])
            .arg(&sample_path)
            .output()
            .expect("ffmpeg smoke command should run");

        assert!(
            output.status.success(),
            "ffmpeg smoke fixture creation should succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let provider = FfprobeMediaProvider::new(runtime);
        let probe = provider
            .probe(&sample_path)
            .expect("generated fixture should be probeable");
        let imported = import_media(&provider, &sample_path).expect("import should succeed");

        assert_eq!(probe.width, Some(320));
        assert_eq!(probe.height, Some(180));
        assert_eq!(probe.video_codec.as_deref(), Some("mjpeg"));
        assert_eq!(imported.probe.width, Some(320));
    }
}
