use std::path::{Path, PathBuf};

pub fn google_fonts_css_url(family: &str) -> String {
    format!(
        "https://fonts.googleapis.com/css2?family={}&display=swap",
        encode_query_component(family)
    )
}

pub fn google_font_cache_file(cache_root: &Path, family: &str) -> PathBuf {
    cache_root.join(format!("{}.font", slugify_family_name(family)))
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
}
