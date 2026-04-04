use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportFamily {
    pub id: String,
    pub display_name: String,
    pub supports_alpha: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileCompatibility {
    Any,
    Only(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistributionProfile {
    pub id: String,
    pub display_name: String,
    pub compatibility: ProfileCompatibility,
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
            compatibility: ProfileCompatibility::Only(vec![
                "webm_vp9_opus".into(),
                "webm_av1_opus".into(),
            ]),
        };

        assert!(profile.is_compatible_with("webm_av1_opus"));
        assert!(!profile.is_compatible_with("mov_prores_4444_pcm"));
    }

    #[test]
    fn catalog_resolves_family_and_profile_in_two_layers() {
        let catalog = ExportCatalog::new(
            vec![ExportFamily {
                id: "mov_prores_4444_pcm".into(),
                display_name: "MOV / ProRes 4444 / PCM".into(),
                supports_alpha: true,
            }],
            vec![DistributionProfile {
                id: "adobe_alpha".into(),
                display_name: "Adobe Alpha".into(),
                compatibility: ProfileCompatibility::Only(vec![
                    "mov_prores_4444_pcm".into(),
                ]),
            }],
        );

        let resolved = catalog
            .resolve("mov_prores_4444_pcm", "adobe_alpha")
            .expect("compatible family/profile should resolve");

        assert_eq!(resolved.family.id, "mov_prores_4444_pcm");
        assert_eq!(resolved.profile.id, "adobe_alpha");
    }
}
