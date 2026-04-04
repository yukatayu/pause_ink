use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

pub type ExtraFields = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PauseInkDocument {
    pub format_version: String,
    #[serde(default)]
    pub project: PauseInkProject,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

impl Default for PauseInkDocument {
    fn default() -> Self {
        Self {
            format_version: "1.0.0".to_owned(),
            project: PauseInkProject::default(),
            extra: ExtraFields::default(),
        }
    }
}

impl PauseInkDocument {
    pub fn to_canonical_json(&self) -> Value {
        let mut object = Map::new();
        object.insert(
            "format_version".to_owned(),
            Value::String(canonicalize_format_version(&self.format_version)),
        );
        object.insert("project".to_owned(), self.project.to_canonical_json());

        for (key, value) in &self.extra {
            object.insert(key.clone(), canonicalize_value(value));
        }

        Value::Object(object)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PauseInkProject {
    #[serde(default = "empty_object")]
    pub metadata: Value,
    #[serde(default = "empty_object")]
    pub media: Value,
    #[serde(default = "empty_object")]
    pub settings: Value,
    #[serde(default)]
    pub pages: Vec<Value>,
    #[serde(default)]
    pub objects: Vec<Value>,
    #[serde(default)]
    pub groups: Vec<Value>,
    #[serde(default)]
    pub clear_events: Vec<Value>,
    #[serde(default = "empty_object")]
    pub presets: Value,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

impl Default for PauseInkProject {
    fn default() -> Self {
        Self {
            metadata: empty_object(),
            media: empty_object(),
            settings: empty_object(),
            pages: Vec::new(),
            objects: Vec::new(),
            groups: Vec::new(),
            clear_events: Vec::new(),
            presets: empty_object(),
            extra: ExtraFields::default(),
        }
    }
}

impl PauseInkProject {
    pub fn to_canonical_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("metadata".to_owned(), canonicalize_value(&self.metadata));
        object.insert("media".to_owned(), canonicalize_value(&self.media));
        object.insert("settings".to_owned(), canonicalize_value(&self.settings));
        object.insert(
            "pages".to_owned(),
            Value::Array(self.pages.iter().map(canonicalize_value).collect()),
        );
        object.insert(
            "objects".to_owned(),
            Value::Array(self.objects.iter().map(canonicalize_value).collect()),
        );
        object.insert(
            "groups".to_owned(),
            Value::Array(self.groups.iter().map(canonicalize_value).collect()),
        );
        object.insert(
            "clear_events".to_owned(),
            Value::Array(self.clear_events.iter().map(canonicalize_value).collect()),
        );
        object.insert("presets".to_owned(), canonicalize_value(&self.presets));

        for (key, value) in &self.extra {
            object.insert(key.clone(), canonicalize_value(value));
        }

        Value::Object(object)
    }
}

#[derive(Debug, Error)]
pub enum ProjectIoError {
    #[error("project parse error: {0}")]
    Parse(#[from] json5::Error),
    #[error("project serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub fn load_from_str(source: &str) -> Result<PauseInkDocument, ProjectIoError> {
    let mut document: PauseInkDocument = json5::from_str(source)?;
    document.format_version = canonicalize_format_version(&document.format_version);
    Ok(document)
}

pub fn save_to_string(document: &PauseInkDocument) -> Result<String, ProjectIoError> {
    Ok(serde_json::to_string_pretty(&document.to_canonical_json())?)
}

pub fn canonicalize_format_version(raw: &str) -> String {
    raw.trim().to_string()
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

fn canonicalize_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_value).collect()),
        Value::Object(map) => {
            let mut canonical = Map::new();
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            for key in keys {
                canonical.insert(key.clone(), canonicalize_value(&map[&key]));
            }
            Value::Object(canonical)
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn load_accepts_comments_trailing_commas_and_preserves_unknown_fields() {
        let source = r#"
        // lenient load should accept comments
        {
          format_version: " 1.0.0 ",
          project: {
            metadata: {
              title: "demo",
            },
            media: {
              source_path: "sample.mp4",
            },
            settings: {},
            objects: [],
            groups: [],
            clear_events: [],
            presets: {},
            custom_block: {
              keep_me: true,
            },
          },
          top_unknown: 42,
        }
        "#;

        let document = load_from_str(source).expect("lenient project load should succeed");

        assert_eq!(document.format_version, "1.0.0");
        assert_eq!(document.extra.get("top_unknown"), Some(&json!(42)));
        assert_eq!(
            document.project.extra.get("custom_block"),
            Some(&json!({ "keep_me": true }))
        );
    }

    #[test]
    fn canonical_save_is_deterministic_and_human_readable() {
        let document = PauseInkDocument {
            format_version: "1.0.0".into(),
            project: PauseInkProject {
                metadata: json!({ "title": "demo" }),
                media: json!({ "source_path": "sample.mp4" }),
                settings: json!({}),
                objects: vec![],
                groups: vec![],
                clear_events: vec![],
                presets: json!({}),
                extra: Default::default(),
                ..PauseInkProject::default()
            },
            extra: Default::default(),
        };

        let saved = save_to_string(&document).expect("canonical save should succeed");

        assert_eq!(
            saved,
            "{\n  \"format_version\": \"1.0.0\",\n  \"project\": {\n    \"metadata\": {\n      \"title\": \"demo\"\n    },\n    \"media\": {\n      \"source_path\": \"sample.mp4\"\n    },\n    \"settings\": {},\n    \"pages\": [],\n    \"objects\": [],\n    \"groups\": [],\n    \"clear_events\": [],\n    \"presets\": {}\n  }\n}"
        );
    }
}
