use std::path::{Path, PathBuf};

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
}
