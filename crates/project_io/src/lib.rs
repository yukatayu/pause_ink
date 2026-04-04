use std::collections::BTreeMap;

use pauseink_domain::{AnnotationProject, ClearEvent, GlyphObject, Group, Stroke};
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
    pub strokes: Vec<ProjectStroke>,
    #[serde(default)]
    pub objects: Vec<ProjectGlyphObject>,
    #[serde(default)]
    pub groups: Vec<ProjectGroup>,
    #[serde(default)]
    pub clear_events: Vec<ProjectClearEvent>,
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
            strokes: Vec::new(),
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
            "strokes".to_owned(),
            Value::Array(self.strokes.iter().map(canonicalize_serializable).collect()),
        );
        object.insert(
            "objects".to_owned(),
            Value::Array(self.objects.iter().map(canonicalize_serializable).collect()),
        );
        object.insert(
            "groups".to_owned(),
            Value::Array(self.groups.iter().map(canonicalize_serializable).collect()),
        );
        object.insert(
            "clear_events".to_owned(),
            Value::Array(
                self.clear_events
                    .iter()
                    .map(canonicalize_serializable)
                    .collect(),
            ),
        );
        object.insert("presets".to_owned(), canonicalize_value(&self.presets));

        for (key, value) in &self.extra {
            object.insert(key.clone(), canonicalize_value(value));
        }

        Value::Object(object)
    }

    pub fn to_annotation_project(&self) -> AnnotationProject {
        AnnotationProject {
            strokes: self
                .strokes
                .iter()
                .map(|entry| entry.stroke.clone())
                .collect(),
            glyph_objects: self
                .objects
                .iter()
                .map(|entry| entry.object.clone())
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|entry| entry.group.clone())
                .collect(),
            clear_events: self
                .clear_events
                .iter()
                .map(|entry| entry.clear_event.clone())
                .collect(),
        }
    }

    pub fn sync_annotation_project(&mut self, annotations: &AnnotationProject) {
        let stroke_extra = self
            .strokes
            .iter()
            .map(|entry| (entry.stroke.id.0.clone(), entry.extra.clone()))
            .collect::<BTreeMap<_, _>>();
        let object_extra = self
            .objects
            .iter()
            .map(|entry| (entry.object.id.0.clone(), entry.extra.clone()))
            .collect::<BTreeMap<_, _>>();
        let group_extra = self
            .groups
            .iter()
            .map(|entry| (entry.group.id.0.clone(), entry.extra.clone()))
            .collect::<BTreeMap<_, _>>();
        let clear_event_extra = self
            .clear_events
            .iter()
            .map(|entry| (entry.clear_event.id.0.clone(), entry.extra.clone()))
            .collect::<BTreeMap<_, _>>();

        self.strokes = annotations
            .strokes
            .iter()
            .cloned()
            .map(|stroke| ProjectStroke {
                extra: stroke_extra.get(&stroke.id.0).cloned().unwrap_or_default(),
                stroke,
            })
            .collect();
        self.objects = annotations
            .glyph_objects
            .iter()
            .cloned()
            .map(|object| ProjectGlyphObject {
                extra: object_extra.get(&object.id.0).cloned().unwrap_or_default(),
                object,
            })
            .collect();
        self.groups = annotations
            .groups
            .iter()
            .cloned()
            .map(|group| ProjectGroup {
                extra: group_extra.get(&group.id.0).cloned().unwrap_or_default(),
                group,
            })
            .collect();
        self.clear_events = annotations
            .clear_events
            .iter()
            .cloned()
            .map(|clear_event| ProjectClearEvent {
                extra: clear_event_extra
                    .get(&clear_event.id.0)
                    .cloned()
                    .unwrap_or_default(),
                clear_event,
            })
            .collect();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectStroke {
    #[serde(flatten, default)]
    pub stroke: Stroke,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectGlyphObject {
    #[serde(flatten, default)]
    pub object: GlyphObject,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectGroup {
    #[serde(flatten, default)]
    pub group: Group,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProjectClearEvent {
    #[serde(flatten, default)]
    pub clear_event: ClearEvent,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
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

fn canonicalize_serializable<T: Serialize>(value: &T) -> Value {
    canonicalize_value(
        &serde_json::to_value(value).expect("project value should serialize canonically"),
    )
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
            strokes: [
              {
                id: "stroke-1",
                custom_block: {
                  keep_me_too: true,
                },
              },
            ],
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
        assert_eq!(
            document.project.strokes[0].extra.get("custom_block"),
            Some(&json!({ "keep_me_too": true }))
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
                strokes: vec![],
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
            "{\n  \"format_version\": \"1.0.0\",\n  \"project\": {\n    \"metadata\": {\n      \"title\": \"demo\"\n    },\n    \"media\": {\n      \"source_path\": \"sample.mp4\"\n    },\n    \"settings\": {},\n    \"pages\": [],\n    \"strokes\": [],\n    \"objects\": [],\n    \"groups\": [],\n    \"clear_events\": [],\n    \"presets\": {}\n  }\n}"
        );
    }

    #[test]
    fn typed_entities_roundtrip_through_canonical_save() {
        let document = PauseInkDocument {
            format_version: "1.0.0".into(),
            project: PauseInkProject {
                strokes: vec![ProjectStroke {
                    stroke: Stroke {
                        id: pauseink_domain::StrokeId::new("stroke-1"),
                        ..Stroke::default()
                    },
                    extra: Default::default(),
                }],
                objects: vec![ProjectGlyphObject {
                    object: GlyphObject {
                        id: pauseink_domain::GlyphObjectId::new("object-1"),
                        stroke_ids: vec![pauseink_domain::StrokeId::new("stroke-1")],
                        ..GlyphObject::default()
                    },
                    extra: Default::default(),
                }],
                clear_events: vec![ProjectClearEvent {
                    clear_event: ClearEvent {
                        id: pauseink_domain::ClearEventId::new("clear-1"),
                        ..ClearEvent::default()
                    },
                    extra: Default::default(),
                }],
                ..PauseInkProject::default()
            },
            extra: Default::default(),
        };

        let saved = save_to_string(&document).expect("save should succeed");
        let loaded = load_from_str(&saved).expect("load should succeed");

        assert_eq!(loaded.project.strokes[0].stroke.id.0, "stroke-1");
        assert_eq!(loaded.project.objects[0].object.id.0, "object-1");
        assert_eq!(loaded.project.clear_events[0].clear_event.id.0, "clear-1");
    }

    #[test]
    fn repository_minimal_sample_loads_and_roundtrips() {
        let source = include_str!("../../../samples/minimal_project.pauseink");

        let document = load_from_str(source).expect("repository sample should load");
        assert_eq!(document.project.strokes.len(), 1);
        assert_eq!(document.project.objects.len(), 1);
        assert_eq!(document.project.clear_events.len(), 1);
        assert_eq!(document.project.strokes[0].stroke.id.0, "stroke_0001");
        assert_eq!(
            document.project.objects[0].object.stroke_ids,
            vec![pauseink_domain::StrokeId::new("stroke_0001")]
        );
        assert_eq!(
            document.project.clear_events[0].clear_event.time,
            pauseink_domain::MediaTime::from_millis(4_000)
        );

        let saved = save_to_string(&document).expect("repository sample should save");
        let reloaded = load_from_str(&saved).expect("saved sample should reload");

        assert_eq!(reloaded, document);
    }

    #[test]
    fn sync_annotation_project_preserves_known_entity_extra_fields() {
        let mut project = PauseInkProject {
            strokes: vec![ProjectStroke {
                stroke: Stroke {
                    id: pauseink_domain::StrokeId::new("stroke-1"),
                    ..Stroke::default()
                },
                extra: BTreeMap::from([("keep_me".into(), json!(true))]),
            }],
            objects: vec![ProjectGlyphObject {
                object: GlyphObject {
                    id: pauseink_domain::GlyphObjectId::new("object-1"),
                    ..GlyphObject::default()
                },
                extra: BTreeMap::from([("custom".into(), json!("value"))]),
            }],
            ..PauseInkProject::default()
        };
        let annotations = AnnotationProject {
            strokes: vec![
                Stroke {
                    id: pauseink_domain::StrokeId::new("stroke-1"),
                    created_at: pauseink_domain::MediaTime::from_millis(123),
                    ..Stroke::default()
                },
                Stroke {
                    id: pauseink_domain::StrokeId::new("stroke-2"),
                    ..Stroke::default()
                },
            ],
            glyph_objects: vec![GlyphObject {
                id: pauseink_domain::GlyphObjectId::new("object-1"),
                stroke_ids: vec![pauseink_domain::StrokeId::new("stroke-1")],
                ..GlyphObject::default()
            }],
            ..AnnotationProject::default()
        };

        project.sync_annotation_project(&annotations);

        assert_eq!(project.strokes.len(), 2);
        assert_eq!(project.strokes[0].extra.get("keep_me"), Some(&json!(true)));
        assert!(project.strokes[1].extra.is_empty());
        assert_eq!(
            project.objects[0].extra.get("custom"),
            Some(&json!("value"))
        );
        assert_eq!(project.to_annotation_project(), annotations);
    }
}
