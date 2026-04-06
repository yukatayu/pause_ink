use std::io;
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

    pub fn user_style_presets_dir(&self) -> PathBuf {
        self.config_dir.join("style_presets")
    }

    pub fn google_fonts_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("google_fonts")
    }

    pub fn font_index_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("font_index")
    }

    pub fn media_probe_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("media_probe")
    }

    pub fn thumbnail_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("thumbnails")
    }

    pub fn runtime_ffmpeg_dir(&self) -> PathBuf {
        self.runtime_dir.join("ffmpeg")
    }

    pub fn autosave_file(&self, stem: &str) -> PathBuf {
        self.autosave_dir.join(format!("{stem}.pauseink"))
    }

    pub fn ensure_exists(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.user_style_presets_dir())?;
        std::fs::create_dir_all(&self.logs_dir)?;
        std::fs::create_dir_all(&self.autosave_dir)?;
        std::fs::create_dir_all(&self.temp_dir)?;
        std::fs::create_dir_all(&self.google_fonts_cache_dir())?;
        std::fs::create_dir_all(&self.font_index_cache_dir())?;
        std::fs::create_dir_all(&self.media_probe_cache_dir())?;
        std::fs::create_dir_all(&self.thumbnail_cache_dir())?;
        std::fs::create_dir_all(&self.runtime_ffmpeg_dir())?;
        Ok(())
    }
}

pub fn portable_root(executable_dir: &Path) -> PathBuf {
    portable_root_with_override(executable_dir, None)
}

pub fn portable_root_with_override(executable_dir: &Path, override_root: Option<&Path>) -> PathBuf {
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
    pub autosave_interval_seconds: u64,
    pub stroke_stabilization_default: u8,
    pub google_fonts: GoogleFontsSettings,
    pub local_font_dirs: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            history_depth: 256,
            guide_modifier: "platform_default".to_owned(),
            guide_slope_degrees: 0.0,
            gpu_preview_enabled: true,
            media_hwaccel_enabled: true,
            autosave_interval_seconds: 10,
            stroke_stabilization_default: 35,
            google_fonts: GoogleFontsSettings::default(),
            local_font_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum PortableFsError {
    #[error("settings parse error: {0}")]
    Parse(#[from] json5::Error),
    #[error("settings serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("portable filesystem I/O error: {0}")]
    Io(#[from] io::Error),
}

pub fn load_settings_from_str(source: &str) -> Result<Settings, PortableFsError> {
    Ok(json5::from_str(source)?)
}

pub fn save_settings_to_string(settings: &Settings) -> Result<String, PortableFsError> {
    Ok(serde_json::to_string_pretty(settings)?)
}

pub fn load_settings_from_file(paths: &PortablePaths) -> Result<Settings, PortableFsError> {
    Ok(load_settings_from_str(&std::fs::read_to_string(
        paths.settings_file(),
    )?)?)
}

pub fn load_settings_or_default(paths: &PortablePaths) -> Result<Settings, PortableFsError> {
    match std::fs::read_to_string(paths.settings_file()) {
        Ok(raw) => load_settings_from_str(&raw),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(error) => Err(PortableFsError::Io(error)),
    }
}

pub fn save_settings_to_file(
    paths: &PortablePaths,
    settings: &Settings,
) -> Result<(), PortableFsError> {
    paths.ensure_exists()?;
    std::fs::write(paths.settings_file(), save_settings_to_string(settings)?)?;
    Ok(())
}

pub fn directory_size(path: &Path) -> Result<u64, PortableFsError> {
    if !path.exists() {
        return Ok(0);
    }
    let metadata = std::fs::metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        total += directory_size(&entry.path())?;
    }
    Ok(total)
}

pub fn clear_directory_contents(path: &Path) -> Result<(), PortableFsError> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
        return Ok(());
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry_path)?;
        } else {
            std::fs::remove_file(entry_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_default_to_executable_local_root() {
        let paths = PortablePaths::from_executable_dir(Path::new("/tmp/demo"));

        assert_eq!(paths.root, Path::new("/tmp/demo/pauseink_data"));
        assert_eq!(
            paths.config_dir,
            Path::new("/tmp/demo/pauseink_data/config")
        );
        assert_eq!(
            paths.autosave_dir,
            Path::new("/tmp/demo/pauseink_data/autosave")
        );
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
        assert_eq!(loaded.autosave_interval_seconds, 10);
        assert!(loaded.google_fonts.enabled);
    }

    #[test]
    fn cache_and_autosave_paths_stay_under_portable_root() {
        let paths = PortablePaths::from_executable_dir(Path::new("/tmp/demo"));

        assert_eq!(
            paths.google_fonts_cache_dir(),
            Path::new("/tmp/demo/pauseink_data/cache/google_fonts")
        );
        assert_eq!(
            paths.runtime_ffmpeg_dir(),
            Path::new("/tmp/demo/pauseink_data/runtime/ffmpeg")
        );
        assert_eq!(
            paths.autosave_file("recovery_latest"),
            Path::new("/tmp/demo/pauseink_data/autosave/recovery_latest.pauseink")
        );
        assert_eq!(
            paths.user_style_presets_dir(),
            Path::new("/tmp/demo/pauseink_data/config/style_presets")
        );
    }

    #[test]
    fn settings_file_roundtrip_works_on_disk() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        let settings = Settings::default();

        save_settings_to_file(&paths, &settings).expect("settings save should work");
        let loaded = load_settings_from_file(&paths).expect("settings load should work");

        assert_eq!(loaded.history_depth, settings.history_depth);
        assert!(paths.thumbnail_cache_dir().is_dir());
        assert!(paths.user_style_presets_dir().is_dir());
    }

    #[test]
    fn load_settings_or_default_uses_defaults_when_file_is_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));

        let loaded = load_settings_or_default(&paths).expect("default settings should load");

        assert_eq!(loaded.history_depth, 256);
    }

    #[test]
    fn directory_size_counts_nested_files_and_clear_keeps_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path().join("cache");
        std::fs::create_dir_all(root.join("nested")).expect("nested dir");
        std::fs::write(root.join("a.bin"), vec![0u8; 7]).expect("top file");
        std::fs::write(root.join("nested").join("b.bin"), vec![0u8; 5]).expect("nested file");

        assert_eq!(directory_size(&root).expect("size should compute"), 12);

        clear_directory_contents(&root).expect("cache clear should work");

        assert!(root.is_dir());
        assert_eq!(directory_size(&root).expect("size should recompute"), 0);
    }
}
