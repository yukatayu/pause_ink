use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeTier {
    Mainline,
    OptionalCodecPack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    CompositeOnly,
    TransparentOnly,
    TransparentOrComposite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportFamily {
    pub id: String,
    pub display_name: String,
    pub container: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub supports_alpha: bool,
    pub allows_audio: bool,
    pub output_kind: OutputKind,
    pub required_muxers: Vec<String>,
    pub required_video_encoders: Vec<String>,
    pub required_audio_encoders: Vec<String>,
    pub runtime_tier: RuntimeTier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileCompatibility {
    Any,
    Only(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSourceKind {
    Official,
    AppAuthored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProfilePublicConstraints {
    pub min_fps: Option<u32>,
    pub max_fps: Option<u32>,
    pub min_resolution_px: Option<u32>,
    pub max_resolution_px: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConcreteExportSettingsTemplate {
    pub target_video_bitrate_kbps: Option<u32>,
    pub max_video_bitrate_kbps: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub sample_rate_hz: Option<u32>,
    pub keyframe_interval_seconds: Option<u32>,
    #[serde(default)]
    pub preferred_audio_codecs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionProfile {
    pub id: String,
    pub display_name: String,
    pub source_kind: ProfileSourceKind,
    pub source_urls: Vec<String>,
    pub compatibility: ProfileCompatibility,
    pub notes: String,
    pub public_constraints: ProfilePublicConstraints,
    pub settings_buckets: BTreeMap<String, ConcreteExportSettingsTemplate>,
}

impl DistributionProfile {
    pub fn is_compatible_with(&self, family_id: &str) -> bool {
        match &self.compatibility {
            ProfileCompatibility::Any => true,
            ProfileCompatibility::Only(allowed) => {
                allowed.iter().any(|candidate| candidate == family_id)
            }
        }
    }

    pub fn setting_bucket(&self, bucket_id: &str) -> Option<&ConcreteExportSettingsTemplate> {
        self.settings_buckets.get(bucket_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportCatalog {
    families: BTreeMap<String, ExportFamily>,
    profiles: BTreeMap<String, DistributionProfile>,
}

impl ExportCatalog {
    pub fn new(families: Vec<ExportFamily>, profiles: Vec<DistributionProfile>) -> Self {
        Self {
            families: families
                .into_iter()
                .map(|family| (family.id.clone(), family))
                .collect(),
            profiles: profiles
                .into_iter()
                .map(|profile| (profile.id.clone(), profile))
                .collect(),
        }
    }

    pub fn resolve(
        &self,
        family_id: &str,
        profile_id: &str,
    ) -> Result<ResolvedExportSelection, ResolveError> {
        let family = self
            .families
            .get(family_id)
            .cloned()
            .ok_or_else(|| ResolveError::UnknownFamily(family_id.to_owned()))?;
        let profile = self
            .profiles
            .get(profile_id)
            .cloned()
            .ok_or_else(|| ResolveError::UnknownProfile(profile_id.to_owned()))?;

        if !profile.is_compatible_with(family_id) {
            return Err(ResolveError::IncompatibleSelection {
                family_id: family_id.to_owned(),
                profile_id: profile_id.to_owned(),
            });
        }

        Ok(ResolvedExportSelection { family, profile })
    }

    pub fn families_for_tier(&self, runtime_tier: RuntimeTier) -> Vec<&ExportFamily> {
        self.families
            .values()
            .filter(|family| family.runtime_tier == runtime_tier)
            .collect()
    }

    pub fn load_builtin_from_dir(profile_dir: &Path) -> Result<Self, ProfileLoadError> {
        Ok(Self::new(
            built_in_export_families(),
            load_distribution_profiles_from_dir(profile_dir)?,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedExportSelection {
    pub family: ExportFamily,
    pub profile: DistributionProfile,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ResolveError {
    #[error("unknown export family: {0}")]
    UnknownFamily(String),
    #[error("unknown distribution profile: {0}")]
    UnknownProfile(String),
    #[error("incompatible family/profile selection: {family_id} x {profile_id}")]
    IncompatibleSelection { family_id: String, profile_id: String },
}

#[derive(Debug, Error)]
pub enum ProfileLoadError {
    #[error("profile read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("profile parse failed in {path}: {source}")]
    Parse {
        path: PathBuf,
        source: json5::Error,
    },
    #[error("invalid profile compatibility in {path}: {value}")]
    InvalidCompatibility { path: PathBuf, value: String },
    #[error("invalid bitrate in {path} for bucket {bucket}: {mbps}")]
    InvalidBitrate {
        path: PathBuf,
        bucket: String,
        mbps: f64,
    },
    #[error("duplicate profile id: {0}")]
    DuplicateProfileId(String),
}

pub fn built_in_export_families() -> Vec<ExportFamily> {
    vec![
        ExportFamily {
            id: "webm_vp9_opus".into(),
            display_name: "WebM / VP9 / Opus".into(),
            container: "webm".into(),
            video_codec: Some("vp9".into()),
            audio_codec: Some("opus".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["webm".into()],
            required_video_encoders: vec!["libvpx-vp9".into()],
            required_audio_encoders: vec!["libopus".into()],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "webm_av1_opus".into(),
            display_name: "WebM / AV1 / Opus".into(),
            container: "webm".into(),
            video_codec: Some("av1".into()),
            audio_codec: Some("opus".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["webm".into()],
            required_video_encoders: vec!["libaom-av1".into()],
            required_audio_encoders: vec!["libopus".into()],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "mp4_av1_aac".into(),
            display_name: "MP4 / AV1 / AAC-LC".into(),
            container: "mp4".into(),
            video_codec: Some("av1".into()),
            audio_codec: Some("aac".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["mp4".into()],
            required_video_encoders: vec!["libaom-av1".into()],
            required_audio_encoders: vec!["aac".into()],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "mov_prores_422hq_pcm".into(),
            display_name: "MOV / ProRes 422 HQ / PCM".into(),
            container: "mov".into(),
            video_codec: Some("prores_ks".into()),
            audio_codec: Some("pcm_s16le".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["mov".into()],
            required_video_encoders: vec!["prores_ks".into()],
            required_audio_encoders: vec!["pcm_s16le".into()],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "mov_prores_4444_pcm".into(),
            display_name: "MOV / ProRes 4444 / PCM".into(),
            container: "mov".into(),
            video_codec: Some("prores_ks".into()),
            audio_codec: Some("pcm_s16le".into()),
            supports_alpha: true,
            allows_audio: true,
            output_kind: OutputKind::TransparentOrComposite,
            required_muxers: vec!["mov".into()],
            required_video_encoders: vec!["prores_ks".into()],
            required_audio_encoders: vec!["pcm_s16le".into()],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "png_sequence_rgba".into(),
            display_name: "PNG Sequence / RGBA".into(),
            container: "image2".into(),
            video_codec: Some("png".into()),
            audio_codec: None,
            supports_alpha: true,
            allows_audio: false,
            output_kind: OutputKind::TransparentOrComposite,
            required_muxers: vec!["image2".into()],
            required_video_encoders: vec!["png".into()],
            required_audio_encoders: vec![],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "avi_mjpeg_pcm".into(),
            display_name: "AVI / MJPEG / PCM".into(),
            container: "avi".into(),
            video_codec: Some("mjpeg".into()),
            audio_codec: Some("pcm_s16le".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["avi".into()],
            required_video_encoders: vec!["mjpeg".into()],
            required_audio_encoders: vec!["pcm_s16le".into()],
            runtime_tier: RuntimeTier::Mainline,
        },
        ExportFamily {
            id: "mp4_h264_aac".into(),
            display_name: "MP4 / H.264 / AAC-LC".into(),
            container: "mp4".into(),
            video_codec: Some("h264".into()),
            audio_codec: Some("aac".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["mp4".into()],
            required_video_encoders: vec!["libx264".into()],
            required_audio_encoders: vec!["aac".into()],
            runtime_tier: RuntimeTier::OptionalCodecPack,
        },
        ExportFamily {
            id: "mp4_hevc_aac".into(),
            display_name: "MP4 / HEVC / AAC-LC".into(),
            container: "mp4".into(),
            video_codec: Some("hevc".into()),
            audio_codec: Some("aac".into()),
            supports_alpha: false,
            allows_audio: true,
            output_kind: OutputKind::CompositeOnly,
            required_muxers: vec!["mp4".into()],
            required_video_encoders: vec!["libx265".into()],
            required_audio_encoders: vec!["aac".into()],
            runtime_tier: RuntimeTier::OptionalCodecPack,
        },
    ]
}

pub fn load_distribution_profiles_from_dir(
    profile_dir: &Path,
) -> Result<Vec<DistributionProfile>, ProfileLoadError> {
    let mut paths = fs::read_dir(profile_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json5"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut profiles = Vec::new();
    let mut seen_ids = BTreeSet::new();

    for path in paths {
        let profile = load_distribution_profile_from_path(&path)?;
        if !seen_ids.insert(profile.id.clone()) {
            return Err(ProfileLoadError::DuplicateProfileId(profile.id));
        }
        profiles.push(profile);
    }

    Ok(profiles)
}

pub fn load_distribution_profile_from_path(
    path: &Path,
) -> Result<DistributionProfile, ProfileLoadError> {
    let raw = fs::read_to_string(path)?;
    load_distribution_profile_from_str_with_path(&raw, path)
}

pub fn load_distribution_profile_from_str(
    raw: &str,
) -> Result<DistributionProfile, ProfileLoadError> {
    load_distribution_profile_from_str_with_path(raw, Path::new("<inline>"))
}

fn load_distribution_profile_from_str_with_path(
    raw: &str,
    path: &Path,
) -> Result<DistributionProfile, ProfileLoadError> {
    let file: DistributionProfileFile = json5::from_str(raw).map_err(|source| {
        ProfileLoadError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })?;

    file.into_profile(path)
}

#[derive(Debug, Deserialize)]
struct DistributionProfileFile {
    id: String,
    display_name: String,
    source_kind: Option<ProfileSourceKind>,
    #[serde(default)]
    source_urls: Vec<String>,
    compatibility: Option<CompatibilityFile>,
    family: Option<String>,
    intended_families: Option<Vec<String>>,
    notes: Option<String>,
    #[serde(default)]
    public_constraints: ProfilePublicConstraints,
    #[serde(default)]
    settings_buckets: BTreeMap<String, ConcreteExportSettingsTemplate>,
    #[serde(default)]
    video_bitrate_ladder_mbps: BTreeMap<String, f64>,
    #[serde(default)]
    app_safe_defaults: BTreeMap<String, LegacySafeDefault>,
    audio: Option<LegacyAudioDefaults>,
}

impl DistributionProfileFile {
    fn into_profile(self, path: &Path) -> Result<DistributionProfile, ProfileLoadError> {
        let compatibility = match self.compatibility {
            Some(file) => file.into_profile_compatibility(path)?,
            None => infer_compatibility(path, self.family, self.intended_families)?,
        };

        let source_kind = self.source_kind.unwrap_or_else(|| {
            if self.source_urls.is_empty() {
                ProfileSourceKind::AppAuthored
            } else {
                ProfileSourceKind::Official
            }
        });

        let mut settings_buckets = self.settings_buckets;

        if !self.video_bitrate_ladder_mbps.is_empty() {
            for (bucket, mbps) in self.video_bitrate_ladder_mbps {
                let entry = settings_buckets.entry(bucket.clone()).or_default();
                entry.target_video_bitrate_kbps = Some(mbps_to_kbps(path, &bucket, mbps)?);
                apply_legacy_audio_defaults(entry, self.audio.as_ref());
            }
        }

        if !self.app_safe_defaults.is_empty() {
            for (bucket, defaults) in self.app_safe_defaults {
                let entry = settings_buckets.entry(bucket.clone()).or_default();
                entry.target_video_bitrate_kbps =
                    Some(mbps_to_kbps(path, &bucket, defaults.video_bitrate_mbps)?);
                entry.audio_bitrate_kbps = Some(defaults.audio_bitrate_kbps);
                apply_legacy_audio_defaults(entry, self.audio.as_ref());
            }
        }

        Ok(DistributionProfile {
            id: self.id,
            display_name: self.display_name,
            source_kind,
            source_urls: self.source_urls,
            compatibility,
            notes: self.notes.unwrap_or_default(),
            public_constraints: self.public_constraints,
            settings_buckets,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CompatibilityFile {
    Keyword(String),
    FamilyList(Vec<String>),
}

impl CompatibilityFile {
    fn into_profile_compatibility(self, path: &Path) -> Result<ProfileCompatibility, ProfileLoadError> {
        match self {
            Self::Keyword(value) if value.eq_ignore_ascii_case("any") => {
                Ok(ProfileCompatibility::Any)
            }
            Self::Keyword(value) => Err(ProfileLoadError::InvalidCompatibility {
                path: path.to_path_buf(),
                value,
            }),
            Self::FamilyList(families) => Ok(ProfileCompatibility::Only(families)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct LegacySafeDefault {
    video_bitrate_mbps: f64,
    audio_bitrate_kbps: u32,
}

#[derive(Debug, Deserialize)]
struct LegacyAudioDefaults {
    #[serde(default)]
    codec_preference: Vec<String>,
    sample_rate_hz: Option<u32>,
    bitrate_kbps_stereo: Option<u32>,
}

fn infer_compatibility(
    path: &Path,
    family: Option<String>,
    intended_families: Option<Vec<String>>,
) -> Result<ProfileCompatibility, ProfileLoadError> {
    if let Some(family) = family {
        return Ok(ProfileCompatibility::Only(vec![family]));
    }

    if let Some(families) = intended_families {
        return Ok(ProfileCompatibility::Only(families));
    }

    CompatibilityFile::Keyword("any".into()).into_profile_compatibility(path)
}

fn apply_legacy_audio_defaults(
    entry: &mut ConcreteExportSettingsTemplate,
    audio: Option<&LegacyAudioDefaults>,
) {
    let Some(audio) = audio else {
        return;
    };

    if entry.audio_bitrate_kbps.is_none() {
        entry.audio_bitrate_kbps = audio.bitrate_kbps_stereo;
    }
    if entry.sample_rate_hz.is_none() {
        entry.sample_rate_hz = audio.sample_rate_hz;
    }
    for codec in &audio.codec_preference {
        if !entry.preferred_audio_codecs.contains(codec) {
            entry.preferred_audio_codecs.push(codec.clone());
        }
    }
}

fn mbps_to_kbps(path: &Path, bucket: &str, mbps: f64) -> Result<u32, ProfileLoadError> {
    if !mbps.is_finite() || mbps <= 0.0 {
        return Err(ProfileLoadError::InvalidBitrate {
            path: path.to_path_buf(),
            bucket: bucket.to_owned(),
            mbps,
        });
    }

    Ok((mbps * 1000.0).round() as u32)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn profile_compatibility_accepts_listed_family() {
        let profile = DistributionProfile {
            id: "youtube".into(),
            display_name: "YouTube".into(),
            source_kind: ProfileSourceKind::Official,
            source_urls: vec!["https://support.google.com/youtube/answer/1722171".into()],
            compatibility: ProfileCompatibility::Only(vec![
                "webm_vp9_opus".into(),
                "webm_av1_opus".into(),
            ]),
            notes: "Official guidance-based preset".into(),
            public_constraints: ProfilePublicConstraints::default(),
            settings_buckets: BTreeMap::new(),
        };

        assert!(profile.is_compatible_with("webm_av1_opus"));
        assert!(!profile.is_compatible_with("mov_prores_4444_pcm"));
        assert_eq!(profile.source_kind, ProfileSourceKind::Official);
    }

    #[test]
    fn catalog_resolves_family_and_profile_in_two_layers() {
        let catalog = ExportCatalog::new(
            vec![ExportFamily {
                id: "mov_prores_4444_pcm".into(),
                display_name: "MOV / ProRes 4444 / PCM".into(),
                container: "mov".into(),
                video_codec: Some("prores_ks".into()),
                audio_codec: Some("pcm_s16le".into()),
                supports_alpha: true,
                allows_audio: true,
                output_kind: OutputKind::TransparentOrComposite,
                required_muxers: vec!["mov".into()],
                required_video_encoders: vec!["prores_ks".into()],
                required_audio_encoders: vec!["pcm_s16le".into()],
                runtime_tier: RuntimeTier::Mainline,
            }],
            vec![DistributionProfile {
                id: "adobe_alpha".into(),
                display_name: "Adobe Alpha".into(),
                source_kind: ProfileSourceKind::AppAuthored,
                source_urls: vec![],
                compatibility: ProfileCompatibility::Only(vec![
                    "mov_prores_4444_pcm".into(),
                ]),
                notes: "Adobe-focused intermediate preset".into(),
                public_constraints: ProfilePublicConstraints::default(),
                settings_buckets: BTreeMap::new(),
            }],
        );

        let resolved = catalog
            .resolve("mov_prores_4444_pcm", "adobe_alpha")
            .expect("compatible family/profile should resolve");

        assert_eq!(resolved.family.id, "mov_prores_4444_pcm");
        assert_eq!(resolved.profile.id, "adobe_alpha");
    }

    #[test]
    fn mainline_family_listing_excludes_optional_codec_pack_families() {
        let catalog = ExportCatalog::new(
            vec![
                ExportFamily {
                    id: "webm_vp9_opus".into(),
                    display_name: "WebM / VP9 / Opus".into(),
                    container: "webm".into(),
                    video_codec: Some("vp9".into()),
                    audio_codec: Some("opus".into()),
                    supports_alpha: false,
                    allows_audio: true,
                    output_kind: OutputKind::CompositeOnly,
                    required_muxers: vec!["webm".into()],
                    required_video_encoders: vec!["libvpx-vp9".into()],
                    required_audio_encoders: vec!["libopus".into()],
                    runtime_tier: RuntimeTier::Mainline,
                },
                ExportFamily {
                    id: "mp4_h264_aac".into(),
                    display_name: "MP4 / H.264 / AAC".into(),
                    container: "mp4".into(),
                    video_codec: Some("h264".into()),
                    audio_codec: Some("aac".into()),
                    supports_alpha: false,
                    allows_audio: true,
                    output_kind: OutputKind::CompositeOnly,
                    required_muxers: vec!["mp4".into()],
                    required_video_encoders: vec!["libx264".into()],
                    required_audio_encoders: vec!["aac".into()],
                    runtime_tier: RuntimeTier::OptionalCodecPack,
                },
            ],
            vec![],
        );

        let family_ids = catalog
            .families_for_tier(RuntimeTier::Mainline)
            .into_iter()
            .map(|family| family.id.clone())
            .collect::<Vec<_>>();

        assert_eq!(family_ids, vec!["webm_vp9_opus".to_owned()]);
    }

    #[test]
    fn legacy_profile_schema_is_normalized_into_setting_buckets() {
        let profile = load_distribution_profile_from_str(
            r#"{
              id: "legacy_youtube",
              display_name: "Legacy YouTube",
              intended_families: ["webm_vp9_opus", "webm_av1_opus"],
              source_urls: ["https://support.google.com/youtube/answer/1722171"],
              video_bitrate_ladder_mbps: {
                "1080p_sdr": 8,
              },
              audio: {
                codec_preference: ["aac", "libopus"],
                sample_rate_hz: 48000,
                bitrate_kbps_stereo: 384,
              },
            }"#,
        )
        .expect("legacy schema should normalize");

        assert_eq!(profile.source_kind, ProfileSourceKind::Official);
        assert!(profile.is_compatible_with("webm_av1_opus"));
        let bucket = profile
            .setting_bucket("1080p_sdr")
            .expect("bucket should exist");
        assert_eq!(bucket.target_video_bitrate_kbps, Some(8000));
        assert_eq!(bucket.audio_bitrate_kbps, Some(384));
        assert_eq!(bucket.sample_rate_hz, Some(48000));
        assert_eq!(bucket.preferred_audio_codecs, vec!["aac", "libopus"]);
    }

    #[test]
    fn repository_export_profiles_load_with_builtin_catalog() {
        let profile_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/export_profiles");
        let catalog =
            ExportCatalog::load_builtin_from_dir(&profile_dir).expect("catalog should load");

        let low = catalog
            .resolve("webm_vp9_opus", "low")
            .expect("low profile should resolve with webm");
        assert_eq!(low.profile.display_name, "低");

        let youtube = catalog
            .resolve("webm_av1_opus", "youtube")
            .expect("youtube profile should resolve with webm av1");
        assert_eq!(youtube.profile.source_kind, ProfileSourceKind::Official);
        assert_eq!(
            youtube
                .profile
                .setting_bucket("1080p_sdr")
                .and_then(|bucket| bucket.target_video_bitrate_kbps),
            Some(8000)
        );

        assert!(catalog.resolve("png_sequence_rgba", "youtube").is_err());
    }
}
