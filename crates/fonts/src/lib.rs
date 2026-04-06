use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use fontdb::{Database, Family, Query, Stretch, Style, Weight};
use thiserror::Error;

pub fn google_fonts_css_url(family: &str) -> String {
    format!(
        "https://fonts.googleapis.com/css2?family={}&display=swap",
        encode_query_component(family)
    )
}

pub fn google_font_cache_file(cache_root: &Path, family: &str) -> PathBuf {
    cache_root.join(format!("{}.font", slugify_family_name(family)))
}

pub fn google_font_is_cached(cache_root: &Path, family: &str) -> bool {
    google_font_cache_file(cache_root, family).is_file()
}

pub fn extract_font_url_from_css(css: &str) -> Option<String> {
    let start = css.find("url(")?;
    let remainder = &css[start + 4..];
    let end = remainder.find(')')?;
    let candidate = remainder[..end].trim().trim_matches('\'').trim_matches('"');

    if candidate.is_empty() {
        return None;
    }

    Some(candidate.to_owned())
}

pub fn discover_local_font_families(extra_dirs: &[PathBuf]) -> Vec<String> {
    let mut database = fontdb::Database::new();
    database.load_system_fonts();

    for directory in extra_dirs {
        if directory.exists() {
            database.load_fonts_dir(directory);
        }
    }

    let mut families = database
        .faces()
        .flat_map(|face| face.families.iter().map(|family| family.0.clone()))
        .collect::<Vec<_>>();
    families.sort();
    families.dedup();
    families
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedUiFont {
    pub family_name: String,
    pub bytes: Vec<u8>,
}

pub fn preferred_ui_font_families(configured_families: &[String]) -> Vec<String> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    for family in configured_families
        .iter()
        .map(|family| family.trim())
        .filter(|family| !family.is_empty())
        .chain(
            [
                "Noto Sans JP",
                "Noto Sans CJK JP",
                "M PLUS Rounded 1c",
                "BIZ UDPGothic",
                "BIZ UDGothic",
                "BIZ UDPMincho",
                "Yu Gothic UI",
                "Yu Gothic",
                "Meiryo UI",
                "Meiryo",
                "MS UI Gothic",
                "MS Gothic",
                "Hiragino Sans",
                "Hiragino Kaku Gothic ProN",
                "Hiragino Kaku Gothic Pro",
                "IPAexGothic",
                "IPAGothic",
                "TakaoGothic",
                "VL PGothic",
                "Source Han Sans JP",
            ]
            .into_iter(),
        )
    {
        let normalized = family.to_owned();
        if seen.insert(normalized.clone()) {
            ordered.push(normalized);
        }
    }

    ordered
}

pub fn load_ui_font_candidates(
    extra_dirs: &[PathBuf],
    configured_families: &[String],
    max_fonts: usize,
) -> Vec<LoadedUiFont> {
    if max_fonts == 0 {
        return Vec::new();
    }

    let mut database = Database::new();
    database.load_system_fonts();

    for directory in extra_dirs {
        if directory.exists() {
            database.load_fonts_dir(directory);
        }
    }

    let mut loaded = Vec::new();
    for family_name in preferred_ui_font_families(configured_families) {
        let query_families = [Family::Name(family_name.as_str())];
        let query = Query {
            families: &query_families,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
        };
        let Some(face_id) = database.query(&query) else {
            continue;
        };
        let Some(bytes) = database.with_face_data(face_id, |data, _| data.to_vec()) else {
            continue;
        };

        loaded.push(LoadedUiFont { family_name, bytes });
        if loaded.len() >= max_fonts {
            break;
        }
    }

    loaded
}

