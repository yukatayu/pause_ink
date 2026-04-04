use std::path::{Path, PathBuf};

pub fn portable_root(executable_dir: &Path) -> PathBuf {
    executable_dir.join("pauseink_data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_root_is_local() {
        let root = portable_root(Path::new("/tmp/demo"));
        assert!(root.ends_with("pauseink_data"));
    }
}
