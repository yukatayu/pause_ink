use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use pauseink_domain::{
    BlendMode, ClearKind, ClearOrdering, ClearTargetGranularity, ColorMode, ColorStop,
    DropShadowStyle, EntranceBehavior, EntranceDurationMode, EntranceKind, GlowStyle,
    GradientRepeat, GradientSpace, LinearGradientStyle, MediaDuration, OutlineStyle,
    PostAction, PostActionKind, PostActionTimingScope, RevealHeadColorSource, RevealHeadEffect,
    RevealHeadKind, RgbaColor, StyleSnapshot,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(test)]
use pauseink_domain::{EffectOrder, EffectScope};

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

#[derive(Debug, Clone, PartialEq)]
pub struct BaseStylePreset {
    pub id: String,
    pub display_name: String,
    pub thickness: Option<f32>,
    pub color_rgba: Option<[u8; 4]>,
    pub color_mode: Option<ColorMode>,
    pub gradient: Option<LinearGradientStyle>,
    pub opacity: Option<f32>,
    pub outline: Option<OutlineStyle>,
    pub drop_shadow: Option<DropShadowStyle>,
    pub glow: Option<GlowStyle>,
    pub blend_mode: Option<BlendMode>,
    pub stabilization_strength: Option<f32>,
    pub source: StylePresetSource,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntrancePreset {
    pub id: String,
    pub display_name: String,
    pub entrance: EntranceBehavior,
    pub post_actions: Vec<PostAction>,
    pub source: StylePresetSource,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClearPreset {
    pub id: String,
    pub display_name: String,
    pub kind: Option<ClearKind>,
    pub duration_ms: Option<i64>,
    pub granularity: Option<ClearTargetGranularity>,
    pub ordering: Option<ClearOrdering>,
    pub source: StylePresetSource,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ComboPresetRefs {
    pub style_preset_id: Option<String>,
    pub entrance_preset_id: Option<String>,
    pub clear_preset_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComboPreset {
    pub id: String,
    pub display_name: String,
    pub refs: ComboPresetRefs,
    pub style_override: Option<BaseStylePreset>,
    pub entrance_override: Option<EntrancePreset>,
    pub clear_override: Option<ClearPreset>,
    pub source: StylePresetSource,
    pub file_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PresetCatalogs {
    pub style_presets: Vec<BaseStylePreset>,
    pub entrance_presets: Vec<EntrancePreset>,
    pub clear_presets: Vec<ClearPreset>,
    pub combo_presets: Vec<ComboPreset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StylePresetSource {
    BuiltIn,
    User,
}

impl Default for StylePresetSource {
    fn default() -> Self {
        Self::BuiltIn
    }
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

    pub fn family(&self, family_id: &str) -> Option<&ExportFamily> {
        self.families.get(family_id)
    }

    pub fn families_for_tier(&self, runtime_tier: RuntimeTier) -> Vec<&ExportFamily> {
        self.families
            .values()
            .filter(|family| family.runtime_tier == runtime_tier)
            .collect()
    }

    pub fn profiles_for_family(&self, family_id: &str) -> Vec<&DistributionProfile> {
        self.profiles
            .values()
            .filter(|profile| profile.is_compatible_with(family_id))
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
    IncompatibleSelection {
        family_id: String,
        profile_id: String,
    },
}

#[derive(Debug, Error)]
pub enum ProfileLoadError {
    #[error("profile read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("profile parse failed in {path}: {source}")]
    Parse { path: PathBuf, source: json5::Error },
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

#[derive(Debug, Error)]
pub enum StylePresetLoadError {
    #[error("style preset read failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("style preset parse failed in {path}: {source}")]
    Parse { path: PathBuf, source: json5::Error },
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

pub fn load_base_style_presets_from_dir(
    preset_dir: &Path,
) -> Result<Vec<BaseStylePreset>, StylePresetLoadError> {
    load_base_style_presets_overlay(preset_dir, None)
}

#[derive(Debug, Clone, Copy)]
pub struct PresetDirectorySet<'a> {
    pub style_dir: &'a Path,
    pub entrance_dir: Option<&'a Path>,
    pub clear_dir: Option<&'a Path>,
    pub combo_dir: Option<&'a Path>,
}

pub fn load_preset_catalogs_overlay(
    builtin_dirs: PresetDirectorySet<'_>,
    user_dirs: Option<PresetDirectorySet<'_>>,
) -> Result<PresetCatalogs, StylePresetLoadError> {
    let style_presets = load_base_style_presets_overlay(
        builtin_dirs.style_dir,
        user_dirs.map(|dirs| dirs.style_dir),
    )?;

    let mut entrance_presets = BTreeMap::new();
    load_entrance_presets_into_map(
        &mut entrance_presets,
        builtin_dirs.entrance_dir,
        Some(builtin_dirs.style_dir),
        StylePresetSource::BuiltIn,
    )?;
    if let Some(user_dirs) = user_dirs {
        load_entrance_presets_into_map(
            &mut entrance_presets,
            user_dirs.entrance_dir,
            Some(user_dirs.style_dir),
            StylePresetSource::User,
        )?;
    }

    let mut clear_presets = BTreeMap::new();
    load_clear_presets_into_map(
        &mut clear_presets,
        builtin_dirs.clear_dir,
        StylePresetSource::BuiltIn,
    )?;
    if let Some(user_dirs) = user_dirs {
        load_clear_presets_into_map(
            &mut clear_presets,
            user_dirs.clear_dir,
            StylePresetSource::User,
        )?;
    }

    let mut combo_presets = BTreeMap::new();
    load_combo_presets_into_map(
        &mut combo_presets,
        builtin_dirs.combo_dir,
        StylePresetSource::BuiltIn,
    )?;
    if let Some(user_dirs) = user_dirs {
        load_combo_presets_into_map(
            &mut combo_presets,
            user_dirs.combo_dir,
            StylePresetSource::User,
        )?;
    }

    Ok(PresetCatalogs {
        style_presets,
        entrance_presets: entrance_presets.into_values().collect(),
        clear_presets: clear_presets.into_values().collect(),
        combo_presets: combo_presets.into_values().collect(),
    })
}

pub fn load_base_style_presets_overlay(
    builtin_dir: &Path,
    user_dir: Option<&Path>,
) -> Result<Vec<BaseStylePreset>, StylePresetLoadError> {
    let mut presets = BTreeMap::new();
    for preset in load_base_style_presets_from_single_dir(builtin_dir, StylePresetSource::BuiltIn)?
    {
        presets.insert(preset.id.clone(), preset);
    }

    if let Some(user_dir) = user_dir {
        if user_dir.exists() {
            for preset in
                load_base_style_presets_from_single_dir(user_dir, StylePresetSource::User)?
            {
                presets.insert(preset.id.clone(), preset);
            }
        }
    }

    Ok(presets.into_values().collect())
}

pub fn load_entrance_presets_from_dir(
    preset_dir: &Path,
) -> Result<Vec<EntrancePreset>, StylePresetLoadError> {
    load_entrance_presets_overlay(Some(preset_dir), None, None, None)
}

pub fn load_entrance_presets_overlay(
    builtin_dir: Option<&Path>,
    user_dir: Option<&Path>,
    legacy_builtin_style_dir: Option<&Path>,
    legacy_user_style_dir: Option<&Path>,
) -> Result<Vec<EntrancePreset>, StylePresetLoadError> {
    let mut presets = BTreeMap::new();
    load_entrance_presets_into_map(
        &mut presets,
        builtin_dir,
        legacy_builtin_style_dir,
        StylePresetSource::BuiltIn,
    )?;
    load_entrance_presets_into_map(
        &mut presets,
        user_dir,
        legacy_user_style_dir,
        StylePresetSource::User,
    )?;
    Ok(presets.into_values().collect())
}

pub fn load_clear_presets_from_dir(
    preset_dir: &Path,
) -> Result<Vec<ClearPreset>, StylePresetLoadError> {
    load_clear_presets_overlay(Some(preset_dir), None)
}

pub fn load_clear_presets_overlay(
    builtin_dir: Option<&Path>,
    user_dir: Option<&Path>,
) -> Result<Vec<ClearPreset>, StylePresetLoadError> {
    let mut presets = BTreeMap::new();
    load_clear_presets_into_map(&mut presets, builtin_dir, StylePresetSource::BuiltIn)?;
    load_clear_presets_into_map(&mut presets, user_dir, StylePresetSource::User)?;
    Ok(presets.into_values().collect())
}

pub fn load_combo_presets_from_dir(
    preset_dir: &Path,
) -> Result<Vec<ComboPreset>, StylePresetLoadError> {
    load_combo_presets_overlay(Some(preset_dir), None)
}

pub fn load_combo_presets_overlay(
    builtin_dir: Option<&Path>,
    user_dir: Option<&Path>,
) -> Result<Vec<ComboPreset>, StylePresetLoadError> {
    let mut presets = BTreeMap::new();
    load_combo_presets_into_map(&mut presets, builtin_dir, StylePresetSource::BuiltIn)?;
    load_combo_presets_into_map(&mut presets, user_dir, StylePresetSource::User)?;
    Ok(presets.into_values().collect())
}

fn load_base_style_presets_from_single_dir(
    preset_dir: &Path,
    default_source: StylePresetSource,
) -> Result<Vec<BaseStylePreset>, StylePresetLoadError> {
    let mut paths = fs::read_dir(preset_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json5"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut presets = Vec::new();
    for path in paths {
        presets.push(load_base_style_preset_from_path_with_default_source(
            &path,
            default_source,
        )?);
    }
    Ok(presets)
}

fn load_entrance_presets_into_map(
    presets: &mut BTreeMap<String, EntrancePreset>,
    preset_dir: Option<&Path>,
    legacy_style_dir: Option<&Path>,
    default_source: StylePresetSource,
) -> Result<(), StylePresetLoadError> {
    if let Some(preset_dir) = preset_dir {
        if preset_dir.exists() {
            for preset in load_entrance_presets_from_single_dir(preset_dir, default_source)? {
                presets.insert(preset.id.clone(), preset);
            }
        }
    }
    if let Some(legacy_style_dir) = legacy_style_dir {
        if legacy_style_dir.exists() {
            for preset in
                load_legacy_entrance_presets_from_style_dir(legacy_style_dir, default_source)?
            {
                presets.entry(preset.id.clone()).or_insert(preset);
            }
        }
    }
    Ok(())
}

fn load_entrance_presets_from_single_dir(
    preset_dir: &Path,
    default_source: StylePresetSource,
) -> Result<Vec<EntrancePreset>, StylePresetLoadError> {
    let mut paths = fs::read_dir(preset_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json5"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut presets = Vec::new();
    for path in paths {
        presets.push(load_entrance_preset_from_path_with_default_source(
            &path,
            default_source,
        )?);
    }
    Ok(presets)
}

fn load_legacy_entrance_presets_from_style_dir(
    preset_dir: &Path,
    default_source: StylePresetSource,
) -> Result<Vec<EntrancePreset>, StylePresetLoadError> {
    let mut paths = fs::read_dir(preset_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json5"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut presets = Vec::new();
    for path in paths {
        if let Some(preset) =
            load_legacy_entrance_preset_from_style_path_with_default_source(&path, default_source)?
        {
            presets.push(preset);
        }
    }
    Ok(presets)
}

fn load_clear_presets_into_map(
    presets: &mut BTreeMap<String, ClearPreset>,
    preset_dir: Option<&Path>,
    default_source: StylePresetSource,
) -> Result<(), StylePresetLoadError> {
    let Some(preset_dir) = preset_dir else {
        return Ok(());
    };
    if !preset_dir.exists() {
        return Ok(());
    }
    for preset in load_clear_presets_from_single_dir(preset_dir, default_source)? {
        presets.insert(preset.id.clone(), preset);
    }
    Ok(())
}

fn load_clear_presets_from_single_dir(
    preset_dir: &Path,
    default_source: StylePresetSource,
) -> Result<Vec<ClearPreset>, StylePresetLoadError> {
    let mut paths = fs::read_dir(preset_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json5"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut presets = Vec::new();
    for path in paths {
        presets.push(load_clear_preset_from_path_with_default_source(
            &path,
            default_source,
        )?);
    }
    Ok(presets)
}

fn load_combo_presets_into_map(
    presets: &mut BTreeMap<String, ComboPreset>,
    preset_dir: Option<&Path>,
    default_source: StylePresetSource,
) -> Result<(), StylePresetLoadError> {
    let Some(preset_dir) = preset_dir else {
        return Ok(());
    };
    if !preset_dir.exists() {
        return Ok(());
    }
    for preset in load_combo_presets_from_single_dir(preset_dir, default_source)? {
        presets.insert(preset.id.clone(), preset);
    }
    Ok(())
}

fn load_combo_presets_from_single_dir(
    preset_dir: &Path,
    default_source: StylePresetSource,
) -> Result<Vec<ComboPreset>, StylePresetLoadError> {
    let mut paths = fs::read_dir(preset_dir)?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json5"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut presets = Vec::new();
    for path in paths {
        presets.push(load_combo_preset_from_path_with_default_source(
            &path,
            default_source,
        )?);
    }
    Ok(presets)
}

pub fn load_base_style_preset_from_path(
    path: &Path,
) -> Result<BaseStylePreset, StylePresetLoadError> {
    load_base_style_preset_from_path_with_default_source(path, StylePresetSource::BuiltIn)
}

fn load_base_style_preset_from_path_with_default_source(
    path: &Path,
    default_source: StylePresetSource,
) -> Result<BaseStylePreset, StylePresetLoadError> {
    let raw = fs::read_to_string(path)?;
    let file: BaseStylePresetFile =
        json5::from_str(&raw).map_err(|source| StylePresetLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(file.into_preset(path, default_source))
}

pub fn load_entrance_preset_from_path(path: &Path) -> Result<EntrancePreset, StylePresetLoadError> {
    load_entrance_preset_from_path_with_default_source(path, StylePresetSource::BuiltIn)
}

fn load_entrance_preset_from_path_with_default_source(
    path: &Path,
    default_source: StylePresetSource,
) -> Result<EntrancePreset, StylePresetLoadError> {
    let raw = fs::read_to_string(path)?;
    let file: EntrancePresetFile =
        json5::from_str(&raw).map_err(|source| StylePresetLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(file.into_preset(path, default_source))
}

fn load_legacy_entrance_preset_from_style_path_with_default_source(
    path: &Path,
    default_source: StylePresetSource,
) -> Result<Option<EntrancePreset>, StylePresetLoadError> {
    let raw = fs::read_to_string(path)?;
    let file: BaseStylePresetFile =
        json5::from_str(&raw).map_err(|source| StylePresetLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(file.into_legacy_entrance_preset(path, default_source))
}

pub fn load_clear_preset_from_path(path: &Path) -> Result<ClearPreset, StylePresetLoadError> {
    load_clear_preset_from_path_with_default_source(path, StylePresetSource::BuiltIn)
}

fn load_clear_preset_from_path_with_default_source(
    path: &Path,
    default_source: StylePresetSource,
) -> Result<ClearPreset, StylePresetLoadError> {
    let raw = fs::read_to_string(path)?;
    let file: ClearPresetFile =
        json5::from_str(&raw).map_err(|source| StylePresetLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(file.into_preset(path, default_source))
}

pub fn load_combo_preset_from_path(path: &Path) -> Result<ComboPreset, StylePresetLoadError> {
    load_combo_preset_from_path_with_default_source(path, StylePresetSource::BuiltIn)
}

fn load_combo_preset_from_path_with_default_source(
    path: &Path,
    default_source: StylePresetSource,
) -> Result<ComboPreset, StylePresetLoadError> {
    let raw = fs::read_to_string(path)?;
    let file: ComboPresetFile =
        json5::from_str(&raw).map_err(|source| StylePresetLoadError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(file.into_preset(path, default_source))
}

pub fn save_base_style_preset_to_path(
    path: &Path,
    preset: &BaseStylePreset,
) -> Result<(), StylePresetLoadError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(&BaseStylePresetFile::from_preset(preset))
        .expect("style preset should serialize");
    fs::write(path, serialized)?;
    Ok(())
}

pub fn save_entrance_preset_to_path(
    path: &Path,
    preset: &EntrancePreset,
) -> Result<(), StylePresetLoadError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(&EntrancePresetFile::from_preset(preset))
        .expect("entrance preset should serialize");
    fs::write(path, serialized)?;
    Ok(())
}

pub fn save_clear_preset_to_path(
    path: &Path,
    preset: &ClearPreset,
) -> Result<(), StylePresetLoadError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(&ClearPresetFile::from_preset(preset))
        .expect("clear preset should serialize");
    fs::write(path, serialized)?;
    Ok(())
}

pub fn save_combo_preset_to_path(
    path: &Path,
    preset: &ComboPreset,
) -> Result<(), StylePresetLoadError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string_pretty(&ComboPresetFile::from_preset(preset))
        .expect("combo preset should serialize");
    fs::write(path, serialized)?;
    Ok(())
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
    let file: DistributionProfileFile =
        json5::from_str(raw).map_err(|source| ProfileLoadError::Parse {
            path: path.to_path_buf(),
            source,
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

#[derive(Debug, Serialize, Deserialize)]
struct BaseStylePresetFile {
    id: String,
    display_name: String,
    #[serde(default)]
    source: StylePresetSource,
    #[serde(default)]
    base_style: BaseStylePresetStyleFile,
    #[serde(default)]
    entrance: Option<BaseStylePresetEntranceFile>,
}

impl BaseStylePresetFile {
    fn into_preset(self, path: &Path, default_source: StylePresetSource) -> BaseStylePreset {
        let (color_rgba, opacity) = normalize_style_preset_color_and_opacity(
            self.base_style.color_rgba,
            self.base_style.opacity,
        );
        BaseStylePreset {
            id: self.id,
            display_name: self.display_name,
            thickness: self.base_style.thickness,
            color_rgba,
            color_mode: self
                .base_style
                .color_mode
                .map(ColorModeFile::into_domain)
                .or_else(|| {
                    self.base_style
                        .gradient
                        .as_ref()
                        .map(|_| ColorMode::LinearGradient)
                }),
            gradient: self
                .base_style
                .gradient
                .map(LinearGradientStyleFile::into_domain),
            opacity,
            outline: self.base_style.outline.map(OutlineStyleFile::into_domain),
            drop_shadow: self
                .base_style
                .drop_shadow
                .map(DropShadowStyleFile::into_domain),
            glow: self.base_style.glow.map(GlowStyleFile::into_domain),
            blend_mode: self.base_style.blend_mode.map(BlendModeFile::into_domain),
            stabilization_strength: self.base_style.stabilization_strength,
            source: match self.source {
                StylePresetSource::BuiltIn => default_source,
                StylePresetSource::User => StylePresetSource::User,
            },
            file_path: Some(path.to_path_buf()),
        }
    }

    fn into_legacy_entrance_preset(
        self,
        path: &Path,
        default_source: StylePresetSource,
    ) -> Option<EntrancePreset> {
        let entrance = self.entrance?;
        Some(EntrancePreset {
            id: self.id,
            display_name: format!("{} / 出現", self.display_name),
            entrance: entrance.into_domain(),
            post_actions: Vec::new(),
            source: match self.source {
                StylePresetSource::BuiltIn => default_source,
                StylePresetSource::User => StylePresetSource::User,
            },
            file_path: Some(path.to_path_buf()),
        })
    }

    fn from_preset(preset: &BaseStylePreset) -> Self {
        Self {
            id: preset.id.clone(),
            display_name: preset.display_name.clone(),
            source: preset.source,
            base_style: BaseStylePresetStyleFile {
                thickness: preset.thickness,
                color_rgba: preset.color_rgba.map(|rgba| {
                    let mut rgba = float_color_rgba(rgba);
                    rgba[3] = preset.opacity.unwrap_or(rgba[3]).clamp(0.0, 1.0);
                    rgba
                }),
                fill_color_rgba: None,
                color_mode: preset.color_mode.map(ColorModeFile::from_domain),
                gradient: preset
                    .gradient
                    .as_ref()
                    .map(LinearGradientStyleFile::from_domain),
                opacity: preset.opacity,
                outline: preset.outline.as_ref().map(OutlineStyleFile::from_domain),
                drop_shadow: preset
                    .drop_shadow
                    .as_ref()
                    .map(DropShadowStyleFile::from_domain),
                glow: preset.glow.as_ref().map(GlowStyleFile::from_domain),
                blend_mode: preset.blend_mode.map(BlendModeFile::from_domain),
                stabilization_strength: preset.stabilization_strength,
            },
            entrance: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct EntrancePresetFile {
    id: String,
    display_name: String,
    #[serde(default)]
    source: StylePresetSource,
    #[serde(default)]
    entrance: BaseStylePresetEntranceFile,
    #[serde(default)]
    post_actions: Vec<PostActionFile>,
}

impl EntrancePresetFile {
    fn into_preset(self, path: &Path, default_source: StylePresetSource) -> EntrancePreset {
        EntrancePreset {
            id: self.id,
            display_name: self.display_name,
            entrance: self.entrance.into_domain(),
            post_actions: self
                .post_actions
                .into_iter()
                .map(PostActionFile::into_domain)
                .collect(),
            source: match self.source {
                StylePresetSource::BuiltIn => default_source,
                StylePresetSource::User => StylePresetSource::User,
            },
            file_path: Some(path.to_path_buf()),
        }
    }

    fn from_preset(preset: &EntrancePreset) -> Self {
        Self {
            id: preset.id.clone(),
            display_name: preset.display_name.clone(),
            source: preset.source,
            entrance: BaseStylePresetEntranceFile::from_domain(&preset.entrance),
            post_actions: preset
                .post_actions
                .iter()
                .map(PostActionFile::from_domain)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ClearPresetFile {
    id: String,
    display_name: String,
    #[serde(default)]
    source: StylePresetSource,
    #[serde(default)]
    clear: ClearPresetFieldsFile,
}

impl ClearPresetFile {
    fn into_preset(self, path: &Path, default_source: StylePresetSource) -> ClearPreset {
        ClearPreset {
            id: self.id,
            display_name: self.display_name,
            kind: self.clear.kind.map(ClearKindFile::into_domain),
            duration_ms: self.clear.duration_ms,
            granularity: self
                .clear
                .granularity
                .map(ClearTargetGranularityFile::into_domain),
            ordering: self.clear.ordering.map(ClearOrderingFile::into_domain),
            source: match self.source {
                StylePresetSource::BuiltIn => default_source,
                StylePresetSource::User => StylePresetSource::User,
            },
            file_path: Some(path.to_path_buf()),
        }
    }

    fn from_preset(preset: &ClearPreset) -> Self {
        Self {
            id: preset.id.clone(),
            display_name: preset.display_name.clone(),
            source: preset.source,
            clear: ClearPresetFieldsFile {
                kind: preset.kind.map(ClearKindFile::from_domain),
                duration_ms: preset.duration_ms,
                granularity: preset
                    .granularity
                    .map(ClearTargetGranularityFile::from_domain),
                ordering: preset.ordering.map(ClearOrderingFile::from_domain),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ComboPresetFile {
    id: String,
    display_name: String,
    #[serde(default)]
    source: StylePresetSource,
    #[serde(default)]
    refs: ComboPresetRefs,
    #[serde(default)]
    style_override: Option<BaseStylePresetFile>,
    #[serde(default)]
    entrance_override: Option<EntrancePresetFile>,
    #[serde(default)]
    clear_override: Option<ClearPresetFile>,
}

impl ComboPresetFile {
    fn into_preset(self, path: &Path, default_source: StylePresetSource) -> ComboPreset {
        ComboPreset {
            id: self.id,
            display_name: self.display_name,
            refs: self.refs,
            style_override: self
                .style_override
                .map(|override_file| override_file.into_preset(path, default_source)),
            entrance_override: self
                .entrance_override
                .map(|override_file| override_file.into_preset(path, default_source)),
            clear_override: self
                .clear_override
                .map(|override_file| override_file.into_preset(path, default_source)),
            source: match self.source {
                StylePresetSource::BuiltIn => default_source,
                StylePresetSource::User => StylePresetSource::User,
            },
            file_path: Some(path.to_path_buf()),
        }
    }

    fn from_preset(preset: &ComboPreset) -> Self {
        Self {
            id: preset.id.clone(),
            display_name: preset.display_name.clone(),
            source: preset.source,
            refs: preset.refs.clone(),
            style_override: preset
                .style_override
                .as_ref()
                .map(BaseStylePresetFile::from_preset),
            entrance_override: preset
                .entrance_override
                .as_ref()
                .map(EntrancePresetFile::from_preset),
            clear_override: preset
                .clear_override
                .as_ref()
                .map(ClearPresetFile::from_preset),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct BaseStylePresetStyleFile {
    thickness: Option<f32>,
    color_rgba: Option<[f32; 4]>,
    fill_color_rgba: Option<[f32; 4]>,
    color_mode: Option<ColorModeFile>,
    gradient: Option<LinearGradientStyleFile>,
    opacity: Option<f32>,
    outline: Option<OutlineStyleFile>,
    drop_shadow: Option<DropShadowStyleFile>,
    glow: Option<GlowStyleFile>,
    blend_mode: Option<BlendModeFile>,
    stabilization_strength: Option<f32>,
}

impl BaseStylePresetStyleFile {
    fn into_domain(self) -> StyleSnapshot {
        let (color_rgba, opacity) =
            normalize_style_preset_color_and_opacity(self.color_rgba, self.opacity);
        StyleSnapshot {
            thickness: self.thickness.unwrap_or_else(|| StyleSnapshot::default().thickness),
            color: color_rgba
                .map(rgba_color_from_bytes)
                .unwrap_or_else(|| StyleSnapshot::default().color),
            fill_color: self
                .fill_color_rgba
                .map(normalize_color_rgba)
                .map(rgba_color_from_bytes),
            color_mode: self
                .color_mode
                .map(ColorModeFile::into_domain)
                .or_else(|| self.gradient.as_ref().map(|_| ColorMode::LinearGradient))
                .unwrap_or_else(|| StyleSnapshot::default().color_mode),
            gradient: self.gradient.map(LinearGradientStyleFile::into_domain),
            opacity: opacity.unwrap_or_else(|| StyleSnapshot::default().opacity),
            outline: self
                .outline
                .map(OutlineStyleFile::into_domain)
                .unwrap_or_else(|| StyleSnapshot::default().outline),
            drop_shadow: self
                .drop_shadow
                .map(DropShadowStyleFile::into_domain)
                .unwrap_or_else(|| StyleSnapshot::default().drop_shadow),
            glow: self
                .glow
                .map(GlowStyleFile::into_domain)
                .unwrap_or_else(|| StyleSnapshot::default().glow),
            blend_mode: self
                .blend_mode
                .map(BlendModeFile::into_domain)
                .unwrap_or_else(|| StyleSnapshot::default().blend_mode),
            stabilization_strength: self
                .stabilization_strength
                .unwrap_or_else(|| StyleSnapshot::default().stabilization_strength),
        }
    }

    fn from_style_snapshot(style: &StyleSnapshot) -> Self {
        Self {
            thickness: Some(style.thickness),
            color_rgba: Some(float_color_rgba(rgba_color_to_bytes(style.color))),
            fill_color_rgba: style
                .fill_color
                .map(rgba_color_to_bytes)
                .map(float_color_rgba),
            color_mode: Some(ColorModeFile::from_domain(style.color_mode)),
            gradient: style
                .gradient
                .as_ref()
                .map(LinearGradientStyleFile::from_domain),
            opacity: Some(style.opacity),
            outline: Some(OutlineStyleFile::from_domain(&style.outline)),
            drop_shadow: Some(DropShadowStyleFile::from_domain(&style.drop_shadow)),
            glow: Some(GlowStyleFile::from_domain(&style.glow)),
            blend_mode: Some(BlendModeFile::from_domain(style.blend_mode)),
            stabilization_strength: Some(style.stabilization_strength),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct OutlineStyleFile {
    enabled: bool,
    width: f32,
    color_rgba: Option<[f32; 4]>,
}

impl OutlineStyleFile {
    fn into_domain(self) -> OutlineStyle {
        OutlineStyle {
            enabled: self.enabled,
            width: self.width,
            color: self
                .color_rgba
                .map(normalize_color_rgba)
                .map(rgba_color_from_bytes)
                .unwrap_or_else(|| OutlineStyle::default().color),
        }
    }

    fn from_domain(style: &OutlineStyle) -> Self {
        Self {
            enabled: style.enabled,
            width: style.width,
            color_rgba: Some(float_color_rgba(rgba_color_to_bytes(style.color))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct DropShadowStyleFile {
    enabled: bool,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    color_rgba: Option<[f32; 4]>,
}

impl DropShadowStyleFile {
    fn into_domain(self) -> DropShadowStyle {
        DropShadowStyle {
            enabled: self.enabled,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            blur_radius: self.blur_radius,
            color: self
                .color_rgba
                .map(normalize_color_rgba)
                .map(rgba_color_from_bytes)
                .unwrap_or_else(|| DropShadowStyle::default().color),
        }
    }

    fn from_domain(style: &DropShadowStyle) -> Self {
        Self {
            enabled: style.enabled,
            offset_x: style.offset_x,
            offset_y: style.offset_y,
            blur_radius: style.blur_radius,
            color_rgba: Some(float_color_rgba(rgba_color_to_bytes(style.color))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct GlowStyleFile {
    enabled: bool,
    blur_radius: f32,
    color_rgba: Option<[f32; 4]>,
}

impl GlowStyleFile {
    fn into_domain(self) -> GlowStyle {
        GlowStyle {
            enabled: self.enabled,
            blur_radius: self.blur_radius,
            color: self
                .color_rgba
                .map(normalize_color_rgba)
                .map(rgba_color_from_bytes)
                .unwrap_or_else(|| GlowStyle::default().color),
        }
    }

    fn from_domain(style: &GlowStyle) -> Self {
        Self {
            enabled: style.enabled,
            blur_radius: style.blur_radius,
            color_rgba: Some(float_color_rgba(rgba_color_to_bytes(style.color))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BlendModeFile {
    Normal,
    Multiply,
    Screen,
    Additive,
}

impl BlendModeFile {
    fn into_domain(self) -> BlendMode {
        match self {
            Self::Normal => BlendMode::Normal,
            Self::Multiply => BlendMode::Multiply,
            Self::Screen => BlendMode::Screen,
            Self::Additive => BlendMode::Additive,
        }
    }

    fn from_domain(mode: BlendMode) -> Self {
        match mode {
            BlendMode::Normal => Self::Normal,
            BlendMode::Multiply => Self::Multiply,
            BlendMode::Screen => Self::Screen,
            BlendMode::Additive => Self::Additive,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ColorModeFile {
    Solid,
    LinearGradient,
}

impl ColorModeFile {
    fn into_domain(self) -> ColorMode {
        match self {
            Self::Solid => ColorMode::Solid,
            Self::LinearGradient => ColorMode::LinearGradient,
        }
    }

    fn from_domain(mode: ColorMode) -> Self {
        match mode {
            ColorMode::Solid => Self::Solid,
            ColorMode::LinearGradient => Self::LinearGradient,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GradientSpaceFile {
    Stroke,
    GlyphObject,
    Canvas,
}

impl GradientSpaceFile {
    fn into_domain(self) -> GradientSpace {
        match self {
            Self::Stroke => GradientSpace::Stroke,
            Self::GlyphObject => GradientSpace::GlyphObject,
            Self::Canvas => GradientSpace::Canvas,
        }
    }

    fn from_domain(scope: GradientSpace) -> Self {
        match scope {
            GradientSpace::Stroke => Self::Stroke,
            GradientSpace::GlyphObject => Self::GlyphObject,
            GradientSpace::Canvas => Self::Canvas,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GradientRepeatFile {
    None,
    Repeat,
    Mirror,
}

impl GradientRepeatFile {
    fn into_domain(self) -> GradientRepeat {
        match self {
            Self::None => GradientRepeat::None,
            Self::Repeat => GradientRepeat::Repeat,
            Self::Mirror => GradientRepeat::Mirror,
        }
    }

    fn from_domain(repeat: GradientRepeat) -> Self {
        match repeat {
            GradientRepeat::None => Self::None,
            GradientRepeat::Repeat => Self::Repeat,
            GradientRepeat::Mirror => Self::Mirror,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct ColorStopFile {
    position: f32,
    color_rgba: Option<[f32; 4]>,
}

impl ColorStopFile {
    fn into_domain(self) -> ColorStop {
        ColorStop {
            position: self.position,
            color: self
                .color_rgba
                .map(normalize_color_rgba)
                .map(rgba_color_from_bytes)
                .unwrap_or_default(),
        }
    }

    fn from_domain(stop: &ColorStop) -> Self {
        Self {
            position: stop.position,
            color_rgba: Some(float_color_rgba(rgba_color_to_bytes(stop.color))),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct LinearGradientStyleFile {
    scope: Option<GradientSpaceFile>,
    repeat: Option<GradientRepeatFile>,
    angle_degrees: Option<f32>,
    span_ratio: Option<f32>,
    offset_ratio: Option<f32>,
    stops: Vec<ColorStopFile>,
}

impl LinearGradientStyleFile {
    fn into_domain(self) -> LinearGradientStyle {
        let mut gradient = LinearGradientStyle::default();
        if let Some(scope) = self.scope {
            gradient.scope = scope.into_domain();
        }
        if let Some(repeat) = self.repeat {
            gradient.repeat = repeat.into_domain();
        }
        if let Some(angle_degrees) = self.angle_degrees {
            gradient.angle_degrees = angle_degrees;
        }
        if let Some(span_ratio) = self.span_ratio {
            gradient.span_ratio = span_ratio;
        }
        if let Some(offset_ratio) = self.offset_ratio {
            gradient.offset_ratio = offset_ratio;
        }
        if !self.stops.is_empty() {
            gradient.stops = self
                .stops
                .into_iter()
                .map(ColorStopFile::into_domain)
                .collect();
        }
        gradient
    }

    fn from_domain(gradient: &LinearGradientStyle) -> Self {
        Self {
            scope: Some(GradientSpaceFile::from_domain(gradient.scope)),
            repeat: Some(GradientRepeatFile::from_domain(gradient.repeat)),
            angle_degrees: Some(gradient.angle_degrees),
            span_ratio: Some(gradient.span_ratio),
            offset_ratio: Some(gradient.offset_ratio),
            stops: gradient
                .stops
                .iter()
                .map(ColorStopFile::from_domain)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct BaseStylePresetEntranceFile {
    kind: Option<EntranceKindFile>,
    #[serde(alias = "target")]
    scope: Option<EffectScopeFile>,
    order: Option<EffectOrderFile>,
    duration_mode: Option<EntranceDurationModeFile>,
    duration_ms: Option<i64>,
    speed_scalar: Option<f32>,
    head_effect: Option<RevealHeadEffectFile>,
}

impl BaseStylePresetEntranceFile {
    fn into_domain(self) -> EntranceBehavior {
        let mut entrance = EntranceBehavior::default();
        if let Some(kind) = self.kind {
            entrance.kind = kind.into_domain();
        }
        if let Some(scope) = self.scope {
            entrance.scope = scope.into_domain();
        }
        if let Some(order) = self.order {
            entrance.order = order.into_domain();
        }
        if let Some(duration_mode) = self.duration_mode {
            entrance.duration_mode = duration_mode.into_domain();
        }
        if let Some(duration_ms) = self.duration_ms {
            entrance.duration = MediaDuration::from_millis(duration_ms.max(0));
        }
        if let Some(speed_scalar) = self.speed_scalar {
            entrance.speed_scalar = speed_scalar;
        }
        if let Some(head_effect) = self.head_effect {
            entrance.head_effect = Some(head_effect.into_domain());
        }
        entrance
    }

    fn from_domain(entrance: &EntranceBehavior) -> Self {
        Self {
            kind: Some(EntranceKindFile::from_domain(entrance.kind)),
            scope: Some(EffectScopeFile::from_domain(entrance.scope)),
            order: Some(EffectOrderFile::from_domain(entrance.order)),
            duration_mode: Some(EntranceDurationModeFile::from_domain(
                entrance.duration_mode,
            )),
            duration_ms: Some(media_duration_millis(entrance.duration)),
            speed_scalar: Some(entrance.speed_scalar),
            head_effect: entrance
                .head_effect
                .as_ref()
                .map(RevealHeadEffectFile::from_domain),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct PostActionFile {
    timing_scope: Option<PostActionTimingScopeFile>,
    action: PostActionKindFile,
}

impl Default for PostActionFile {
    fn default() -> Self {
        Self {
            timing_scope: Some(PostActionTimingScopeFile::AfterGlyphObject),
            action: PostActionKindFile::NoOp,
        }
    }
}

impl PostActionFile {
    fn into_domain(self) -> PostAction {
        PostAction {
            timing_scope: self
                .timing_scope
                .unwrap_or(PostActionTimingScopeFile::AfterGlyphObject)
                .into_domain(),
            action: self.action.into_domain(),
        }
    }

    fn from_domain(action: &PostAction) -> Self {
        Self {
            timing_scope: Some(PostActionTimingScopeFile::from_domain(action.timing_scope)),
            action: PostActionKindFile::from_domain(&action.action),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PostActionTimingScopeFile {
    DuringReveal,
    AfterStroke,
    AfterGlyphObject,
    AfterGroup,
    AfterRun,
}

impl PostActionTimingScopeFile {
    fn into_domain(self) -> PostActionTimingScope {
        match self {
            Self::DuringReveal => PostActionTimingScope::DuringReveal,
            Self::AfterStroke => PostActionTimingScope::AfterStroke,
            Self::AfterGlyphObject => PostActionTimingScope::AfterGlyphObject,
            Self::AfterGroup => PostActionTimingScope::AfterGroup,
            Self::AfterRun => PostActionTimingScope::AfterRun,
        }
    }

    fn from_domain(scope: PostActionTimingScope) -> Self {
        match scope {
            PostActionTimingScope::DuringReveal => Self::DuringReveal,
            PostActionTimingScope::AfterStroke => Self::AfterStroke,
            PostActionTimingScope::AfterGlyphObject => Self::AfterGlyphObject,
            PostActionTimingScope::AfterGroup => Self::AfterGroup,
            PostActionTimingScope::AfterRun => Self::AfterRun,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PostActionKindFile {
    NoOp,
    StyleChange {
        style: BaseStylePresetStyleFile,
    },
    InterpolatedStyleChange {
        style: BaseStylePresetStyleFile,
        duration_ms: i64,
    },
    Pulse {
        cycles: u32,
        duration_ms: i64,
    },
    Blink {
        cycles: u32,
        duration_ms: i64,
    },
}

impl PostActionKindFile {
    fn into_domain(self) -> PostActionKind {
        match self {
            Self::NoOp => PostActionKind::NoOp,
            Self::StyleChange { style } => PostActionKind::StyleChange {
                style: style.into_domain(),
            },
            Self::InterpolatedStyleChange { style, duration_ms } => {
                PostActionKind::InterpolatedStyleChange {
                    style: style.into_domain(),
                    duration: MediaDuration::from_millis(duration_ms.max(0)),
                }
            }
            Self::Pulse {
                cycles,
                duration_ms,
            } => PostActionKind::Pulse {
                cycles,
                duration: MediaDuration::from_millis(duration_ms.max(0)),
            },
            Self::Blink {
                cycles,
                duration_ms,
            } => PostActionKind::Blink {
                cycles,
                duration: MediaDuration::from_millis(duration_ms.max(0)),
            },
        }
    }

    fn from_domain(kind: &PostActionKind) -> Self {
        match kind {
            PostActionKind::NoOp => Self::NoOp,
            PostActionKind::StyleChange { style } => Self::StyleChange {
                style: BaseStylePresetStyleFile::from_style_snapshot(style),
            },
            PostActionKind::InterpolatedStyleChange { style, duration } => {
                Self::InterpolatedStyleChange {
                    style: BaseStylePresetStyleFile::from_style_snapshot(style),
                    duration_ms: media_duration_millis(*duration),
                }
            }
            PostActionKind::Pulse { cycles, duration } => Self::Pulse {
                cycles: *cycles,
                duration_ms: media_duration_millis(*duration),
            },
            PostActionKind::Blink { cycles, duration } => Self::Blink {
                cycles: *cycles,
                duration_ms: media_duration_millis(*duration),
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct ClearPresetFieldsFile {
    kind: Option<ClearKindFile>,
    duration_ms: Option<i64>,
    granularity: Option<ClearTargetGranularityFile>,
    ordering: Option<ClearOrderingFile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EntranceKindFile {
    PathTrace,
    Instant,
    Wipe,
    Dissolve,
}

impl EntranceKindFile {
    fn into_domain(self) -> EntranceKind {
        match self {
            Self::PathTrace => EntranceKind::PathTrace,
            Self::Instant => EntranceKind::Instant,
            Self::Wipe => EntranceKind::Wipe,
            Self::Dissolve => EntranceKind::Dissolve,
        }
    }

    fn from_domain(kind: EntranceKind) -> Self {
        match kind {
            EntranceKind::PathTrace => Self::PathTrace,
            EntranceKind::Instant => Self::Instant,
            EntranceKind::Wipe => Self::Wipe,
            EntranceKind::Dissolve => Self::Dissolve,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectScopeFile {
    Stroke,
    Glyph,
    Group,
    Run,
}

impl EffectScopeFile {
    fn into_domain(self) -> pauseink_domain::EffectScope {
        match self {
            Self::Stroke => pauseink_domain::EffectScope::Stroke,
            Self::Glyph => pauseink_domain::EffectScope::GlyphObject,
            Self::Group => pauseink_domain::EffectScope::Group,
            Self::Run => pauseink_domain::EffectScope::Run,
        }
    }

    fn from_domain(scope: pauseink_domain::EffectScope) -> Self {
        match scope {
            pauseink_domain::EffectScope::Stroke => Self::Stroke,
            pauseink_domain::EffectScope::GlyphObject => Self::Glyph,
            pauseink_domain::EffectScope::Group => Self::Group,
            pauseink_domain::EffectScope::Run => Self::Run,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EffectOrderFile {
    Serial,
    Reverse,
    Parallel,
}

impl EffectOrderFile {
    fn into_domain(self) -> pauseink_domain::EffectOrder {
        match self {
            Self::Serial => pauseink_domain::EffectOrder::Serial,
            Self::Reverse => pauseink_domain::EffectOrder::Reverse,
            Self::Parallel => pauseink_domain::EffectOrder::Parallel,
        }
    }

    fn from_domain(order: pauseink_domain::EffectOrder) -> Self {
        match order {
            pauseink_domain::EffectOrder::Serial => Self::Serial,
            pauseink_domain::EffectOrder::Reverse => Self::Reverse,
            pauseink_domain::EffectOrder::Parallel => Self::Parallel,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EntranceDurationModeFile {
    LengthProportional,
    FixedTotalDuration,
}

impl EntranceDurationModeFile {
    fn into_domain(self) -> EntranceDurationMode {
        match self {
            Self::LengthProportional => EntranceDurationMode::ProportionalToStrokeLength,
            Self::FixedTotalDuration => EntranceDurationMode::FixedTotalDuration,
        }
    }

    fn from_domain(mode: EntranceDurationMode) -> Self {
        match mode {
            EntranceDurationMode::ProportionalToStrokeLength => Self::LengthProportional,
            EntranceDurationMode::FixedTotalDuration => Self::FixedTotalDuration,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RevealHeadKindFile {
    Solid,
    Glow,
    CometTail,
}

impl RevealHeadKindFile {
    fn into_domain(self) -> RevealHeadKind {
        match self {
            Self::Solid => RevealHeadKind::SolidHead,
            Self::Glow => RevealHeadKind::GlowHead,
            Self::CometTail => RevealHeadKind::CometTail,
        }
    }

    fn from_domain(kind: RevealHeadKind) -> Self {
        match kind {
            RevealHeadKind::SolidHead => Self::Solid,
            RevealHeadKind::GlowHead => Self::Glow,
            RevealHeadKind::CometTail => Self::CometTail,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum RevealHeadColorSourceFile {
    Keyword(RevealHeadColorKeywordFile),
    Custom([f32; 4]),
}

impl RevealHeadColorSourceFile {
    fn into_domain(self) -> RevealHeadColorSource {
        match self {
            Self::Keyword(RevealHeadColorKeywordFile::PresetAccent) => {
                RevealHeadColorSource::PresetAccent
            }
            Self::Keyword(RevealHeadColorKeywordFile::StrokeColor) => {
                RevealHeadColorSource::StrokeColor
            }
            Self::Custom(color) => {
                RevealHeadColorSource::Custom(rgba_color_from_bytes(normalize_color_rgba(color)))
            }
        }
    }

    fn from_domain(source: &RevealHeadColorSource) -> Self {
        match source {
            RevealHeadColorSource::PresetAccent => {
                Self::Keyword(RevealHeadColorKeywordFile::PresetAccent)
            }
            RevealHeadColorSource::StrokeColor => {
                Self::Keyword(RevealHeadColorKeywordFile::StrokeColor)
            }
            RevealHeadColorSource::Custom(color) => {
                Self::Custom(float_color_rgba(rgba_color_to_bytes(*color)))
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RevealHeadColorKeywordFile {
    PresetAccent,
    StrokeColor,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(default)]
struct RevealHeadEffectFile {
    kind: Option<RevealHeadKindFile>,
    color_source: Option<RevealHeadColorSourceFile>,
    size_multiplier: Option<f32>,
    blur_radius: Option<f32>,
    tail_length: Option<f32>,
    persistence: Option<f32>,
    blend_mode: Option<BlendModeFile>,
}

impl RevealHeadEffectFile {
    fn into_domain(self) -> RevealHeadEffect {
        let mut head = RevealHeadEffect::default();
        if let Some(kind) = self.kind {
            head.kind = kind.into_domain();
        }
        if let Some(color_source) = self.color_source {
            head.color_source = color_source.into_domain();
        }
        if let Some(size_multiplier) = self.size_multiplier {
            head.size_multiplier = size_multiplier;
        }
        if let Some(blur_radius) = self.blur_radius {
            head.blur_radius = blur_radius;
        }
        if let Some(tail_length) = self.tail_length {
            head.tail_length = tail_length;
        }
        if let Some(persistence) = self.persistence {
            head.persistence = persistence;
        }
        if let Some(blend_mode) = self.blend_mode {
            head.blend_mode = blend_mode.into_domain();
        }
        head
    }

    fn from_domain(head: &RevealHeadEffect) -> Self {
        Self {
            kind: Some(RevealHeadKindFile::from_domain(head.kind)),
            color_source: Some(RevealHeadColorSourceFile::from_domain(&head.color_source)),
            size_multiplier: Some(head.size_multiplier),
            blur_radius: Some(head.blur_radius),
            tail_length: Some(head.tail_length),
            persistence: Some(head.persistence),
            blend_mode: Some(BlendModeFile::from_domain(head.blend_mode)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClearKindFile {
    Instant,
    Ordered,
    ReverseOrdered,
    WipeOut,
    DissolveOut,
}

impl ClearKindFile {
    fn into_domain(self) -> ClearKind {
        match self {
            Self::Instant => ClearKind::Instant,
            Self::Ordered => ClearKind::Ordered,
            Self::ReverseOrdered => ClearKind::ReverseOrdered,
            Self::WipeOut => ClearKind::WipeOut,
            Self::DissolveOut => ClearKind::DissolveOut,
        }
    }

    fn from_domain(kind: ClearKind) -> Self {
        match kind {
            ClearKind::Instant => Self::Instant,
            ClearKind::Ordered => Self::Ordered,
            ClearKind::ReverseOrdered => Self::ReverseOrdered,
            ClearKind::WipeOut => Self::WipeOut,
            ClearKind::DissolveOut => Self::DissolveOut,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClearTargetGranularityFile {
    Object,
    Group,
    Stroke,
    AllParallel,
}

impl ClearTargetGranularityFile {
    fn into_domain(self) -> ClearTargetGranularity {
        match self {
            Self::Object => ClearTargetGranularity::Object,
            Self::Group => ClearTargetGranularity::Group,
            Self::Stroke => ClearTargetGranularity::Stroke,
            Self::AllParallel => ClearTargetGranularity::AllParallel,
        }
    }

    fn from_domain(value: ClearTargetGranularity) -> Self {
        match value {
            ClearTargetGranularity::Object => Self::Object,
            ClearTargetGranularity::Group => Self::Group,
            ClearTargetGranularity::Stroke => Self::Stroke,
            ClearTargetGranularity::AllParallel => Self::AllParallel,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClearOrderingFile {
    Serial,
    Reverse,
    Parallel,
}

impl ClearOrderingFile {
    fn into_domain(self) -> ClearOrdering {
        match self {
            Self::Serial => ClearOrdering::Serial,
            Self::Reverse => ClearOrdering::Reverse,
            Self::Parallel => ClearOrdering::Parallel,
        }
    }

    fn from_domain(value: ClearOrdering) -> Self {
        match value {
            ClearOrdering::Serial => Self::Serial,
            ClearOrdering::Reverse => Self::Reverse,
            ClearOrdering::Parallel => Self::Parallel,
        }
    }
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
    fn into_profile_compatibility(
        self,
        path: &Path,
    ) -> Result<ProfileCompatibility, ProfileLoadError> {
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

fn normalize_color_rgba(raw: [f32; 4]) -> [u8; 4] {
    raw.map(|component| (component.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn normalize_style_preset_color_and_opacity(
    color_rgba: Option<[f32; 4]>,
    opacity: Option<f32>,
) -> (Option<[u8; 4]>, Option<f32>) {
    let Some(mut rgba) = color_rgba.map(normalize_color_rgba) else {
        return (None, opacity.map(|value| value.clamp(0.0, 1.0)));
    };

    let resolved_opacity = opacity.unwrap_or(rgba[3] as f32 / 255.0).clamp(0.0, 1.0);
    rgba[3] = 255;
    (Some(rgba), Some(resolved_opacity))
}

fn float_color_rgba(raw: [u8; 4]) -> [f32; 4] {
    raw.map(|component| component as f32 / 255.0)
}

fn rgba_color_from_bytes(raw: [u8; 4]) -> RgbaColor {
    RgbaColor::new(raw[0], raw[1], raw[2], raw[3])
}

fn rgba_color_to_bytes(color: RgbaColor) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

fn media_duration_millis(duration: MediaDuration) -> i64 {
    ((duration.ticks as f64 * duration.time_base.numerator as f64 * 1000.0)
        / duration.time_base.denominator as f64)
        .round() as i64
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

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
                compatibility: ProfileCompatibility::Only(vec!["mov_prores_4444_pcm".into()]),
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

    #[test]
    fn profiles_for_family_filters_incompatible_profiles_and_keeps_sorted_order() {
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
            vec![
                DistributionProfile {
                    id: "adobe_alpha".into(),
                    display_name: "Adobe アルファ".into(),
                    source_kind: ProfileSourceKind::AppAuthored,
                    source_urls: vec![],
                    compatibility: ProfileCompatibility::Only(vec!["mov_prores_4444_pcm".into()]),
                    notes: String::new(),
                    public_constraints: ProfilePublicConstraints::default(),
                    settings_buckets: BTreeMap::new(),
                },
                DistributionProfile {
                    id: "custom".into(),
                    display_name: "カスタム".into(),
                    source_kind: ProfileSourceKind::AppAuthored,
                    source_urls: vec![],
                    compatibility: ProfileCompatibility::Any,
                    notes: String::new(),
                    public_constraints: ProfilePublicConstraints::default(),
                    settings_buckets: BTreeMap::new(),
                },
                DistributionProfile {
                    id: "youtube".into(),
                    display_name: "YouTube".into(),
                    source_kind: ProfileSourceKind::Official,
                    source_urls: vec![],
                    compatibility: ProfileCompatibility::Only(vec!["webm_vp9_opus".into()]),
                    notes: String::new(),
                    public_constraints: ProfilePublicConstraints::default(),
                    settings_buckets: BTreeMap::new(),
                },
            ],
        );

        let profile_ids = catalog
            .profiles_for_family("mov_prores_4444_pcm")
            .into_iter()
            .map(|profile| profile.id.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            profile_ids,
            vec!["adobe_alpha".to_owned(), "custom".to_owned()]
        );
        assert_eq!(
            catalog
                .family("mov_prores_4444_pcm")
                .map(|family| family.display_name.clone()),
            Some("MOV / ProRes 4444 / PCM".to_owned())
        );
    }

    #[test]
    fn repository_style_presets_load_from_json5_files() {
        let preset_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/style_presets");
        let presets =
            load_base_style_presets_from_dir(&preset_dir).expect("style presets should load");

        assert!(presets.iter().any(|preset| preset.id == "marker_highlight"));
        assert!(presets
            .iter()
            .any(|preset| preset.id == "white_pen_fastwrite"));
    }

    #[test]
    fn repository_category_presets_load_from_split_directories() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets");
        let catalogs = load_preset_catalogs_overlay(
            PresetDirectorySet {
                style_dir: &repo_root.join("style_presets"),
                entrance_dir: Some(&repo_root.join("entrance_presets")),
                clear_dir: Some(&repo_root.join("clear_presets")),
                combo_dir: Some(&repo_root.join("combo_presets")),
            },
            None,
        )
        .expect("split preset catalogs should load");

        assert!(catalogs
            .entrance_presets
            .iter()
            .any(|preset| preset.id == "white_pen_fastwrite"
                && preset.entrance.head_effect.is_some()));
        assert!(catalogs
            .clear_presets
            .iter()
            .any(|preset| preset.id == "instant_all_parallel"));
        assert!(catalogs
            .combo_presets
            .iter()
            .any(|preset| preset.id == "marker_highlight"));
    }

    #[test]
    fn style_preset_color_is_normalized_into_rgba_bytes() {
        let preset = load_base_style_preset_from_path(Path::new(
            "presets/style_presets/marker_highlight.json5",
        ));
        assert!(preset.is_err());

        assert_eq!(
            normalize_color_rgba([1.0, 0.5, 0.0, 0.25]),
            [255, 128, 0, 64]
        );
    }

    #[test]
    fn user_style_presets_overlay_builtins_and_roundtrip_disk_edits() {
        let temp_dir = tempdir().expect("temp dir");
        let builtin_style_dir = temp_dir.path().join("builtin_style");
        let builtin_entrance_dir = temp_dir.path().join("builtin_entrance");
        let builtin_clear_dir = temp_dir.path().join("builtin_clear");
        let builtin_combo_dir = temp_dir.path().join("builtin_combo");
        let user_style_dir = temp_dir.path().join("user_style");
        let user_entrance_dir = temp_dir.path().join("user_entrance");
        let user_clear_dir = temp_dir.path().join("user_clear");
        let user_combo_dir = temp_dir.path().join("user_combo");
        for dir in [
            &builtin_style_dir,
            &builtin_entrance_dir,
            &builtin_clear_dir,
            &builtin_combo_dir,
            &user_style_dir,
            &user_entrance_dir,
            &user_clear_dir,
            &user_combo_dir,
        ] {
            fs::create_dir_all(dir).expect("preset dir");
        }

        fs::write(
            builtin_style_dir.join("marker.json5"),
            r#"
            {
              id: "marker_highlight",
              display_name: "Built-in Marker",
              base_style: { thickness: 12.0, color_rgba: [1.0, 1.0, 0.0, 0.30] },
            }
            "#,
        )
        .expect("builtin preset");
        fs::write(
            user_style_dir.join("marker.json5"),
            r#"
            {
              id: "marker_highlight",
              display_name: "User Marker",
              base_style: {
                thickness: 18.0,
                color_rgba: [0.0, 0.5, 1.0, 0.60],
                opacity: 0.60,
                blend_mode: "additive",
                outline: {
                  enabled: true,
                  width: 3.0,
                  color_rgba: [0.0, 0.0, 0.0, 1.0],
                },
                drop_shadow: {
                  enabled: true,
                  offset_x: 4.0,
                  offset_y: 6.0,
                  blur_radius: 8.0,
                  color_rgba: [0.1, 0.1, 0.1, 0.8],
                },
                glow: {
                  enabled: true,
                  blur_radius: 10.0,
                  color_rgba: [0.8, 1.0, 1.0, 0.7],
                },
                stabilization_strength: 0.8,
              },
            }
            "#,
        )
        .expect("user preset");
        fs::write(
            builtin_entrance_dir.join("marker.json5"),
            r#"
            {
              id: "marker_highlight",
              display_name: "Built-in Marker Entrance",
              entrance: {
                kind: "instant",
                target: "group",
              },
            }
            "#,
        )
        .expect("builtin entrance preset");
        fs::write(
            user_entrance_dir.join("marker.json5"),
            r#"
            {
              id: "marker_highlight",
              display_name: "User Marker Entrance",
              entrance: {
                kind: "path_trace",
                duration_mode: "length_proportional",
                duration_ms: 900,
                speed_scalar: 1.8,
                head_effect: {
                  kind: "glow",
                  color_source: "stroke_color",
                  size_multiplier: 1.3,
                },
              },
            }
            "#,
        )
        .expect("user entrance preset");
        fs::write(
            builtin_clear_dir.join("instant.json5"),
            r#"
            {
              id: "instant_screen_clear",
              display_name: "Instant Clear",
              clear: {
                kind: "instant",
                duration_ms: 0,
                granularity: "all_parallel",
                ordering: "parallel",
              },
            }
            "#,
        )
        .expect("builtin clear preset");
        fs::write(
            builtin_combo_dir.join("marker_combo.json5"),
            r#"
            {
              id: "marker_combo",
              display_name: "Marker Combo",
              refs: {
                style_preset_id: "marker_highlight",
                entrance_preset_id: "marker_highlight",
                clear_preset_id: "instant_screen_clear",
              },
            }
            "#,
        )
        .expect("builtin combo preset");

        let catalogs = load_preset_catalogs_overlay(
            PresetDirectorySet {
                style_dir: &builtin_style_dir,
                entrance_dir: Some(&builtin_entrance_dir),
                clear_dir: Some(&builtin_clear_dir),
                combo_dir: Some(&builtin_combo_dir),
            },
            Some(PresetDirectorySet {
                style_dir: &user_style_dir,
                entrance_dir: Some(&user_entrance_dir),
                clear_dir: Some(&user_clear_dir),
                combo_dir: Some(&user_combo_dir),
            }),
        )
        .expect("overlay preset load should succeed");
        let marker = catalogs
            .style_presets
            .iter()
            .find(|preset| preset.id == "marker_highlight")
            .expect("merged marker preset");
        assert_eq!(marker.display_name, "User Marker");
        assert_eq!(marker.source, StylePresetSource::User);
        assert_eq!(marker.thickness, Some(18.0));
        assert_eq!(marker.opacity, Some(0.60));
        assert_eq!(marker.stabilization_strength, Some(0.8));
        assert_eq!(marker.blend_mode, Some(BlendMode::Additive));
        assert!(marker
            .outline
            .as_ref()
            .is_some_and(|outline| outline.enabled));
        assert!(marker
            .drop_shadow
            .as_ref()
            .is_some_and(|shadow| shadow.enabled));
        assert!(marker.glow.as_ref().is_some_and(|glow| glow.enabled));
        let entrance = catalogs
            .entrance_presets
            .iter()
            .find(|preset| preset.id == "marker_highlight")
            .expect("merged entrance preset");
        assert_eq!(entrance.entrance.kind, EntranceKind::PathTrace);
        assert!(entrance.entrance.head_effect.is_some());
        assert!(catalogs
            .clear_presets
            .iter()
            .any(|preset| preset.id == "instant_screen_clear"));
        assert!(catalogs
            .combo_presets
            .iter()
            .any(|preset| preset.id == "marker_combo"));

        let custom_path = user_style_dir.join("custom_soft_marker.json5");
        let custom = BaseStylePreset {
            id: "custom_soft_marker".to_owned(),
            display_name: "Custom Soft Marker".to_owned(),
            thickness: Some(9.5),
            color_rgba: Some([32, 200, 255, 255]),
            color_mode: Some(ColorMode::LinearGradient),
            gradient: Some(LinearGradientStyle {
                scope: GradientSpace::Canvas,
                repeat: GradientRepeat::Mirror,
                angle_degrees: 24.0,
                span_ratio: 0.8,
                offset_ratio: -0.35,
                stops: vec![
                    ColorStop {
                        position: 0.0,
                        color: RgbaColor::new(255, 200, 120, 255),
                    },
                    ColorStop {
                        position: 0.55,
                        color: RgbaColor::new(255, 96, 180, 255),
                    },
                    ColorStop {
                        position: 1.0,
                        color: RgbaColor::new(96, 128, 255, 255),
                    },
                ],
            }),
            opacity: Some(0.35),
            outline: Some(OutlineStyle {
                enabled: true,
                width: 2.5,
                color: RgbaColor::new(16, 32, 48, 255),
            }),
            drop_shadow: Some(DropShadowStyle {
                enabled: true,
                offset_x: 2.0,
                offset_y: 5.0,
                blur_radius: 7.0,
                color: RgbaColor::new(10, 10, 10, 180),
            }),
            glow: Some(GlowStyle {
                enabled: true,
                blur_radius: 9.0,
                color: RgbaColor::new(200, 255, 255, 150),
            }),
            blend_mode: Some(BlendMode::Screen),
            stabilization_strength: Some(0.8),
            source: StylePresetSource::User,
            file_path: None,
        };
        save_base_style_preset_to_path(&custom_path, &custom).expect("preset save should succeed");

        let loaded =
            load_base_style_preset_from_path(&custom_path).expect("saved preset should load");
        assert_eq!(loaded.id, "custom_soft_marker");
        assert_eq!(loaded.display_name, "Custom Soft Marker");
        assert_eq!(loaded.thickness, Some(9.5));
        assert_eq!(loaded.opacity, Some(0.35));
        assert_eq!(loaded.color_mode, Some(ColorMode::LinearGradient));
        let loaded_gradient = loaded.gradient.as_ref().expect("gradient should load");
        assert_eq!(loaded_gradient.scope, GradientSpace::Canvas);
        assert_eq!(loaded_gradient.repeat, GradientRepeat::Mirror);
        assert!((loaded_gradient.span_ratio - 0.8).abs() < 0.001);
        assert!((loaded_gradient.offset_ratio - -0.35).abs() < 0.001);
        assert_eq!(loaded_gradient.stops.len(), 3);
        assert_eq!(loaded.stabilization_strength, Some(0.8));
        assert_eq!(loaded.blend_mode, Some(BlendMode::Screen));
        assert!(loaded
            .outline
            .as_ref()
            .is_some_and(|outline| outline.enabled));
        assert!(loaded
            .drop_shadow
            .as_ref()
            .is_some_and(|shadow| shadow.enabled));
        assert!(loaded.glow.as_ref().is_some_and(|glow| glow.enabled));
        assert_eq!(loaded.source, StylePresetSource::User);

        let entrance_path = user_entrance_dir.join("trace_head.json5");
        let entrance_preset = EntrancePreset {
            id: "trace_head".to_owned(),
            display_name: "Trace Head".to_owned(),
            entrance: EntranceBehavior {
                kind: EntranceKind::Dissolve,
                scope: EffectScope::GlyphObject,
                order: EffectOrder::Serial,
                duration_mode: EntranceDurationMode::FixedTotalDuration,
                duration: MediaDuration::from_millis(1200),
                speed_scalar: 1.5,
                head_effect: Some(RevealHeadEffect {
                    kind: RevealHeadKind::GlowHead,
                    color_source: RevealHeadColorSource::StrokeColor,
                    size_multiplier: 1.4,
                    blur_radius: 5.0,
                    tail_length: 0.0,
                    persistence: 0.2,
                    blend_mode: BlendMode::Screen,
                }),
            },
            post_actions: vec![pauseink_domain::PostAction {
                timing_scope: pauseink_domain::PostActionTimingScope::AfterGlyphObject,
                action: pauseink_domain::PostActionKind::StyleChange {
                    style: pauseink_domain::StyleSnapshot {
                        color: RgbaColor::new(255, 200, 120, 255),
                        opacity: 0.34,
                        ..pauseink_domain::StyleSnapshot::default()
                    },
                },
            }],
            source: StylePresetSource::User,
            file_path: None,
        };
        save_entrance_preset_to_path(&entrance_path, &entrance_preset)
            .expect("entrance preset save should succeed");

        let loaded_entrance =
            load_entrance_preset_from_path(&entrance_path).expect("saved entrance preset");
        assert_eq!(loaded_entrance.id, "trace_head");
        assert_eq!(loaded_entrance.entrance.kind, EntranceKind::Dissolve);
        assert!(loaded_entrance.entrance.head_effect.is_some());
        assert_eq!(loaded_entrance.post_actions.len(), 1);
        match &loaded_entrance.post_actions[0].action {
            pauseink_domain::PostActionKind::StyleChange { style } => {
                assert_eq!(style.color, RgbaColor::new(255, 200, 120, 255));
                assert!((style.opacity - 0.34).abs() < 0.001);
            }
            other => panic!("unexpected post action: {other:?}"),
        }
    }
}
