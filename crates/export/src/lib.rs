use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::{ImageBuffer, Rgba};
use pauseink_domain::{AnnotationProject, MediaDuration, MediaTime, TimeBase};
use pauseink_media::{MediaRuntime, RuntimeCapabilities};
use pauseink_presets_core::{DistributionProfile, ExportCatalog, ExportFamily, ResolveError};
use pauseink_renderer::{render_overlay_rgba, RenderRequest};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct ExportRequest {
    pub family_id: String,
    pub profile_id: String,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub has_audio: bool,
    pub requires_alpha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteExportSettings {
    pub family: ExportFamily,
    pub profile: DistributionProfile,
    pub selected_bucket_id: String,
    pub target_video_bitrate_kbps: Option<u32>,
    pub max_video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub keyframe_interval_seconds: Option<u32>,
    pub preferred_audio_codecs: Vec<String>,
    pub audio_enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportSnapshot {
    pub project: AnnotationProject,
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    pub duration: MediaDuration,
    pub source_media_path: Option<PathBuf>,
    pub has_audio: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportExecutionRequest {
    pub family_id: String,
    pub profile_id: String,
    pub output_path: PathBuf,
    pub transparent: bool,
    pub working_directory: PathBuf,
    pub prefer_hardware: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportOutputRequest {
    pub output_path: PathBuf,
    pub transparent: bool,
    pub working_directory: PathBuf,
    pub prefer_hardware: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportExecutionResult {
    pub output_path: PathBuf,
    pub frame_count: usize,
    pub settings: ConcreteExportSettings,
    pub software_fallback_used: bool,
}

#[derive(Debug, Error)]
pub enum ExportPlanError {
    #[error("transparent output requested, but family does not support alpha: {family_id}")]
    AlphaUnsupported { family_id: String },
    #[error("no settings bucket matched profile {profile_id}; tried {candidates:?}")]
    MissingSettingsBucket {
        profile_id: String,
        candidates: Vec<String>,
    },
    #[error("runtime lacks required {capability_kind}: {name}")]
    MissingCapability {
        capability_kind: &'static str,
        name: String,
    },
    #[error("profile selection failed: {0}")]
    Resolve(#[from] ResolveError),
}

#[derive(Debug, Error)]
pub enum ExportExecutionError {
    #[error("export planning failed: {0}")]
    Plan(#[from] ExportPlanError),
    #[error("export requires source media for composite output")]
    MissingSourceMedia,
    #[error("export output path is invalid for the selected family")]
    InvalidOutputPath,
    #[error("export filesystem I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("overlay render failed: {0}")]
    Render(String),
    #[error("image encoding failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("ffmpeg export command failed: {0}")]
    CommandFailed(String),
}

pub fn plan_export(
    catalog: &ExportCatalog,
    request: &ExportRequest,
    capabilities: Option<&RuntimeCapabilities>,
) -> Result<ConcreteExportSettings, ExportPlanError> {
    let resolved = catalog.resolve(&request.family_id, &request.profile_id)?;
    if request.requires_alpha && !resolved.family.supports_alpha {
        return Err(ExportPlanError::AlphaUnsupported {
            family_id: resolved.family.id,
        });
    }

    if let Some(capabilities) = capabilities {
        validate_family_capabilities(&resolved.family, request.has_audio, capabilities)?;
    }

    let candidates = bucket_candidates(request.width, request.height, request.frame_rate);
    let (selected_bucket_id, template) = candidates
        .iter()
        .find_map(|bucket_id| {
            resolved
                .profile
                .setting_bucket(bucket_id)
                .cloned()
                .map(|template| (bucket_id.clone(), template))
        })
        .ok_or_else(|| ExportPlanError::MissingSettingsBucket {
            profile_id: resolved.profile.id.clone(),
            candidates: candidates.clone(),
        })?;

    let audio_enabled = request.has_audio && resolved.family.allows_audio;

    Ok(ConcreteExportSettings {
        family: resolved.family,
        profile: resolved.profile,
        selected_bucket_id,
        target_video_bitrate_kbps: template.target_video_bitrate_kbps,
        max_video_bitrate_kbps: template.max_video_bitrate_kbps,
        audio_bitrate_kbps: audio_enabled
            .then_some(template.audio_bitrate_kbps)
            .flatten(),
        sample_rate_hz: audio_enabled.then_some(template.sample_rate_hz).flatten(),
        keyframe_interval_seconds: template.keyframe_interval_seconds,
        preferred_audio_codecs: if audio_enabled {
            template.preferred_audio_codecs.clone()
        } else {
            Vec::new()
        },
        audio_enabled,
    })
}

pub fn validate_family_capabilities(
    family: &ExportFamily,
    has_audio: bool,
    capabilities: &RuntimeCapabilities,
) -> Result<(), ExportPlanError> {
    for muxer in &family.required_muxers {
        if !capabilities
            .muxers
            .iter()
            .any(|candidate| candidate == muxer)
        {
            return Err(ExportPlanError::MissingCapability {
                capability_kind: "muxer",
                name: muxer.clone(),
            });
        }
    }

    for encoder in &family.required_video_encoders {
        if !capabilities
            .video_encoders
            .iter()
            .any(|candidate| candidate == encoder)
        {
            return Err(ExportPlanError::MissingCapability {
                capability_kind: "video encoder",
                name: encoder.clone(),
            });
        }
    }

    if has_audio && family.allows_audio {
        for encoder in &family.required_audio_encoders {
            if !capabilities
                .audio_encoders
                .iter()
                .any(|candidate| candidate == encoder)
            {
                return Err(ExportPlanError::MissingCapability {
                    capability_kind: "audio encoder",
                    name: encoder.clone(),
                });
            }
        }
    }

    Ok(())
}

pub fn bucket_candidates(width: u32, height: u32, frame_rate: f64) -> Vec<String> {
    let mut buckets = Vec::new();
    buckets.push(format!("{width}x{height}"));

    let standard_height = classify_standard_height(width, height);
    if standard_height == 2160 {
        if frame_rate > 30.0 {
            buckets.push("2160p_sdr_high".into());
        } else {
            buckets.push("2160p_sdr_low".into());
        }
    } else {
        buckets.push(format!("{standard_height}p_sdr"));
    }
    buckets.push(format!("{standard_height}p"));
    buckets.push("default".into());
    buckets.dedup();
    buckets
}

fn classify_standard_height(width: u32, height: u32) -> u32 {
    match width.min(height) {
        0..=720 => 720,
        721..=1080 => 1080,
        1081..=1440 => 1440,
        _ => 2160,
    }
}

pub fn execute_export(
    catalog: &ExportCatalog,
    runtime: &MediaRuntime,
    capabilities: &RuntimeCapabilities,
    snapshot: &ExportSnapshot,
    request: &ExportExecutionRequest,
) -> Result<ExportExecutionResult, ExportExecutionError> {
    let settings = plan_export(
        catalog,
        &ExportRequest {
            family_id: request.family_id.clone(),
            profile_id: request.profile_id.clone(),
            width: snapshot.width,
            height: snapshot.height,
            frame_rate: snapshot.frame_rate,
            has_audio: snapshot.has_audio,
            requires_alpha: request.transparent,
        },
        Some(capabilities),
    )?;
    execute_export_with_settings(
        runtime,
        capabilities,
        snapshot,
        &settings,
        &ExportOutputRequest {
            output_path: request.output_path.clone(),
            transparent: request.transparent,
            working_directory: request.working_directory.clone(),
            prefer_hardware: request.prefer_hardware,
        },
    )
}

pub fn execute_export_with_settings(
    runtime: &MediaRuntime,
    capabilities: &RuntimeCapabilities,
    snapshot: &ExportSnapshot,
    settings: &ConcreteExportSettings,
    request: &ExportOutputRequest,
) -> Result<ExportExecutionResult, ExportExecutionError> {
    let frames_dir = request.working_directory.join("frames");
    if frames_dir.exists() {
        fs::remove_dir_all(&frames_dir)?;
    }
    fs::create_dir_all(&frames_dir)?;

    let frame_count = render_overlay_sequence(snapshot, &frames_dir)?;
    let software_fallback_used = match settings.family.id.as_str() {
        "png_sequence_rgba" => {
            export_png_sequence(&frames_dir, &request.output_path, frame_count)?;
            false
        }
        _ if request.transparent => {
            export_transparent_video(
                runtime,
                snapshot,
                settings,
                &frames_dir,
                &request.output_path,
            )?;
            false
        }
        _ => export_composite_video(
            runtime,
            capabilities,
            snapshot,
            settings,
            &frames_dir,
            &request.output_path,
            request.prefer_hardware,
        )?,
    };

    Ok(ExportExecutionResult {
        output_path: request.output_path.clone(),
        frame_count,
        settings: settings.clone(),
        software_fallback_used,
    })
}

fn render_overlay_sequence(
    snapshot: &ExportSnapshot,
    frames_dir: &Path,
) -> Result<usize, ExportExecutionError> {
    let frame_count = estimated_frame_count(snapshot.duration, snapshot.frame_rate);

    for frame_index in 0..frame_count {
        let time = frame_time(frame_index, snapshot.frame_rate);
        let overlay = render_overlay_rgba(&RenderRequest {
            project: &snapshot.project,
            time,
            width: snapshot.width,
            height: snapshot.height,
            source_width: snapshot.width,
            source_height: snapshot.height,
            background: pauseink_domain::RgbaColor::new(0, 0, 0, 0),
        })
        .map_err(|error| ExportExecutionError::Render(error.to_string()))?;
        let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
            overlay.width,
            overlay.height,
            overlay.rgba_pixels,
        )
        .ok_or_else(|| ExportExecutionError::Render("overlay buffer shape mismatch".to_owned()))?;
        image.save(frames_dir.join(format!("frame_{frame_index:06}.png")))?;
    }

    Ok(frame_count)
}

fn export_png_sequence(
    frames_dir: &Path,
    output_dir: &Path,
    frame_count: usize,
) -> Result<(), ExportExecutionError> {
    if output_dir.as_os_str().is_empty() {
        return Err(ExportExecutionError::InvalidOutputPath);
    }
    fs::create_dir_all(output_dir)?;
    for frame_index in 0..frame_count {
        let source = frames_dir.join(format!("frame_{frame_index:06}.png"));
        let target = output_dir.join(format!("frame_{frame_index:06}.png"));
        fs::copy(source, target)?;
    }
    Ok(())
}

fn export_transparent_video(
    runtime: &MediaRuntime,
    snapshot: &ExportSnapshot,
    settings: &ConcreteExportSettings,
    frames_dir: &Path,
    output_path: &Path,
) -> Result<(), ExportExecutionError> {
    let mut command = Command::new(&runtime.ffmpeg_path);
    command.args([
        "-y",
        "-loglevel",
        "error",
        "-framerate",
        &format_fps(snapshot.frame_rate),
        "-i",
    ]);
    command.arg(frames_dir.join("frame_%06d.png"));
    command.args(["-c:v", primary_video_encoder(settings)?]);
    apply_video_settings(&mut command, settings, snapshot.frame_rate, true);
    if settings.audio_enabled {
        command.args(["-an"]);
    }
    command.arg(output_path);
    run_ffmpeg_command(command)
}

fn export_composite_video(
    runtime: &MediaRuntime,
    capabilities: &RuntimeCapabilities,
    snapshot: &ExportSnapshot,
    settings: &ConcreteExportSettings,
    frames_dir: &Path,
    output_path: &Path,
    prefer_hardware: bool,
) -> Result<bool, ExportExecutionError> {
    let source_media_path = snapshot
        .source_media_path
        .as_ref()
        .ok_or(ExportExecutionError::MissingSourceMedia)?;
    let try_hardware = should_try_hardware_decode(prefer_hardware, capabilities);

    if try_hardware {
        let hardware_result = run_ffmpeg_command(build_composite_command(
            runtime,
            source_media_path,
            snapshot,
            settings,
            frames_dir,
            output_path,
            true,
        ));
        if hardware_result.is_ok() {
            return Ok(false);
        }
    }

    run_ffmpeg_command(build_composite_command(
        runtime,
        source_media_path,
        snapshot,
        settings,
        frames_dir,
        output_path,
        false,
    ))?;
    Ok(try_hardware)
}

fn build_composite_command(
    runtime: &MediaRuntime,
    source_media_path: &Path,
    snapshot: &ExportSnapshot,
    settings: &ConcreteExportSettings,
    frames_dir: &Path,
    output_path: &Path,
    try_hardware: bool,
) -> Command {
    let mut command = Command::new(&runtime.ffmpeg_path);
    command.arg("-y").args(["-loglevel", "error"]);
    if try_hardware {
        command.args(["-hwaccel", "auto"]);
    }
    command.arg("-i");
    command.arg(source_media_path);
    command.args(["-framerate", &format_fps(snapshot.frame_rate), "-i"]);
    command.arg(frames_dir.join("frame_%06d.png"));
    command.args([
        "-filter_complex",
        "[0:v][1:v]overlay=0:0:format=auto[v]",
        "-map",
        "[v]",
    ]);
    if settings.audio_enabled && snapshot.has_audio {
        if let Ok(audio_encoder) = primary_audio_encoder(settings) {
            command.args(["-map", "0:a?"]);
            command.args(["-c:a", audio_encoder]);
        }
        if let Some(audio_bitrate_kbps) = settings.audio_bitrate_kbps {
            command.args(["-b:a", &format!("{audio_bitrate_kbps}k")]);
        }
        if let Some(sample_rate_hz) = settings.sample_rate_hz {
            command.args(["-ar", &sample_rate_hz.to_string()]);
        }
    } else {
        command.arg("-an");
    }
    if let Ok(video_encoder) = primary_video_encoder(settings) {
        command.args(["-c:v", video_encoder]);
    }
    apply_video_settings(&mut command, settings, snapshot.frame_rate, false);
    command.arg(output_path);
    command
}

fn should_try_hardware_decode(prefer_hardware: bool, capabilities: &RuntimeCapabilities) -> bool {
    prefer_hardware && !capabilities.hwaccels.is_empty()
}

fn apply_video_settings(
    command: &mut Command,
    settings: &ConcreteExportSettings,
    frame_rate: f64,
    transparent: bool,
) {
    if !matches!(
        settings.family.id.as_str(),
        "avi_mjpeg_pcm" | "mov_prores_422hq_pcm" | "mov_prores_4444_pcm" | "png_sequence_rgba"
    ) {
        if let Some(target_video_bitrate_kbps) = settings.target_video_bitrate_kbps {
            command.args(["-b:v", &format!("{target_video_bitrate_kbps}k")]);
        }
        if let Some(max_video_bitrate_kbps) = settings.max_video_bitrate_kbps {
            command.args(["-maxrate", &format!("{max_video_bitrate_kbps}k")]);
        }
        if let Some(keyframe_interval_seconds) = settings.keyframe_interval_seconds {
            let gop = (frame_rate * keyframe_interval_seconds as f64)
                .round()
                .max(1.0) as u32;
            command.args(["-g", &gop.to_string()]);
        }
    }

    match settings.family.id.as_str() {
        "mov_prores_422hq_pcm" => {
            command.args(["-profile:v", "3", "-pix_fmt", "yuv422p10le"]);
        }
        "mov_prores_4444_pcm" => {
            command.args(["-profile:v", "4", "-pix_fmt", "yuva444p10le"]);
        }
        "avi_mjpeg_pcm" => {
            command.args(["-pix_fmt", "yuvj420p", "-q:v", "3"]);
        }
        "webm_vp9_opus" | "webm_av1_opus" | "mp4_av1_aac" => {
            command.args(["-pix_fmt", "yuv420p"]);
        }
        _ if transparent => {
            command.args(["-pix_fmt", "rgba"]);
        }
        _ => {}
    }
}

fn primary_video_encoder(settings: &ConcreteExportSettings) -> Result<&str, ExportExecutionError> {
    settings
        .family
        .required_video_encoders
        .first()
        .map(String::as_str)
        .ok_or(ExportExecutionError::InvalidOutputPath)
}

fn primary_audio_encoder(settings: &ConcreteExportSettings) -> Result<&str, ExportExecutionError> {
    settings
        .family
        .required_audio_encoders
        .first()
        .map(String::as_str)
        .ok_or(ExportExecutionError::InvalidOutputPath)
}

fn run_ffmpeg_command(mut command: Command) -> Result<(), ExportExecutionError> {
    let output = command.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ExportExecutionError::CommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn estimated_frame_count(duration: MediaDuration, frame_rate: f64) -> usize {
    let seconds = duration.ticks as f64 * duration.time_base.numerator as f64
        / duration.time_base.denominator as f64;
    (seconds * frame_rate).ceil().max(1.0) as usize
}

fn frame_time(frame_index: usize, frame_rate: f64) -> MediaTime {
    let millis = ((frame_index as f64 / frame_rate.max(1.0)) * 1_000.0).round() as i64;
    MediaTime::new(millis, TimeBase::milliseconds())
}

fn format_fps(frame_rate: f64) -> String {
    format!("{frame_rate:.6}")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::process::Command;

    use pauseink_domain::{
        GlyphObject, GlyphObjectId, OrderingMetadata, Stroke, StrokeId, StrokeSample,
    };
    use pauseink_media::{
        discover_system_runtime, FfprobeMediaProvider, MediaProvider, MediaSupport,
        RuntimeCapabilities,
    };
    use pauseink_presets_core::ExportCatalog;
    use tempfile::tempdir;

    use super::*;

    fn load_catalog() -> ExportCatalog {
        let profile_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/export_profiles");
        ExportCatalog::load_builtin_from_dir(&profile_dir).expect("catalog should load")
    }

    fn av1_capabilities() -> RuntimeCapabilities {
        RuntimeCapabilities {
            video_encoders: vec![
                "libaom-av1".into(),
                "libvpx-vp9".into(),
                "prores_ks".into(),
                "png".into(),
                "mjpeg".into(),
            ],
            audio_encoders: vec!["aac".into(), "libopus".into(), "pcm_s16le".into()],
            muxers: vec![
                "webm".into(),
                "mp4".into(),
                "mov".into(),
                "avi".into(),
                "image2".into(),
            ],
            hwaccels: vec![],
        }
    }

    fn sample_snapshot(source_media_path: Option<PathBuf>, has_audio: bool) -> ExportSnapshot {
        ExportSnapshot {
            project: AnnotationProject {
                strokes: vec![Stroke {
                    id: StrokeId::new("stroke-1"),
                    raw_samples: vec![
                        StrokeSample {
                            position: pauseink_domain::Point2 { x: 40.0, y: 40.0 },
                            at: MediaTime::from_millis(0),
                            pressure: None,
                        },
                        StrokeSample {
                            position: pauseink_domain::Point2 { x: 220.0, y: 120.0 },
                            at: MediaTime::from_millis(300),
                            pressure: None,
                        },
                    ],
                    created_at: MediaTime::from_millis(0),
                    style: pauseink_domain::StyleSnapshot {
                        color: pauseink_domain::RgbaColor::new(255, 255, 0, 255),
                        thickness: 10.0,
                        ..pauseink_domain::StyleSnapshot::default()
                    },
                    ..Stroke::default()
                }],
                glyph_objects: vec![GlyphObject {
                    id: GlyphObjectId::new("object-1"),
                    stroke_ids: vec![StrokeId::new("stroke-1")],
                    ordering: OrderingMetadata {
                        z_index: 0,
                        capture_order: 1,
                        reveal_order: 1,
                    },
                    created_at: MediaTime::from_millis(0),
                    ..GlyphObject::default()
                }],
                ..AnnotationProject::default()
            },
            width: 320,
            height: 180,
            frame_rate: 10.0,
            duration: MediaDuration::from_millis(1_000),
            source_media_path,
            has_audio,
        }
    }

    #[test]
    fn youtube_4k60_prefers_high_sdr_bucket() {
        let catalog = load_catalog();
        let settings = plan_export(
            &catalog,
            &ExportRequest {
                family_id: "webm_av1_opus".into(),
                profile_id: "youtube".into(),
                width: 3840,
                height: 2160,
                frame_rate: 60.0,
                has_audio: true,
                requires_alpha: false,
            },
            Some(&av1_capabilities()),
        )
        .expect("plan should resolve");

        assert_eq!(settings.selected_bucket_id, "2160p_sdr_high");
        assert_eq!(settings.target_video_bitrate_kbps, Some(45000));
        assert_eq!(settings.audio_bitrate_kbps, Some(384));
        assert!(settings.audio_enabled);
    }

    #[test]
    fn instagram_prefers_exact_portrait_bucket() {
        let catalog = load_catalog();
        let settings = plan_export(
            &catalog,
            &ExportRequest {
                family_id: "mp4_av1_aac".into(),
                profile_id: "instagram".into(),
                width: 1080,
                height: 1920,
                frame_rate: 30.0,
                has_audio: true,
                requires_alpha: false,
            },
            Some(&av1_capabilities()),
        )
        .expect("plan should resolve");

        assert_eq!(settings.selected_bucket_id, "1080x1920");
        assert_eq!(settings.target_video_bitrate_kbps, Some(8000));
        assert_eq!(settings.audio_bitrate_kbps, Some(128));
    }

    #[test]
    fn alpha_request_rejects_non_alpha_family() {
        let catalog = load_catalog();
        let error = plan_export(
            &catalog,
            &ExportRequest {
                family_id: "webm_vp9_opus".into(),
                profile_id: "low".into(),
                width: 1280,
                height: 720,
                frame_rate: 30.0,
                has_audio: true,
                requires_alpha: true,
            },
            Some(&av1_capabilities()),
        )
        .expect_err("alpha should be rejected for non-alpha family");

        assert!(matches!(
            error,
            ExportPlanError::AlphaUnsupported { family_id } if family_id == "webm_vp9_opus"
        ));
    }

    #[test]
    fn png_sequence_disables_audio_fields_even_with_audio_profile_defaults() {
        let catalog = load_catalog();
        let settings = plan_export(
            &catalog,
            &ExportRequest {
                family_id: "png_sequence_rgba".into(),
                profile_id: "adobe_alpha".into(),
                width: 1920,
                height: 1080,
                frame_rate: 30.0,
                has_audio: true,
                requires_alpha: true,
            },
            Some(&av1_capabilities()),
        )
        .expect("png sequence should plan");

        assert!(!settings.audio_enabled);
        assert_eq!(settings.audio_bitrate_kbps, None);
        assert_eq!(settings.sample_rate_hz, None);
        assert!(settings.preferred_audio_codecs.is_empty());
    }

    #[test]
    fn missing_required_capability_is_reported() {
        let catalog = load_catalog();
        let error = plan_export(
            &catalog,
            &ExportRequest {
                family_id: "mp4_av1_aac".into(),
                profile_id: "medium".into(),
                width: 1920,
                height: 1080,
                frame_rate: 30.0,
                has_audio: true,
                requires_alpha: false,
            },
            Some(&RuntimeCapabilities {
                video_encoders: vec!["libvpx-vp9".into()],
                audio_encoders: vec!["aac".into()],
                muxers: vec!["mp4".into()],
                hwaccels: vec![],
            }),
        )
        .expect_err("missing av1 encoder should fail");

        assert!(matches!(
            error,
            ExportPlanError::MissingCapability {
                capability_kind: "video encoder",
                name,
            } if name == "libaom-av1"
        ));
    }

    #[test]
    fn transparent_png_sequence_export_smoke_if_host_runtime_exists() {
        let runtime = match discover_system_runtime() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        let provider = FfprobeMediaProvider::new(runtime.clone());
        let capabilities = match provider.capabilities() {
            Ok(capabilities) => capabilities,
            Err(_) => return,
        };
        let catalog = load_catalog();
        let temp_dir = tempdir().expect("temp dir");
        let output_dir = temp_dir.path().join("transparent_frames");

        let result = execute_export(
            &catalog,
            &runtime,
            &capabilities,
            &sample_snapshot(None, false),
            &ExportExecutionRequest {
                family_id: "png_sequence_rgba".into(),
                profile_id: "adobe_alpha".into(),
                output_path: output_dir.clone(),
                transparent: true,
                working_directory: temp_dir.path().join("work_png"),
                prefer_hardware: false,
            },
        )
        .expect("transparent export should succeed");

        assert_eq!(result.frame_count, 10);
        assert!(!result.software_fallback_used);
        assert!(output_dir.join("frame_000000.png").is_file());
        assert!(output_dir.join("frame_000009.png").is_file());
    }

    #[test]
    fn composite_avi_export_smoke_if_host_runtime_exists() {
        let runtime = match discover_system_runtime() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        let provider = FfprobeMediaProvider::new(runtime.clone());
        let capabilities = match provider.capabilities() {
            Ok(capabilities) => capabilities,
            Err(_) => return,
        };
        let temp_dir = tempdir().expect("temp dir");
        let source_path = temp_dir.path().join("source.avi");
        let fixture = Command::new(&runtime.ffmpeg_path)
            .args([
                "-y",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=320x180:d=1:r=10",
                "-an",
                "-c:v",
                "mjpeg",
            ])
            .arg(&source_path)
            .output()
            .expect("fixture command should run");

        assert!(
            fixture.status.success(),
            "fixture creation should succeed: {}",
            String::from_utf8_lossy(&fixture.stderr)
        );

        let output_path = temp_dir.path().join("composite.avi");
        let catalog = load_catalog();
        let result = execute_export(
            &catalog,
            &runtime,
            &capabilities,
            &sample_snapshot(Some(source_path.clone()), false),
            &ExportExecutionRequest {
                family_id: "avi_mjpeg_pcm".into(),
                profile_id: "low".into(),
                output_path: output_path.clone(),
                transparent: false,
                working_directory: temp_dir.path().join("work_avi"),
                prefer_hardware: false,
            },
        )
        .expect("composite export should succeed");

        assert_eq!(result.output_path, output_path);
        assert!(!result.software_fallback_used);
        assert!(output_path.is_file());

        let probe = provider
            .probe(&output_path)
            .expect("exported composite should be probeable");
        assert_eq!(probe.width, Some(320));
        assert_eq!(probe.height, Some(180));
        assert!(matches!(probe.support, MediaSupport::Supported));
    }

    #[test]
    fn hardware_fallback_is_only_attempted_when_enabled_and_available() {
        assert!(should_try_hardware_decode(
            true,
            &RuntimeCapabilities {
                hwaccels: vec!["vaapi".into()],
                ..RuntimeCapabilities::default()
            }
        ));
        assert!(!should_try_hardware_decode(
            false,
            &RuntimeCapabilities {
                hwaccels: vec!["vaapi".into()],
                ..RuntimeCapabilities::default()
            }
        ));
        assert!(!should_try_hardware_decode(
            true,
            &RuntimeCapabilities::default()
        ));
    }
}
