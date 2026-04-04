use pauseink_media::RuntimeCapabilities;
use pauseink_presets_core::{
    DistributionProfile, ExportCatalog, ExportFamily, ResolveError,
};
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
        if !capabilities.muxers.iter().any(|candidate| candidate == muxer) {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use pauseink_media::RuntimeCapabilities;
    use pauseink_presets_core::ExportCatalog;

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
}
