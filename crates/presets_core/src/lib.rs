use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeTier {
    Mainline,
    OptionalCodecPack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
pub enum ProfileSourceKind {
    Official,
    AppAuthored,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionProfile {
    pub id: String,
    pub display_name: String,
    pub source_kind: ProfileSourceKind,
    pub source_urls: Vec<String>,
    pub compatibility: ProfileCompatibility,
    pub notes: String,
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

#[cfg(test)]
mod tests {
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
}
