use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result as AnyhowResult;
use pauseink_domain::{
    AnnotationProject, AppendStrokeToGlyphObjectCommand, ClearEvent, ClearEventId, ClearKind,
    ClearOrdering, ClearTargetGranularity, CommandBatch, CommandHistory, DerivedStrokePath,
    GlyphObject, GlyphObjectId, InsertClearEventCommand, InsertGlyphObjectCommand,
    InsertStrokeCommand, MediaTime, OrderingMetadata, Point2, SetGlyphObjectStyleCommand, Stroke,
    StrokeId, StrokeSample, StyleSnapshot, DEFAULT_HISTORY_DEPTH,
};
use pauseink_export::ExportSnapshot;
use pauseink_media::{import_media, ImportedMedia, MediaError, MediaProvider, PlaybackState};
use pauseink_project_io::{
    load_from_str, save_to_string, PauseInkDocument, ProjectClearEvent, ProjectGlyphObject,
    ProjectGroup, ProjectStroke,
};
use serde_json::{Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMode {
    FreeInk,
    GuideCapture,
    TemplatePlacement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuideState {
    pub visible: bool,
    pub slope_degrees: f32,
    pub anchor: Option<Point2>,
    pub reference_object_id: Option<GlyphObjectId>,
}

impl Default for GuideState {
    fn default() -> Self {
        Self {
            visible: false,
            slope_degrees: 0.0,
            anchor: None,
            reference_object_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokePreview {
    pub points: Vec<Point2>,
    pub style: StyleSnapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateState {
    pub text: String,
    pub anchor: Option<Point2>,
    pub active_slot_index: usize,
}

impl Default for TemplateState {
    fn default() -> Self {
        Self {
            text: String::new(),
            anchor: None,
            active_slot_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct StrokeDraft {
    samples: Vec<StrokeSample>,
}

pub struct AppSession {
    pub document: PauseInkDocument,
    pub project: AnnotationProject,
    pub imported_media: Option<ImportedMedia>,
    pub playback: Option<PlaybackState>,
    pub editor_mode: EditorMode,
    pub active_style: StyleSnapshot,
    pub guide: GuideState,
    pub template: TemplateState,
    pub selected_object_id: Option<GlyphObjectId>,
    pub document_path: Option<PathBuf>,
    pub dirty: bool,
    history: CommandHistory<AnnotationProject>,
    stroke_draft: Option<StrokeDraft>,
    last_created_object_id: Option<GlyphObjectId>,
    next_id_counter: u64,
    next_capture_order: u64,
}

impl Default for AppSession {
    fn default() -> Self {
        Self::with_history_limit(DEFAULT_HISTORY_DEPTH)
    }
}

impl AppSession {
    pub fn with_history_limit(history_limit: usize) -> Self {
        let document = PauseInkDocument::default();
        Self {
            project: AnnotationProject::default(),
            imported_media: None,
            playback: None,
            editor_mode: EditorMode::FreeInk,
            active_style: StyleSnapshot::default(),
            guide: GuideState::default(),
            template: TemplateState::default(),
            selected_object_id: None,
            document_path: None,
            dirty: false,
            history: CommandHistory::with_limit(history_limit),
            stroke_draft: None,
            last_created_object_id: None,
            next_id_counter: 1,
            next_capture_order: 1,
            document,
        }
    }

    pub fn load_project_from_str(source: &str) -> AnyhowResult<Self> {
        let document = load_from_str(source)?;
        let project = annotation_project_from_document(&document);
        let next_id_counter = seed_next_id_counter(&project);
        let next_capture_order = seed_next_capture_order(&project);
        Ok(Self {
            document,
            project,
            imported_media: None,
            playback: None,
            editor_mode: EditorMode::FreeInk,
            active_style: StyleSnapshot::default(),
            guide: GuideState::default(),
            template: TemplateState::default(),
            selected_object_id: None,
            document_path: None,
            dirty: false,
            history: CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH),
            stroke_draft: None,
            last_created_object_id: None,
            next_id_counter,
            next_capture_order,
        })
    }

    pub fn load_project_from_path(path: &Path) -> AnyhowResult<Self> {
        let mut session = Self::load_project_from_str(&fs::read_to_string(path)?)?;
        session.document_path = Some(path.to_path_buf());
        Ok(session)
    }

    pub fn save_project_to_string(&mut self) -> AnyhowResult<String> {
        self.synchronize_document_from_project();
        Ok(save_to_string(&self.document)?)
    }

    pub fn save_project_to_path(&mut self, path: &Path) -> AnyhowResult<()> {
        let serialized = self.save_project_to_string()?;
        fs::write(path, serialized)?;
        self.document_path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    pub fn import_media(
        &mut self,
        provider: &dyn MediaProvider,
        source_path: &Path,
    ) -> Result<(), MediaError> {
        let imported = import_media(provider, source_path)?;
        self.playback = Some(PlaybackState::new(imported.clone()));
        self.imported_media = Some(imported.clone());
        self.update_document_media_metadata(source_path, &imported);
        self.dirty = true;
        Ok(())
    }

    pub fn play(&mut self) -> bool {
        let Some(playback) = &mut self.playback else {
            return false;
        };
        playback.play();
        true
    }

    pub fn pause(&mut self) -> bool {
        let Some(playback) = &mut self.playback else {
            return false;
        };
        playback.pause();
        true
    }

    pub fn seek(&mut self, time: MediaTime) -> bool {
        let Some(playback) = &mut self.playback else {
            return false;
        };
        playback.seek(time);
        true
    }

    pub fn current_time(&self) -> MediaTime {
        self.playback
            .as_ref()
            .map(|playback| playback.current_time)
            .unwrap_or_else(|| MediaTime::from_millis(0))
    }

    pub fn media_source_hint(&self) -> Option<PathBuf> {
        self.document
            .project
            .media
            .get("source_path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
    }

    pub fn project_title(&self) -> String {
        self.document
            .project
            .metadata
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("無題のプロジェクト")
            .to_owned()
    }

    pub fn set_project_title(&mut self, title: impl Into<String>) {
        if !self.document.project.metadata.is_object() {
            self.document.project.metadata = Value::Object(Map::new());
        }
        let metadata = self
            .document
            .project
            .metadata
            .as_object_mut()
            .expect("metadata was set to an object above");
        metadata.insert("title".to_owned(), Value::String(title.into()));
        self.dirty = true;
    }

    pub fn begin_stroke(&mut self, point: Point2, at: MediaTime) {
        self.stroke_draft = Some(StrokeDraft {
            samples: vec![StrokeSample {
                position: point,
                at,
                pressure: None,
            }],
        });
    }

    pub fn append_stroke_point(&mut self, point: Point2, at: MediaTime) {
        let draft = self.stroke_draft.get_or_insert_with(|| StrokeDraft {
            samples: Vec::new(),
        });
        if draft.samples.last().is_some_and(|sample| {
            (sample.position.x - point.x).abs() <= f32::EPSILON
                && (sample.position.y - point.y).abs() <= f32::EPSILON
        }) {
            return;
        }
        draft.samples.push(StrokeSample {
            position: point,
            at,
            pressure: None,
        });
    }

    pub fn cancel_stroke(&mut self) {
        self.stroke_draft = None;
    }

    pub fn current_stroke_preview(&self) -> Option<StrokePreview> {
        let draft = self.stroke_draft.as_ref()?;
        let preview_samples = if draft.samples.len() >= 2 {
            stabilize_samples(&draft.samples, self.active_style.stabilization_strength)
        } else {
            draft.samples.clone()
        };
        Some(StrokePreview {
            points: preview_samples
                .into_iter()
                .map(|sample| sample.position)
                .collect(),
            style: self.active_style.clone(),
        })
    }

    pub fn overwrite_glyph_object_style(
        &mut self,
        object_id: &GlyphObjectId,
        style: StyleSnapshot,
    ) -> bool {
        let Some(object) = self
            .project
            .glyph_objects
            .iter_mut()
            .find(|object| object.id == *object_id)
        else {
            return false;
        };

        if object.style == style {
            return false;
        }

        object.style = style;
        self.dirty = true;
        true
    }

    pub fn commit_stroke(&mut self, shift_group: bool) -> AnyhowResult<Option<GlyphObjectId>> {
        let target_object_id = if shift_group {
            self.last_created_object_id.clone()
        } else {
            None
        };
        self.commit_stroke_into_object(target_object_id)
    }

    pub fn commit_stroke_into_object(
        &mut self,
        target_object_id: Option<GlyphObjectId>,
    ) -> AnyhowResult<Option<GlyphObjectId>> {
        let Some(draft) = self.stroke_draft.take() else {
            return Ok(None);
        };
        if draft.samples.len() < 2 {
            return Ok(None);
        }

        let stroke_id = StrokeId::new(self.allocate_id("stroke"));
        let created_at = draft
            .samples
            .first()
            .map(|sample| sample.at)
            .unwrap_or_else(|| self.current_time());
        let stabilized_samples =
            stabilize_samples(&draft.samples, self.active_style.stabilization_strength);
        let stroke = Stroke {
            id: stroke_id.clone(),
            raw_samples: draft.samples.clone(),
            stabilized_samples: stabilized_samples.clone(),
            derived_path: DerivedStrokePath {
                points: stabilized_samples
                    .iter()
                    .map(|sample| sample.position)
                    .collect(),
            },
            style: self.active_style.clone(),
            created_at,
        };

        let selected_object_id = if let Some(object_id) = target_object_id {
            let previous_style = self
                .project
                .glyph_objects
                .iter()
                .find(|object| object.id == object_id)
                .map(|object| object.style.clone())
                .ok_or_else(|| anyhow::anyhow!("target glyph object not found: {}", object_id.0))?;
            self.history.apply(
                &mut self.project,
                Box::new(CommandBatch::new(vec![
                    Box::new(InsertStrokeCommand {
                        stroke,
                        index: None,
                    }),
                    Box::new(AppendStrokeToGlyphObjectCommand {
                        object_id: object_id.clone(),
                        stroke_id: stroke_id.clone(),
                    }),
                    Box::new(SetGlyphObjectStyleCommand {
                        object_id: object_id.clone(),
                        from: previous_style,
                        to: self.active_style.clone(),
                    }),
                ])),
            )?;
            object_id
        } else {
            let object_id = GlyphObjectId::new(self.allocate_id("object"));
            let capture_order = self.allocate_capture_order();
            let object = GlyphObject {
                id: object_id.clone(),
                stroke_ids: vec![stroke_id.clone()],
                style: self.active_style.clone(),
                ordering: OrderingMetadata {
                    z_index: self.project.glyph_objects.len() as i32,
                    capture_order,
                    reveal_order: capture_order,
                },
                created_at,
                ..GlyphObject::default()
            };
            self.history.apply(
                &mut self.project,
                Box::new(CommandBatch::new(vec![
                    Box::new(InsertStrokeCommand {
                        stroke,
                        index: None,
                    }),
                    Box::new(InsertGlyphObjectCommand {
                        object,
                        index: None,
                    }),
                ])),
            )?;
            object_id
        };

        self.selected_object_id = Some(selected_object_id.clone());
        self.last_created_object_id = Some(selected_object_id.clone());
        self.dirty = true;
        Ok(Some(selected_object_id))
    }

    pub fn object_bounds(&self, object_id: &GlyphObjectId) -> Option<(Point2, Point2)> {
        let object = self
            .project
            .glyph_objects
            .iter()
            .find(|object| object.id == *object_id)?;
        let mut points = object
            .stroke_ids
            .iter()
            .filter_map(|stroke_id| {
                self.project
                    .strokes
                    .iter()
                    .find(|stroke| stroke.id == *stroke_id)
            })
            .flat_map(|stroke| {
                if !stroke.derived_path.points.is_empty() {
                    stroke.derived_path.points.clone()
                } else if !stroke.stabilized_samples.is_empty() {
                    stroke
                        .stabilized_samples
                        .iter()
                        .map(|sample| sample.position)
                        .collect()
                } else {
                    stroke
                        .raw_samples
                        .iter()
                        .map(|sample| sample.position)
                        .collect()
                }
            });
        let first = points.next()?;
        let mut min = first;
        let mut max = first;

        for point in points {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }

        Some((min, max))
    }

    pub fn insert_clear_event(&mut self, kind: ClearKind) -> AnyhowResult<ClearEventId> {
        let clear_event = ClearEvent {
            id: ClearEventId::new(self.allocate_id("clear")),
            time: self.current_time(),
            kind,
            granularity: ClearTargetGranularity::AllParallel,
            ordering: ClearOrdering::Parallel,
            ..ClearEvent::default()
        };
        let clear_event_id = clear_event.id.clone();
        self.history.apply(
            &mut self.project,
            Box::new(InsertClearEventCommand {
                clear_event,
                index: None,
            }),
        )?;
        self.dirty = true;
        Ok(clear_event_id)
    }

    pub fn undo(&mut self) -> AnyhowResult<bool> {
        let changed = self.history.undo(&mut self.project)?;
        if changed {
            self.dirty = true;
        }
        Ok(changed)
    }

    pub fn redo(&mut self) -> AnyhowResult<bool> {
        let changed = self.history.redo(&mut self.project)?;
        if changed {
            self.dirty = true;
        }
        Ok(changed)
    }

    pub fn transport_summary(&self) -> String {
        let Some(playback) = &self.playback else {
            return "メディア未読み込み".to_owned();
        };

        format!(
            "{} / 現在位置 {} ticks",
            if playback.is_playing {
                "再生中"
            } else {
                "一時停止"
            },
            playback.current_time.ticks
        )
    }

    pub fn set_history_limit(&mut self, history_limit: usize) {
        self.history = CommandHistory::with_limit(history_limit);
    }

    pub fn build_export_snapshot(&self) -> ExportSnapshot {
        let width = self
            .imported_media
            .as_ref()
            .and_then(|media| media.probe.width)
            .or_else(|| {
                self.document
                    .project
                    .media
                    .get("width")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32)
            })
            .unwrap_or(1280);
        let height = self
            .imported_media
            .as_ref()
            .and_then(|media| media.probe.height)
            .or_else(|| {
                self.document
                    .project
                    .media
                    .get("height")
                    .and_then(Value::as_u64)
                    .map(|value| value as u32)
            })
            .unwrap_or(720);
        let frame_rate = self
            .imported_media
            .as_ref()
            .and_then(|media| media.probe.frame_rate)
            .or_else(|| {
                self.document
                    .project
                    .media
                    .get("fps_hint")
                    .and_then(Value::as_f64)
            })
            .filter(|value| *value > 0.0)
            .unwrap_or(30.0);
        let duration = self
            .imported_media
            .as_ref()
            .and_then(ImportedMedia::duration)
            .unwrap_or_else(|| project_duration_hint(&self.project));

        ExportSnapshot {
            project: self.project.clone(),
            width,
            height,
            frame_rate,
            duration,
            source_media_path: self
                .imported_media
                .as_ref()
                .map(|media| media.source_path.clone())
                .or_else(|| self.media_source_hint()),
            has_audio: self
                .imported_media
                .as_ref()
                .map(|media| media.probe.has_audio)
                .unwrap_or(false),
        }
    }

    fn update_document_media_metadata(&mut self, source_path: &Path, imported: &ImportedMedia) {
        let mut media = self
            .document
            .project
            .media
            .as_object()
            .cloned()
            .unwrap_or_else(Map::new);

        media.insert(
            "source_path".to_owned(),
            Value::String(source_path.display().to_string()),
        );
        if let Some(width) = imported.probe.width {
            media.insert("width".to_owned(), Value::from(width));
        }
        if let Some(height) = imported.probe.height {
            media.insert("height".to_owned(), Value::from(height));
        }
        if let Some(frame_rate) = imported.probe.frame_rate {
            media.insert("fps_hint".to_owned(), Value::from(frame_rate));
        }
        if let Some(duration) = imported.probe.duration_seconds {
            media.insert("duration_seconds".to_owned(), Value::from(duration));
        }
        self.document.project.media = Value::Object(media);
    }

    fn synchronize_document_from_project(&mut self) {
        self.document.project.strokes =
            sync_stroke_wrappers(&self.project.strokes, &self.document.project.strokes);
        self.document.project.objects =
            sync_object_wrappers(&self.project.glyph_objects, &self.document.project.objects);
        self.document.project.groups =
            sync_group_wrappers(&self.project.groups, &self.document.project.groups);
        self.document.project.clear_events = sync_clear_event_wrappers(
            &self.project.clear_events,
            &self.document.project.clear_events,
        );
    }

    fn allocate_id(&mut self, prefix: &str) -> String {
        let id = format!("{prefix}_{:04}", self.next_id_counter);
        self.next_id_counter += 1;
        id
    }

    fn allocate_capture_order(&mut self) -> u64 {
        let order = self.next_capture_order;
        self.next_capture_order += 1;
        order
    }
}

fn annotation_project_from_document(document: &PauseInkDocument) -> AnnotationProject {
    AnnotationProject {
        strokes: document
            .project
            .strokes
            .iter()
            .map(|entry| entry.stroke.clone())
            .collect(),
        glyph_objects: document
            .project
            .objects
            .iter()
            .map(|entry| entry.object.clone())
            .collect(),
        groups: document
            .project
            .groups
            .iter()
            .map(|entry| entry.group.clone())
            .collect(),
        clear_events: document
            .project
            .clear_events
            .iter()
            .map(|entry| entry.clear_event.clone())
            .collect(),
    }
}

fn sync_stroke_wrappers(strokes: &[Stroke], existing: &[ProjectStroke]) -> Vec<ProjectStroke> {
    let extras = existing
        .iter()
        .map(|entry| (entry.stroke.id.0.clone(), entry.extra.clone()))
        .collect::<BTreeMap<_, _>>();
    strokes
        .iter()
        .map(|stroke| ProjectStroke {
            stroke: stroke.clone(),
            extra: extras.get(&stroke.id.0).cloned().unwrap_or_default(),
        })
        .collect()
}

fn sync_object_wrappers(
    objects: &[GlyphObject],
    existing: &[ProjectGlyphObject],
) -> Vec<ProjectGlyphObject> {
    let extras = existing
        .iter()
        .map(|entry| (entry.object.id.0.clone(), entry.extra.clone()))
        .collect::<BTreeMap<_, _>>();
    objects
        .iter()
        .map(|object| ProjectGlyphObject {
            object: object.clone(),
            extra: extras.get(&object.id.0).cloned().unwrap_or_default(),
        })
        .collect()
}

fn sync_group_wrappers(
    groups: &[pauseink_domain::Group],
    existing: &[ProjectGroup],
) -> Vec<ProjectGroup> {
    let extras = existing
        .iter()
        .map(|entry| (entry.group.id.0.clone(), entry.extra.clone()))
        .collect::<BTreeMap<_, _>>();
    groups
        .iter()
        .map(|group| ProjectGroup {
            group: group.clone(),
            extra: extras.get(&group.id.0).cloned().unwrap_or_default(),
        })
        .collect()
}

fn sync_clear_event_wrappers(
    clear_events: &[ClearEvent],
    existing: &[ProjectClearEvent],
) -> Vec<ProjectClearEvent> {
    let extras = existing
        .iter()
        .map(|entry| (entry.clear_event.id.0.clone(), entry.extra.clone()))
        .collect::<BTreeMap<_, _>>();
    clear_events
        .iter()
        .map(|clear_event| ProjectClearEvent {
            clear_event: clear_event.clone(),
            extra: extras.get(&clear_event.id.0).cloned().unwrap_or_default(),
        })
        .collect()
}

fn seed_next_id_counter(project: &AnnotationProject) -> u64 {
    (project.strokes.len()
        + project.glyph_objects.len()
        + project.groups.len()
        + project.clear_events.len()
        + 1) as u64
}

fn seed_next_capture_order(project: &AnnotationProject) -> u64 {
    project
        .glyph_objects
        .iter()
        .map(|object| object.ordering.capture_order)
        .max()
        .unwrap_or(0)
        + 1
}

fn project_duration_hint(project: &AnnotationProject) -> pauseink_domain::MediaDuration {
    let last_tick = project
        .strokes
        .iter()
        .flat_map(|stroke| stroke.raw_samples.iter().map(|sample| sample.at.ticks))
        .chain(project.clear_events.iter().map(|clear| clear.time.ticks))
        .chain(
            project
                .glyph_objects
                .iter()
                .map(|object| object.created_at.ticks),
        )
        .max()
        .unwrap_or(500);
    pauseink_domain::MediaDuration::from_millis((last_tick + 500).max(1_000))
}

fn stabilize_samples(raw_samples: &[StrokeSample], strength: f32) -> Vec<StrokeSample> {
    if raw_samples.len() <= 2 {
        return raw_samples.to_vec();
    }

    let strength = strength.clamp(0.0, 1.0);
    let mut stabilized = Vec::with_capacity(raw_samples.len());
    stabilized.push(raw_samples[0].clone());

    for index in 1..raw_samples.len() {
        let current = &raw_samples[index];
        let previous_filtered = stabilized
            .last()
            .expect("stabilized always keeps previous sample");
        let previous_raw = &raw_samples[index - 1];
        let next_raw = raw_samples.get(index + 1);

        let dt_ms = ((current.at.ticks - previous_raw.at.ticks).abs()).max(1) as f32;
        let speed = point_distance(previous_raw.position, current.position) / dt_ms;
        let corner_guard = next_raw
            .map(|next| corner_guard(previous_raw.position, current.position, next.position))
            .unwrap_or(1.0);
        let smoothing = (strength * 0.82) / (1.0 + speed * 4.0);
        let alpha = (1.0 - smoothing * corner_guard).clamp(0.18, 1.0);

        stabilized.push(StrokeSample {
            position: Point2 {
                x: previous_filtered.position.x
                    + (current.position.x - previous_filtered.position.x) * alpha,
                y: previous_filtered.position.y
                    + (current.position.y - previous_filtered.position.y) * alpha,
            },
            at: current.at,
            pressure: current.pressure,
        });
    }

    stabilized
}

fn point_distance(left: Point2, right: Point2) -> f32 {
    let dx = right.x - left.x;
    let dy = right.y - left.y;
    (dx * dx + dy * dy).sqrt()
}

fn corner_guard(previous: Point2, current: Point2, next: Point2) -> f32 {
    let a = Point2 {
        x: current.x - previous.x,
        y: current.y - previous.y,
    };
    let b = Point2 {
        x: next.x - current.x,
        y: next.y - current.y,
    };
    let a_len = point_distance(Point2::default(), a);
    let b_len = point_distance(Point2::default(), b);
    if a_len <= f32::EPSILON || b_len <= f32::EPSILON {
        return 1.0;
    }

    let dot = (a.x * b.x + a.y * b.y) / (a_len * b_len);
    let angle = dot.clamp(-1.0, 1.0).acos();
    if angle > 0.65 {
        0.25
    } else if angle > 0.35 {
        0.55
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use pauseink_domain::RgbaColor;
    use pauseink_media::{
        MediaProbe, MediaProvider, MediaRuntime, MediaSupport, PreviewFrame, RuntimeCapabilities,
    };
    use tempfile::tempdir;

    use super::*;

    struct MockMediaProvider {
        probe: MediaProbe,
    }

    impl MediaProvider for MockMediaProvider {
        fn probe(&self, _source_path: &Path) -> Result<MediaProbe, MediaError> {
            Ok(self.probe.clone())
        }

        fn capabilities(&self) -> Result<RuntimeCapabilities, MediaError> {
            Ok(RuntimeCapabilities::default())
        }

        fn preview_frame(
            &self,
            _source_path: &Path,
            _time: MediaTime,
            _max_width: u32,
            _max_height: u32,
        ) -> Result<PreviewFrame, MediaError> {
            Ok(PreviewFrame {
                width: 1,
                height: 1,
                rgba_pixels: vec![0, 0, 0, 0],
            })
        }

        fn diagnostics(&self) -> MediaRuntime {
            MediaRuntime::from_system_path()
        }
    }

    #[test]
    fn import_media_initializes_playback_state() {
        let mut session = AppSession::default();
        let provider = MockMediaProvider {
            probe: MediaProbe {
                format_name: Some("mp4".into()),
                duration_seconds: Some(8.0),
                duration_raw: Some("8.000000".into()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                avg_frame_rate_raw: Some("30/1".into()),
                r_frame_rate_raw: Some("30/1".into()),
                pix_fmt: Some("yuv420p".into()),
                has_alpha: false,
                has_audio: true,
                video_codec: Some("h264".into()),
                audio_codec: Some("aac".into()),
                support: MediaSupport::Supported,
            },
        };

        session
            .import_media(&provider, Path::new("sample.mp4"))
            .expect("import should succeed");

        assert_eq!(
            session
                .imported_media
                .as_ref()
                .map(|media| media.source_path.clone()),
            Some(PathBuf::from("sample.mp4"))
        );
        assert_eq!(
            session
                .playback
                .as_ref()
                .map(|playback| playback.current_time),
            Some(MediaTime::from_millis(0))
        );
    }

    #[test]
    fn play_pause_seek_update_transport_summary() {
        let mut session = AppSession {
            playback: Some(PlaybackState::new(ImportedMedia {
                source_path: PathBuf::from("sample.mp4"),
                probe: MediaProbe {
                    format_name: Some("mp4".into()),
                    duration_seconds: Some(5.0),
                    duration_raw: Some("5.000000".into()),
                    width: Some(1280),
                    height: Some(720),
                    frame_rate: Some(30.0),
                    avg_frame_rate_raw: Some("30/1".into()),
                    r_frame_rate_raw: Some("30/1".into()),
                    pix_fmt: Some("yuv420p".into()),
                    has_alpha: false,
                    has_audio: true,
                    video_codec: Some("h264".into()),
                    audio_codec: Some("aac".into()),
                    support: MediaSupport::Supported,
                },
            })),
            ..AppSession::default()
        };

        assert!(session.play());
        assert!(session.transport_summary().contains("再生中"));

        assert!(session.seek(MediaTime::from_millis(2_000)));
        assert!(session.transport_summary().contains("2000"));

        assert!(session.pause());
        assert!(session.transport_summary().contains("一時停止"));
    }

    #[test]
    fn free_ink_commit_creates_stroke_and_glyph_object() {
        let mut session = AppSession::default();

        session.begin_stroke(Point2 { x: 10.0, y: 20.0 }, MediaTime::from_millis(100));
        session.append_stroke_point(Point2 { x: 40.0, y: 30.0 }, MediaTime::from_millis(120));
        let object_id = session
            .commit_stroke(false)
            .expect("stroke commit should succeed")
            .expect("object should be created");

        assert_eq!(session.project.strokes.len(), 1);
        assert_eq!(session.project.glyph_objects.len(), 1);
        assert_eq!(session.project.glyph_objects[0].id, object_id);
        assert_eq!(
            session.project.glyph_objects[0].stroke_ids,
            vec![session.project.strokes[0].id.clone()]
        );
        assert_eq!(session.project.strokes[0].raw_samples.len(), 2);
        assert_eq!(session.project.strokes[0].stabilized_samples.len(), 2);
        assert_eq!(session.project.strokes[0].derived_path.points.len(), 2);
    }

    #[test]
    fn shift_grouping_appends_second_stroke_to_previous_object() {
        let mut session = AppSession::default();

        session.begin_stroke(Point2 { x: 0.0, y: 0.0 }, MediaTime::from_millis(0));
        session.append_stroke_point(Point2 { x: 20.0, y: 20.0 }, MediaTime::from_millis(10));
        let first_object = session
            .commit_stroke(false)
            .expect("first stroke should commit")
            .expect("first object should exist");

        session.begin_stroke(Point2 { x: 30.0, y: 10.0 }, MediaTime::from_millis(20));
        session.append_stroke_point(Point2 { x: 45.0, y: 12.0 }, MediaTime::from_millis(30));
        let grouped_object = session
            .commit_stroke(true)
            .expect("second stroke should commit")
            .expect("grouped object should exist");

        assert_eq!(grouped_object, first_object);
        assert_eq!(session.project.glyph_objects.len(), 1);
        assert_eq!(session.project.glyph_objects[0].stroke_ids.len(), 2);
    }

    #[test]
    fn appending_into_existing_object_updates_object_style_to_latest_active_style() {
        let mut session = AppSession::default();

        session.begin_stroke(Point2 { x: 0.0, y: 0.0 }, MediaTime::from_millis(0));
        session.append_stroke_point(Point2 { x: 20.0, y: 20.0 }, MediaTime::from_millis(10));
        let object_id = session
            .commit_stroke(false)
            .expect("first stroke should commit")
            .expect("first object should exist");

        session.active_style.color = RgbaColor::new(255, 64, 32, 255);
        session.active_style.thickness = 12.0;

        session.begin_stroke(Point2 { x: 30.0, y: 10.0 }, MediaTime::from_millis(20));
        session.append_stroke_point(Point2 { x: 48.0, y: 12.0 }, MediaTime::from_millis(30));
        session
            .commit_stroke_into_object(Some(object_id))
            .expect("second stroke should append");

        let object = &session.project.glyph_objects[0];
        assert_eq!(object.stroke_ids.len(), 2);
        assert_eq!(object.style.color, RgbaColor::new(255, 64, 32, 255));
        assert_eq!(object.style.thickness, 12.0);
    }

    #[test]
    fn clear_event_uses_current_transport_time() {
        let mut session = AppSession {
            playback: Some(PlaybackState::new(ImportedMedia {
                source_path: PathBuf::from("sample.mp4"),
                probe: MediaProbe {
                    format_name: Some("mp4".into()),
                    duration_seconds: Some(5.0),
                    duration_raw: Some("5.000000".into()),
                    width: Some(1280),
                    height: Some(720),
                    frame_rate: Some(30.0),
                    avg_frame_rate_raw: Some("30/1".into()),
                    r_frame_rate_raw: Some("30/1".into()),
                    pix_fmt: Some("yuv420p".into()),
                    has_alpha: false,
                    has_audio: true,
                    video_codec: Some("h264".into()),
                    audio_codec: Some("aac".into()),
                    support: MediaSupport::Supported,
                },
            })),
            ..AppSession::default()
        };
        session.seek(MediaTime::from_millis(2_500));

        session
            .insert_clear_event(ClearKind::Instant)
            .expect("clear event should insert");

        assert_eq!(session.project.clear_events.len(), 1);
        assert_eq!(
            session.project.clear_events[0].time,
            MediaTime::from_millis(2_500)
        );
    }

    #[test]
    fn save_and_reload_preserves_unknown_fields_for_existing_entities() {
        let source = r#"
        {
          format_version: "1.0.0",
          top_unknown: true,
          project: {
            strokes: [
              {
                id: "stroke_0001",
                custom_block: { keep_me: 1 },
              },
            ],
            objects: [
              {
                id: "object_0002",
                stroke_ids: ["stroke_0001"],
                custom_block: { keep_object: 2 },
              },
            ],
            groups: [],
            clear_events: [],
          },
        }
        "#;
        let mut session =
            AppSession::load_project_from_str(source).expect("session load should succeed");
        let saved = session
            .save_project_to_string()
            .expect("session save should succeed");

        assert!(saved.contains("\"top_unknown\": true"));
        assert!(saved.contains("\"keep_me\": 1"));
        assert!(saved.contains("\"keep_object\": 2"));
    }

    #[test]
    fn undo_and_redo_roundtrip_stroke_creation() {
        let mut session = AppSession::default();
        session.begin_stroke(Point2 { x: 0.0, y: 0.0 }, MediaTime::from_millis(0));
        session.append_stroke_point(Point2 { x: 10.0, y: 10.0 }, MediaTime::from_millis(10));
        session.commit_stroke(false).expect("commit should succeed");

        assert_eq!(session.project.strokes.len(), 1);
        assert!(session.undo().expect("undo should succeed"));
        assert!(session.project.strokes.is_empty());
        assert!(session.redo().expect("redo should succeed"));
        assert_eq!(session.project.strokes.len(), 1);
    }

    #[test]
    fn export_snapshot_prefers_imported_media_probe() {
        let session = AppSession {
            imported_media: Some(ImportedMedia {
                source_path: PathBuf::from("sample.mp4"),
                probe: MediaProbe {
                    format_name: Some("mp4".into()),
                    duration_seconds: Some(3.5),
                    duration_raw: Some("3.500000".into()),
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: Some(60.0),
                    avg_frame_rate_raw: Some("60/1".into()),
                    r_frame_rate_raw: Some("60/1".into()),
                    pix_fmt: Some("yuv420p".into()),
                    has_alpha: false,
                    has_audio: true,
                    video_codec: Some("h264".into()),
                    audio_codec: Some("aac".into()),
                    support: MediaSupport::Supported,
                },
            }),
            ..AppSession::default()
        };

        let snapshot = session.build_export_snapshot();

        assert_eq!(snapshot.width, 1920);
        assert_eq!(snapshot.height, 1080);
        assert_eq!(snapshot.frame_rate, 60.0);
        assert_eq!(
            snapshot.duration,
            pauseink_domain::MediaDuration::from_millis(3_500)
        );
        assert_eq!(
            snapshot.source_media_path,
            Some(PathBuf::from("sample.mp4"))
        );
        assert!(snapshot.has_audio);
    }

    #[test]
    fn export_snapshot_falls_back_to_project_timeline_when_media_is_missing() {
        let mut session = AppSession::default();
        session.project.clear_events.push(ClearEvent {
            time: MediaTime::from_millis(2_000),
            ..ClearEvent::default()
        });
        session.project.strokes.push(Stroke {
            id: StrokeId::new("stroke-1"),
            raw_samples: vec![
                StrokeSample {
                    position: Point2 { x: 10.0, y: 10.0 },
                    at: MediaTime::from_millis(1_200),
                    pressure: None,
                },
                StrokeSample {
                    position: Point2 { x: 20.0, y: 30.0 },
                    at: MediaTime::from_millis(1_800),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(1_200),
            ..Stroke::default()
        });

        let snapshot = session.build_export_snapshot();

        assert_eq!(snapshot.width, 1280);
        assert_eq!(snapshot.height, 720);
        assert_eq!(snapshot.frame_rate, 30.0);
        assert_eq!(
            snapshot.duration,
            pauseink_domain::MediaDuration::from_millis(2_500)
        );
        assert_eq!(snapshot.source_media_path, None);
        assert!(!snapshot.has_audio);
    }

    #[test]
    fn create_save_reopen_compare_smoke() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("create-save-reopen.pauseink");
        let mut session = AppSession::default();
        session.set_project_title("保存再読込スモーク");
        session.begin_stroke(Point2 { x: 16.0, y: 24.0 }, MediaTime::from_millis(100));
        session.append_stroke_point(Point2 { x: 42.0, y: 48.0 }, MediaTime::from_millis(120));
        session
            .commit_stroke(false)
            .expect("stroke commit should succeed");

        let expected_project = session.project.clone();
        let expected_title = session.project_title();

        session
            .save_project_to_path(&path)
            .expect("project save should succeed");

        let reopened = AppSession::load_project_from_path(&path).expect("project reload");

        assert_eq!(reopened.project, expected_project);
        assert_eq!(reopened.project_title(), expected_title);
        assert_eq!(reopened.document_path.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn current_stroke_preview_uses_active_style_while_drawing() {
        let mut session = AppSession::default();
        session.active_style.thickness = 9.0;
        session.begin_stroke(Point2 { x: 10.0, y: 20.0 }, MediaTime::from_millis(0));
        session.append_stroke_point(Point2 { x: 40.0, y: 50.0 }, MediaTime::from_millis(10));

        let preview = session.current_stroke_preview().expect("preview");

        assert_eq!(preview.style.thickness, 9.0);
        assert_eq!(preview.points.len(), 2);
        assert_eq!(preview.points[0].x, 10.0);
    }

    #[test]
    fn current_stroke_preview_is_cleared_after_commit_or_cancel() {
        let mut session = AppSession::default();
        session.begin_stroke(Point2 { x: 10.0, y: 20.0 }, MediaTime::from_millis(0));
        session.append_stroke_point(Point2 { x: 40.0, y: 50.0 }, MediaTime::from_millis(10));
        assert!(session.current_stroke_preview().is_some());

        session.cancel_stroke();
        assert!(session.current_stroke_preview().is_none());

        session.begin_stroke(Point2 { x: 10.0, y: 20.0 }, MediaTime::from_millis(0));
        session.append_stroke_point(Point2 { x: 40.0, y: 50.0 }, MediaTime::from_millis(10));
        session
            .commit_stroke_into_object(None)
            .expect("commit should succeed");
        assert!(session.current_stroke_preview().is_none());
    }

    #[test]
    fn import_annotate_clear_save_smoke() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("import-annotate-clear.pauseink");
        let provider = MockMediaProvider {
            probe: MediaProbe {
                format_name: Some("mp4".into()),
                duration_seconds: Some(8.0),
                duration_raw: Some("8.000000".into()),
                width: Some(1280),
                height: Some(720),
                frame_rate: Some(30.0),
                avg_frame_rate_raw: Some("30/1".into()),
                r_frame_rate_raw: Some("30/1".into()),
                pix_fmt: Some("yuv420p".into()),
                has_alpha: false,
                has_audio: true,
                video_codec: Some("h264".into()),
                audio_codec: Some("aac".into()),
                support: MediaSupport::Supported,
            },
        };
        let mut session = AppSession::default();
        session
            .import_media(&provider, Path::new("sample.mp4"))
            .expect("media import should succeed");
        session.seek(MediaTime::from_millis(1_200));
        session.begin_stroke(Point2 { x: 20.0, y: 30.0 }, MediaTime::from_millis(1_050));
        session.append_stroke_point(Point2 { x: 70.0, y: 88.0 }, MediaTime::from_millis(1_100));
        session
            .commit_stroke(false)
            .expect("stroke commit should succeed");
        session
            .insert_clear_event(ClearKind::Instant)
            .expect("clear event should insert");

        session
            .save_project_to_path(&path)
            .expect("project save should succeed");

        let reopened = AppSession::load_project_from_path(&path).expect("project reload");

        assert_eq!(
            reopened.media_source_hint(),
            Some(PathBuf::from("sample.mp4"))
        );
        assert_eq!(reopened.project.strokes.len(), 1);
        assert_eq!(reopened.project.glyph_objects.len(), 1);
        assert_eq!(reopened.project.clear_events.len(), 1);
        assert_eq!(
            reopened.project.clear_events[0].time,
            MediaTime::from_millis(1_200)
        );
        assert_eq!(
            reopened
                .document
                .project
                .media
                .get("width")
                .and_then(Value::as_u64),
            Some(1280)
        );
        assert_eq!(
            reopened
                .document
                .project
                .media
                .get("duration_seconds")
                .and_then(Value::as_f64),
            Some(8.0)
        );
    }
}
