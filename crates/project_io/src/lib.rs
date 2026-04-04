use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectHeader {
    pub format_version: String,
}

pub fn canonicalize_format_version(raw: &str) -> String {
    raw.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_trims() {
        assert_eq!(canonicalize_format_version(" 1.0.0 "), "1.0.0");
    }
}
