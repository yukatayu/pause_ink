use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PORTABLE_ROOT_OVERRIDE_ENV: &str = "PAUSEINK_PORTABLE_ROOT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortablePaths {
    pub root: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub autosave_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub temp_dir: PathBuf,
}

impl PortablePaths {
    pub fn from_executable_dir(executable_dir: &Path) -> Self {
        let root = portable_root(executable_dir);
        Self::from_root(root)
    }

    pub fn from_override_or_executable_dir(
        executable_dir: &Path,
        override_root: Option<&Path>,
    ) -> Self {
        Self::from_root(portable_root_with_override(executable_dir, override_root))
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self {
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            logs_dir: root.join("logs"),
            autosave_dir: root.join("autosave"),
            runtime_dir: root.join("runtime"),
            temp_dir: root.join("temp"),
            root,
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.config_dir.join("settings.json5")
    }
}

pub fn portable_root(executable_dir: &Path) -> PathBuf {
    portable_root_with_override(executable_dir, None)
}

pub fn portable_root_with_override(
    executable_dir: &Path,
    override_root: Option<&Path>,
) -> PathBuf {
    override_root
        .map(Path::to_path_buf)
        .unwrap_or_else(|| executable_dir.join("pauseink_data"))
}

pub fn portable_root_from_env(executable_dir: &Path) -> PathBuf {
    let override_root = std::env::var_os(PORTABLE_ROOT_OVERRIDE_ENV).map(PathBuf::from);
    portable_root_with_override(executable_dir, override_root.as_deref())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoogleFontsSettings {
    pub enabled: bool,
    pub families: Vec<String>,
}

impl Default for GoogleFontsSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            families: vec!["M PLUS Rounded 1c".to_owned(), "Noto Sans JP".to_owned()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub history_depth: usize,
    pub guide_modifier: String,
    pub guide_slope_degrees: f32,
    pub gpu_preview_enabled: bool,
    pub media_hwaccel_enabled: bool,
    pub stroke_stabilization_default: u8,
    pub google_fonts: GoogleFontsSettings,
    pub local_font_dirs: Vec<PathBuf>,
    pub portable_root_override: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_depth: 256,
            guide_modifier: "platform_default".to_owned(),
            guide_slope_degrees: 0.0,
            gpu_preview_enabled: true,
            media_hwaccel_enabled: true,
            stroke_stabilization_default: 35,
            google_fonts: GoogleFontsSettings::default(),
            local_font_dirs: Vec::new(),
            portable_root_override: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum PortableFsError {
    #[error("settings parse error: {0}")]
    Parse(#[from] json5::Error),
    #[error("settings serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub fn load_settings_from_str(source: &str) -> Result<Settings, PortableFsError> {
    Ok(json5::from_str(source)?)
}

pub fn save_settings_to_string(settings: &Settings) -> Result<String, PortableFsError> {
    Ok(serde_json::to_string_pretty(settings)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_default_to_executable_local_root() {
        let paths = PortablePaths::from_executable_dir(Path::new("/tmp/demo"));

        assert_eq!(paths.root, Path::new("/tmp/demo/pauseink_data"));
        assert_eq!(paths.config_dir, Path::new("/tmp/demo/pauseink_data/config"));
        assert_eq!(paths.autosave_dir, Path::new("/tmp/demo/pauseink_data/autosave"));
    }

    #[test]
    fn override_root_replaces_executable_local_root() {
        let root = portable_root_with_override(
            Path::new("/tmp/demo"),
            Some(Path::new("/tmp/pauseink-test-root")),
        );

        assert_eq!(root, Path::new("/tmp/pauseink-test-root"));
    }

    #[test]
    fn env_override_changes_root_and_settings_location() {
        let paths = PortablePaths::from_override_or_executable_dir(
            Path::new("/tmp/demo"),
            Some(Path::new("/tmp/pauseink-env-root")),
        );

        assert_eq!(paths.root, Path::new("/tmp/pauseink-env-root"));
        assert_eq!(
            paths.settings_file(),
            Path::new("/tmp/pauseink-env-root/config/settings.json5")
        );
    }

    #[test]
    fn settings_roundtrip_keeps_required_defaults() {
        let settings = Settings::default();
        let saved = save_settings_to_string(&settings).expect("settings save should succeed");
        let loaded = load_settings_from_str(&saved).expect("settings load should succeed");

        assert_eq!(loaded.history_depth, 256);
        assert!(loaded.gpu_preview_enabled);
        assert!(loaded.media_hwaccel_enabled);
        assert!(loaded.google_fonts.enabled);
    }
}