#[derive(Debug, Error)]
pub enum GoogleFontFetchError {
    #[error("Google Fonts family name is empty")]
    EmptyFamily,
    #[error("Google Fonts CSS request failed: {0}")]
    CssRequest(String),
    #[error("Google Fonts CSS did not include a downloadable font URL")]
    CssMissingFontUrl,
    #[error("Google Fonts asset request failed: {0}")]
    AssetRequest(String),
    #[error("Google Fonts cache I/O failed: {0}")]
    Io(#[from] std::io::Error),
}

pub fn fetch_google_font_to_cache(
    cache_root: &Path,
    family: &str,
) -> Result<PathBuf, GoogleFontFetchError> {
    let family = family.trim();
    if family.is_empty() {
        return Err(GoogleFontFetchError::EmptyFamily);
    }

    fs::create_dir_all(cache_root)?;

    let css = ureq::get(&google_fonts_css_url(family))
        .set("User-Agent", "PauseInk/1.0")
        .call()
        .map_err(|error| GoogleFontFetchError::CssRequest(error.to_string()))?
        .into_string()?;
    let font_url =
        extract_font_url_from_css(&css).ok_or(GoogleFontFetchError::CssMissingFontUrl)?;

    let response = ureq::get(&font_url)
        .set("User-Agent", "PauseInk/1.0")
        .call()
        .map_err(|error| GoogleFontFetchError::AssetRequest(error.to_string()))?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes)?;

    let cache_path = google_font_cache_file(cache_root, family);
    fs::write(&cache_path, bytes)?;
    Ok(cache_path)
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

fn slugify_family_name(family: &str) -> String {
    let mut slug = String::with_capacity(family.len());
    let mut last_was_dash = false;

    for ch in family.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    slug.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn css2_url_uses_google_fonts_api_and_swap_display() {
        let url = google_fonts_css_url("Noto Sans JP");

        assert_eq!(
            url,
            "https://fonts.googleapis.com/css2?family=Noto+Sans+JP&display=swap"
        );
    }

    #[test]
    fn cache_file_stays_under_portable_root() {
        let path = google_font_cache_file(
            Path::new("/tmp/pauseink_data/cache/google_fonts"),
            "M PLUS Rounded 1c",
        );

        assert_eq!(
            path,
            Path::new("/tmp/pauseink_data/cache/google_fonts/m-plus-rounded-1c.font")
        );
    }

    #[test]
    fn broken_css_does_not_produce_a_font_url() {
        assert_eq!(extract_font_url_from_css("body { color: red; }"), None);
    }

    #[test]
    fn missing_extra_font_dirs_are_ignored() {
        let families =
            discover_local_font_families(&[PathBuf::from("/tmp/pauseink-this-dir-does-not-exist")]);

        assert!(families.is_sorted());
    }

    #[test]
    fn empty_google_font_family_is_rejected_before_network_access() {
        let error = fetch_google_font_to_cache(Path::new("/tmp"), "   ")
            .expect_err("empty family should be rejected");

        assert!(matches!(error, GoogleFontFetchError::EmptyFamily));
    }

    #[test]
    fn cache_presence_checks_the_expected_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let cache_root = temp_dir.path();
        let cache_file = google_font_cache_file(cache_root, "Noto Sans JP");
        assert!(!google_font_is_cached(cache_root, "Noto Sans JP"));

        std::fs::write(&cache_file, b"font").expect("cache file");

        assert!(google_font_is_cached(cache_root, "Noto Sans JP"));
    }

    #[test]
    fn preferred_ui_font_families_prioritize_configured_entries() {
        let families =
            preferred_ui_font_families(&["My Preferred UI".to_owned(), "Noto Sans JP".to_owned()]);

        assert_eq!(
            families.first().map(String::as_str),
            Some("My Preferred UI")
        );
        assert_eq!(families.get(1).map(String::as_str), Some("Noto Sans JP"));
    }

    #[test]
    fn preferred_ui_font_families_trim_and_deduplicate_names() {
        let families = preferred_ui_font_families(&[
            "  Noto Sans JP  ".to_owned(),
            "".to_owned(),
            "Noto Sans JP".to_owned(),
        ]);

        let noto_count = families
            .iter()
            .filter(|family| family.as_str() == "Noto Sans JP")
            .count();
        assert_eq!(noto_count, 1);
        assert!(families.iter().any(|family| family == "Yu Gothic UI"));
    }
}
