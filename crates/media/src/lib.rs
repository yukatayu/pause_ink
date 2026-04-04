use std::path::{Path, PathBuf};
use std::process::Command;

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
            PathBuf::from("ffmpeg"),
            PathBuf::from("ffprobe"),
            RuntimeOrigin::SystemHost,
        )
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
    Parse(#[from] serde_json::Error),
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

pub fn parse_ffprobe_output(json: &str) -> Result<MediaProbe, MediaError> {
    let payload: FfprobePayload = serde_json::from_str(json)?;
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
    use std::path::PathBuf;

    use super::*;

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
}
