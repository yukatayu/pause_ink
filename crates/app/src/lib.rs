use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result as AnyhowResult;
use pauseink_domain::{
    AnnotationProject, AppendStrokeToGlyphObjectCommand, BatchSetGlyphObjectEntranceCommand,
    BatchSetGlyphObjectPostActionsCommand, BatchSetGlyphObjectStyleCommand, ClearEvent,
    ClearEventId, ClearKind, ClearOrdering, ClearTargetGranularity, CommandBatch, CommandHistory,
    DerivedStrokePath, EntranceBehavior, GlyphObject, GlyphObjectEntranceChange, GlyphObjectId,
    GlyphObjectPostActionsChange, GlyphObjectStyleChange, GlyphObjectZIndexChange, Group, GroupId,
    InsertClearEventCommand, InsertGlyphObjectCommand, InsertGroupCommand, InsertStrokeCommand,
    MediaTime, NormalizeZOrderCommand, OrderingMetadata, Point2, PostAction, RemoveGroupCommand,
    SetGlyphObjectEntranceCommand, SetGlyphObjectPostActionsCommand, SetGlyphObjectStyleCommand,
    Stroke, StrokeId, StrokeSample, StyleSnapshot, UpdateGroupMembershipCommand,
    DEFAULT_HISTORY_DEPTH,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionState {
    pub selected_object_ids: Vec<GlyphObjectId>,
    pub selected_group_ids: Vec<GroupId>,
    pub focused_object_id: Option<GlyphObjectId>,
    pub focused_group_id: Option<GroupId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct GroupSelectionContext {
    group_ids: Vec<GroupId>,
    object_ids: Vec<GlyphObjectId>,
}

#[derive(Debug, Clone, PartialEq)]
enum AutoGroupContext {
    Pending {
        object_id: GlyphObjectId,
        page_index: usize,
        style: StyleSnapshot,
        entrance: EntranceBehavior,
    },
    Active {
        group_id: GroupId,
        page_index: usize,
        style: StyleSnapshot,
        entrance: EntranceBehavior,
    },
}

struct AutoGroupPlan {
    commands: Vec<Box<dyn pauseink_domain::Command<AnnotationProject>>>,
    next_context: Option<AutoGroupContext>,
}

pub struct AppSession {
    pub document: PauseInkDocument,
    pub project: AnnotationProject,
    pub imported_media: Option<ImportedMedia>,
    pub playback: Option<PlaybackState>,
    pub editor_mode: EditorMode,
    pub active_style: StyleSnapshot,
    pub active_entrance: EntranceBehavior,
    pub active_post_actions: Vec<PostAction>,
    pub guide: GuideState,
    pub template: TemplateState,
    pub selection: SelectionState,
    pub document_path: Option<PathBuf>,
    pub dirty: bool,
    history: CommandHistory<AnnotationProject>,
    stroke_draft: Option<StrokeDraft>,
    last_created_object_id: Option<GlyphObjectId>,
    last_group_selection_context: Option<GroupSelectionContext>,
    auto_group_context: Option<AutoGroupContext>,
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
        let mut active_entrance = EntranceBehavior::default();
        active_entrance.duration = pauseink_domain::MediaDuration::from_millis(600);
        Self {
            project: AnnotationProject::default(),
            imported_media: None,
            playback: None,
            editor_mode: EditorMode::FreeInk,
            active_style: StyleSnapshot::default(),
            active_entrance,
            active_post_actions: Vec::new(),
            guide: GuideState::default(),
            template: TemplateState::default(),
            selection: SelectionState::default(),
            document_path: None,
            dirty: false,
            history: CommandHistory::with_limit(history_limit),
            stroke_draft: None,
            last_created_object_id: None,
            last_group_selection_context: None,
            auto_group_context: None,
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
        let mut active_entrance = EntranceBehavior::default();
        active_entrance.duration = pauseink_domain::MediaDuration::from_millis(600);
        Ok(Self {
            document,
            project,
            imported_media: None,
            playback: None,
            editor_mode: EditorMode::FreeInk,
            active_style: StyleSnapshot::default(),
            active_entrance,
            active_post_actions: Vec::new(),
            guide: GuideState::default(),
            template: TemplateState::default(),
            selection: SelectionState::default(),
            document_path: None,
            dirty: false,
            history: CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH),
            stroke_draft: None,
            last_created_object_id: None,
            last_group_selection_context: None,
            auto_group_context: None,
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

    pub fn resolved_media_source_hint(&self) -> Option<PathBuf> {
        self.media_source_hint()
            .map(|path| self.resolve_media_source_path(&path))
    }

    pub fn restore_media_from_hint(
        &mut self,
        provider: &dyn MediaProvider,
    ) -> Result<bool, MediaError> {
        let Some(source_path) = self.resolved_media_source_hint() else {
            return Ok(false);
        };
        let imported = import_media(provider, &source_path)?;
        self.playback = Some(PlaybackState::new(imported.clone()));
        self.imported_media = Some(imported);
        Ok(true)
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

    pub fn selected_object_ids(&self) -> Vec<GlyphObjectId> {
        self.selection.selected_object_ids.clone()
    }

    pub fn selected_group_ids(&self) -> Vec<GroupId> {
        self.selection.selected_group_ids.clone()
    }

    pub fn focused_object_id(&self) -> Option<GlyphObjectId> {
        self.selection.focused_object_id.clone()
    }

    pub fn is_object_selected(&self, object_id: &GlyphObjectId) -> bool {
        self.selection.selected_object_ids.contains(object_id)
    }

    pub fn is_group_selected(&self, group_id: &GroupId) -> bool {
        self.selection.selected_group_ids.contains(group_id)
    }

    pub fn replace_object_selection(&mut self, object_ids: Vec<GlyphObjectId>) {
        self.selection.selected_object_ids = dedupe_object_ids(
            object_ids
                .into_iter()
                .filter(|object_id| self.project.glyph_object_index(object_id).is_some()),
        );
        self.selection.selected_group_ids.clear();
        self.selection.focused_group_id = None;
        self.selection.focused_object_id = self.selection.selected_object_ids.last().cloned();
    }

    pub fn replace_group_selection(&mut self, group_ids: Vec<GroupId>) {
        self.selection.selected_group_ids = dedupe_group_ids(
            group_ids
                .into_iter()
                .filter(|group_id| self.project.group_index(group_id).is_some()),
        );
        self.selection.selected_object_ids.clear();
        self.selection.focused_group_id = self.selection.selected_group_ids.last().cloned();
        self.selection.focused_object_id = self
            .selection
            .focused_group_id
            .as_ref()
            .and_then(|group_id| self.group_member_object_ids(group_id).into_iter().next());
    }

    pub fn toggle_object_selection(&mut self, object_id: GlyphObjectId) {
        if self.project.glyph_object_index(&object_id).is_none() {
            return;
        }
        toggle_vec_membership(&mut self.selection.selected_object_ids, object_id.clone());
        self.selection.selected_group_ids.clear();
        self.selection.focused_group_id = None;
        self.selection.focused_object_id = if self.selection.selected_object_ids.is_empty() {
            None
        } else {
            Some(object_id)
        };
    }

    pub fn toggle_group_selection(&mut self, group_id: GroupId) {
        if self.project.group_index(&group_id).is_none() {
            return;
        }
        toggle_vec_membership(&mut self.selection.selected_group_ids, group_id.clone());
        self.selection.selected_object_ids.clear();
        self.selection.focused_group_id = if self.selection.selected_group_ids.is_empty() {
            None
        } else {
            Some(group_id.clone())
        };
        self.selection.focused_object_id = if self.selection.selected_group_ids.is_empty() {
            None
        } else {
            self.group_member_object_ids(&group_id).into_iter().next()
        };
    }

    pub fn clear_selection(&mut self) {
        self.selection = SelectionState::default();
    }

    pub fn note_auto_group_break(&mut self) {
        self.auto_group_context = None;
    }

    pub fn selected_groupable_object_count(&self) -> usize {
        self.selected_target_object_ids().len()
    }

    pub fn apply_active_style_to_selection(&mut self) -> AnyhowResult<bool> {
        self.apply_style_to_object_ids(
            &self.selected_target_object_ids(),
            self.active_style.clone(),
        )
    }

    pub fn apply_active_entrance_to_selection(&mut self) -> AnyhowResult<bool> {
        self.apply_entrance_to_object_ids(
            &self.selected_target_object_ids(),
            self.active_entrance.clone(),
        )
    }

    pub fn apply_active_post_actions_to_selection(&mut self) -> AnyhowResult<bool> {
        self.apply_post_actions_to_object_ids(
            &self.selected_target_object_ids(),
            self.active_post_actions.clone(),
        )
    }

    pub fn overwrite_glyph_object_style(
        &mut self,
        object_id: &GlyphObjectId,
        style: StyleSnapshot,
    ) -> AnyhowResult<bool> {
        self.apply_style_to_object_ids(std::slice::from_ref(object_id), style)
    }

    pub fn overwrite_glyph_object_entrance(
        &mut self,
        object_id: &GlyphObjectId,
        entrance: EntranceBehavior,
    ) -> AnyhowResult<bool> {
        self.apply_entrance_to_object_ids(std::slice::from_ref(object_id), entrance)
    }

    pub fn overwrite_glyph_object_post_actions(
        &mut self,
        object_id: &GlyphObjectId,
        post_actions: Vec<PostAction>,
    ) -> AnyhowResult<bool> {
        self.apply_post_actions_to_object_ids(std::slice::from_ref(object_id), post_actions)
    }

    pub fn group_selected_objects(&mut self) -> AnyhowResult<Option<GroupId>> {
        let object_ids = self.selected_target_object_ids();
        if object_ids.len() < 2 {
            return Ok(None);
        }
        let page_indices = object_ids
            .iter()
            .filter_map(|object_id| {
                self.project
                    .glyph_objects
                    .iter()
                    .find(|object| object.id == *object_id)
                    .map(|object| object.page_index(&self.project.clear_events))
            })
            .collect::<HashSet<_>>();
        if page_indices.len() > 1 {
            anyhow::bail!("異なる page の object は同じ group にできません。");
        }

        let existing_group_ids = dedupe_group_ids(
            object_ids
                .iter()
                .filter_map(|object_id| self.group_for_object_id(object_id)),
        );
        let ordered_group_ids = self
            .project
            .groups
            .iter()
            .filter(|group| existing_group_ids.contains(&group.id))
            .map(|group| group.id.clone())
            .collect::<Vec<_>>();

        let group_id = if let Some(anchor_group_id) = ordered_group_ids.first().cloned() {
            let anchor_group = self
                .project
                .groups
                .iter()
                .find(|group| group.id == anchor_group_id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("anchor group not found: {}", anchor_group_id.0))?;
            let mut commands: Vec<Box<dyn pauseink_domain::Command<AnnotationProject>>> =
                Vec::new();
            if anchor_group.glyph_object_ids != object_ids {
                commands.push(Box::new(UpdateGroupMembershipCommand {
                    group_id: anchor_group_id.clone(),
                    from_glyph_object_ids: anchor_group.glyph_object_ids.clone(),
                    to_glyph_object_ids: object_ids.clone(),
                    from_loose_stroke_ids: anchor_group.loose_stroke_ids.clone(),
                    to_loose_stroke_ids: anchor_group.loose_stroke_ids.clone(),
                }));
            }

            for group in self.project.groups.iter().rev() {
                if group.id != anchor_group_id && ordered_group_ids.contains(&group.id) {
                    let index = self
                        .project
                        .group_index(&group.id)
                        .expect("existing group should have index");
                    commands.push(Box::new(RemoveGroupCommand {
                        group: group.clone(),
                        index,
                    }));
                }
            }

            if commands.is_empty() {
                return Ok(None);
            }
            self.history
                .apply(&mut self.project, Box::new(CommandBatch::new(commands)))?;
            anchor_group_id
        } else {
            let created_at = object_ids
                .iter()
                .filter_map(|object_id| {
                    self.project
                        .glyph_objects
                        .iter()
                        .find(|object| object.id == *object_id)
                        .map(|object| object.created_at)
                })
                .min()
                .unwrap_or_else(|| self.current_time());
            let group = Group {
                id: GroupId::new(self.allocate_id("group")),
                glyph_object_ids: object_ids.clone(),
                created_at,
                ..Group::default()
            };
            let group_id = group.id.clone();
            self.history.apply(
                &mut self.project,
                Box::new(InsertGroupCommand { group, index: None }),
            )?;
            group_id
        };

        self.last_group_selection_context = Some(GroupSelectionContext {
            group_ids: vec![group_id.clone()],
            object_ids: object_ids.clone(),
        });
        self.auto_group_context = Some(AutoGroupContext::Active {
            group_id: group_id.clone(),
            page_index: page_indices.into_iter().next().unwrap_or(0),
            style: self.active_style.clone(),
            entrance: self.active_entrance.clone(),
        });
        self.replace_group_selection(vec![group_id.clone()]);
        self.dirty = true;
        Ok(Some(group_id))
    }

    pub fn ungroup_selected_groups(&mut self) -> AnyhowResult<bool> {
        let groups = self
            .project
            .groups
            .iter()
            .enumerate()
            .filter(|(_, group)| self.selection.selected_group_ids.contains(&group.id))
            .map(|(index, group)| (index, group.clone()))
            .collect::<Vec<_>>();
        if groups.is_empty() {
            return Ok(false);
        }

        let object_ids = dedupe_object_ids(
            groups
                .iter()
                .flat_map(|(_, group)| group.glyph_object_ids.clone()),
        );
        let group_ids = groups
            .iter()
            .map(|(_, group)| group.id.clone())
            .collect::<Vec<_>>();
        let commands = groups
            .iter()
            .map(|(index, group)| {
                Box::new(RemoveGroupCommand {
                    group: group.clone(),
                    index: *index,
                }) as Box<dyn pauseink_domain::Command<AnnotationProject>>
            })
            .collect();
        self.history
            .apply(&mut self.project, Box::new(CommandBatch::new(commands)))?;
        self.last_group_selection_context = Some(GroupSelectionContext {
            group_ids,
            object_ids: object_ids.clone(),
        });
        self.repair_auto_group_context_after_project_change();
        self.replace_object_selection(object_ids);
        self.dirty = true;
        Ok(true)
    }

    pub fn z_ordered_object_ids(&self) -> Vec<GlyphObjectId> {
        let mut objects = self.project.glyph_objects.iter().collect::<Vec<_>>();
        objects.sort_by(|left, right| {
            left.ordering
                .z_index
                .cmp(&right.ordering.z_index)
                .then(
                    left.ordering
                        .capture_order
                        .cmp(&right.ordering.capture_order),
                )
                .then(left.id.0.cmp(&right.id.0))
        });
        objects
            .into_iter()
            .map(|object| object.id.clone())
            .collect()
    }

    pub fn move_selected_objects_to_front(&mut self) -> AnyhowResult<bool> {
        self.reorder_selected_objects(|ordered_ids, selected_ids| {
            let selected = selected_ids.iter().cloned().collect::<HashSet<_>>();
            let mut remaining = Vec::new();
            let mut picked = Vec::new();
            for object_id in ordered_ids {
                if selected.contains(&object_id) {
                    picked.push(object_id);
                } else {
                    remaining.push(object_id);
                }
            }
            remaining.extend(picked);
            remaining
        })
    }

    pub fn move_selected_objects_to_back(&mut self) -> AnyhowResult<bool> {
        self.reorder_selected_objects(|ordered_ids, selected_ids| {
            let selected = selected_ids.iter().cloned().collect::<HashSet<_>>();
            let mut remaining = Vec::new();
            let mut picked = Vec::new();
            for object_id in ordered_ids {
                if selected.contains(&object_id) {
                    picked.push(object_id);
                } else {
                    remaining.push(object_id);
                }
            }
            picked.extend(remaining);
            picked
        })
    }

    pub fn move_selected_objects_forward_one(&mut self) -> AnyhowResult<bool> {
        self.reorder_selected_objects(|mut ordered_ids, selected_ids| {
            let selected = selected_ids.iter().cloned().collect::<HashSet<_>>();
            if ordered_ids.len() >= 2 {
                for index in (0..ordered_ids.len() - 1).rev() {
                    if selected.contains(&ordered_ids[index])
                        && !selected.contains(&ordered_ids[index + 1])
                    {
                        ordered_ids.swap(index, index + 1);
                    }
                }
            }
            ordered_ids
        })
    }

    pub fn move_selected_objects_backward_one(&mut self) -> AnyhowResult<bool> {
        self.reorder_selected_objects(|mut ordered_ids, selected_ids| {
            let selected = selected_ids.iter().cloned().collect::<HashSet<_>>();
            for index in 1..ordered_ids.len() {
                if selected.contains(&ordered_ids[index])
                    && !selected.contains(&ordered_ids[index - 1])
                {
                    ordered_ids.swap(index - 1, index);
                }
            }
            ordered_ids
        })
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
            let previous_entrance = self
                .project
                .glyph_objects
                .iter()
                .find(|object| object.id == object_id)
                .map(|object| object.entrance.clone())
                .ok_or_else(|| anyhow::anyhow!("target glyph object not found: {}", object_id.0))?;
            let previous_post_actions = self
                .project
                .glyph_objects
                .iter()
                .find(|object| object.id == object_id)
                .map(|object| object.post_actions.clone())
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
                    Box::new(SetGlyphObjectEntranceCommand {
                        object_id: object_id.clone(),
                        from: previous_entrance,
                        to: self.active_entrance.clone(),
                    }),
                    Box::new(SetGlyphObjectPostActionsCommand {
                        object_id: object_id.clone(),
                        from: previous_post_actions,
                        to: self.active_post_actions.clone(),
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
                entrance: self.active_entrance.clone(),
                post_actions: self.active_post_actions.clone(),
                ordering: OrderingMetadata {
                    z_index: self.project.glyph_objects.len() as i32,
                    capture_order,
                    reveal_order: capture_order,
                },
                created_at,
                ..GlyphObject::default()
            };
            let auto_group_plan = self.plan_auto_group_for_new_object(&object_id, created_at);
            let mut commands: Vec<Box<dyn pauseink_domain::Command<AnnotationProject>>> = vec![
                Box::new(InsertStrokeCommand {
                    stroke,
                    index: None,
                }),
                Box::new(InsertGlyphObjectCommand {
                    object,
                    index: None,
                }),
            ];
            commands.extend(auto_group_plan.commands);
            self.history
                .apply(&mut self.project, Box::new(CommandBatch::new(commands)))?;
            self.auto_group_context = auto_group_plan.next_context;
            object_id
        };

        self.replace_object_selection(vec![selected_object_id.clone()]);
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

    fn selected_target_object_ids(&self) -> Vec<GlyphObjectId> {
        let selected_objects = self
            .selection
            .selected_object_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let selected_groups = self
            .selection
            .selected_group_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let mut target_ids = selected_objects;
        for group in &self.project.groups {
            if selected_groups.contains(&group.id) {
                for object_id in &group.glyph_object_ids {
                    target_ids.insert(object_id.clone());
                }
            }
        }

        self.project
            .glyph_objects
            .iter()
            .filter(|object| target_ids.contains(&object.id))
            .map(|object| object.id.clone())
            .collect()
    }

    fn group_for_object_id(&self, object_id: &GlyphObjectId) -> Option<GroupId> {
        self.project
            .groups
            .iter()
            .find(|group| group.glyph_object_ids.contains(object_id))
            .map(|group| group.id.clone())
    }

    fn group_member_object_ids(&self, group_id: &GroupId) -> Vec<GlyphObjectId> {
        self.project
            .groups
            .iter()
            .find(|group| group.id == *group_id)
            .map(|group| group.glyph_object_ids.clone())
            .unwrap_or_default()
    }

    fn plan_auto_group_for_new_object(
        &mut self,
        new_object_id: &GlyphObjectId,
        created_at: MediaTime,
    ) -> AutoGroupPlan {
        let page_index =
            pauseink_domain::page_index_for_time(&self.project.clear_events, created_at);
        let pending_context = || AutoGroupContext::Pending {
            object_id: new_object_id.clone(),
            page_index,
            style: self.active_style.clone(),
            entrance: self.active_entrance.clone(),
        };

        let Some(context) = self.auto_group_context.clone() else {
            return AutoGroupPlan {
                commands: Vec::new(),
                next_context: Some(pending_context()),
            };
        };
        if !auto_group_context_matches(
            &context,
            page_index,
            &self.active_style,
            &self.active_entrance,
        ) {
            return AutoGroupPlan {
                commands: Vec::new(),
                next_context: Some(pending_context()),
            };
        }

        match context {
            AutoGroupContext::Pending { object_id, .. } => {
                if let Some(group_id) = self.group_for_object_id(&object_id) {
                    return self.plan_append_to_existing_group(
                        &group_id,
                        new_object_id,
                        page_index,
                        pending_context,
                    );
                }

                if self.project.glyph_object_index(&object_id).is_none() {
                    return AutoGroupPlan {
                        commands: Vec::new(),
                        next_context: Some(pending_context()),
                    };
                }

                let group_id = GroupId::new(self.allocate_id("group"));
                let group = Group {
                    id: group_id.clone(),
                    glyph_object_ids: vec![object_id.clone(), new_object_id.clone()],
                    created_at: created_at
                        .min(self.object_created_at(&object_id).unwrap_or(created_at)),
                    ..Group::default()
                };
                AutoGroupPlan {
                    commands: vec![Box::new(InsertGroupCommand { group, index: None })],
                    next_context: Some(AutoGroupContext::Active {
                        group_id,
                        page_index,
                        style: self.active_style.clone(),
                        entrance: self.active_entrance.clone(),
                    }),
                }
            }
            AutoGroupContext::Active { group_id, .. } => self.plan_append_to_existing_group(
                &group_id,
                new_object_id,
                page_index,
                pending_context,
            ),
        }
    }

    fn plan_append_to_existing_group<F>(
        &self,
        group_id: &GroupId,
        new_object_id: &GlyphObjectId,
        page_index: usize,
        pending_context: F,
    ) -> AutoGroupPlan
    where
        F: FnOnce() -> AutoGroupContext,
    {
        let Some(group) = self
            .project
            .groups
            .iter()
            .find(|group| group.id == *group_id)
        else {
            return AutoGroupPlan {
                commands: Vec::new(),
                next_context: Some(pending_context()),
            };
        };
        let mut next_object_ids = group.glyph_object_ids.clone();
        if !next_object_ids.contains(new_object_id) {
            next_object_ids.push(new_object_id.clone());
        }
        AutoGroupPlan {
            commands: vec![Box::new(UpdateGroupMembershipCommand {
                group_id: group_id.clone(),
                from_glyph_object_ids: group.glyph_object_ids.clone(),
                to_glyph_object_ids: next_object_ids,
                from_loose_stroke_ids: group.loose_stroke_ids.clone(),
                to_loose_stroke_ids: group.loose_stroke_ids.clone(),
            })],
            next_context: Some(AutoGroupContext::Active {
                group_id: group_id.clone(),
                page_index,
                style: self.active_style.clone(),
                entrance: self.active_entrance.clone(),
            }),
        }
    }

    fn object_created_at(&self, object_id: &GlyphObjectId) -> Option<MediaTime> {
        self.project
            .glyph_objects
            .iter()
            .find(|object| object.id == *object_id)
            .map(|object| object.created_at)
    }

    fn apply_style_to_object_ids(
        &mut self,
        object_ids: &[GlyphObjectId],
        style: StyleSnapshot,
    ) -> AnyhowResult<bool> {
        let changes = object_ids
            .iter()
            .filter_map(|object_id| {
                self.project
                    .glyph_objects
                    .iter()
                    .find(|object| object.id == *object_id)
                    .filter(|object| object.style != style)
                    .map(|object| GlyphObjectStyleChange {
                        object_id: object_id.clone(),
                        from: object.style.clone(),
                        to: style.clone(),
                    })
            })
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(false);
        }

        self.history.apply(
            &mut self.project,
            Box::new(BatchSetGlyphObjectStyleCommand { changes }),
        )?;
        self.dirty = true;
        Ok(true)
    }

    fn apply_entrance_to_object_ids(
        &mut self,
        object_ids: &[GlyphObjectId],
        entrance: EntranceBehavior,
    ) -> AnyhowResult<bool> {
        let changes = object_ids
            .iter()
            .filter_map(|object_id| {
                self.project
                    .glyph_objects
                    .iter()
                    .find(|object| object.id == *object_id)
                    .filter(|object| object.entrance != entrance)
                    .map(|object| GlyphObjectEntranceChange {
                        object_id: object_id.clone(),
                        from: object.entrance.clone(),
                        to: entrance.clone(),
                    })
            })
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(false);
        }

        self.history.apply(
            &mut self.project,
            Box::new(BatchSetGlyphObjectEntranceCommand { changes }),
        )?;
        self.dirty = true;
        Ok(true)
    }

    fn apply_post_actions_to_object_ids(
        &mut self,
        object_ids: &[GlyphObjectId],
        post_actions: Vec<PostAction>,
    ) -> AnyhowResult<bool> {
        let changes = object_ids
            .iter()
            .filter_map(|object_id| {
                self.project
                    .glyph_objects
                    .iter()
                    .find(|object| object.id == *object_id)
                    .filter(|object| object.post_actions != post_actions)
                    .map(|object| GlyphObjectPostActionsChange {
                        object_id: object_id.clone(),
                        from: object.post_actions.clone(),
                        to: post_actions.clone(),
                    })
            })
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(false);
        }

        self.history.apply(
            &mut self.project,
            Box::new(BatchSetGlyphObjectPostActionsCommand { changes }),
        )?;
        self.dirty = true;
        Ok(true)
    }

    fn reorder_selected_objects<F>(&mut self, transform: F) -> AnyhowResult<bool>
    where
        F: FnOnce(Vec<GlyphObjectId>, &[GlyphObjectId]) -> Vec<GlyphObjectId>,
    {
        let selected_ids = self.selected_target_object_ids();
        if selected_ids.is_empty() {
            return Ok(false);
        }
        let current_order = self.z_ordered_object_ids();
        let next_order = transform(current_order.clone(), &selected_ids);
        if next_order == current_order {
            return Ok(false);
        }

        let changes = next_order
            .iter()
            .enumerate()
            .filter_map(|(index, object_id)| {
                self.project
                    .glyph_objects
                    .iter()
                    .find(|object| object.id == *object_id)
                    .and_then(|object| {
                        let to = index as i32;
                        (object.ordering.z_index != to).then(|| GlyphObjectZIndexChange {
                            object_id: object_id.clone(),
                            from: object.ordering.z_index,
                            to,
                        })
                    })
            })
            .collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(false);
        }

        self.history.apply(
            &mut self.project,
            Box::new(NormalizeZOrderCommand { changes }),
        )?;
        self.dirty = true;
        Ok(true)
    }

    fn repair_selection_after_project_change(&mut self) {
        self.repair_auto_group_context_after_project_change();
        self.selection
            .selected_object_ids
            .retain(|object_id| self.project.glyph_object_index(object_id).is_some());
        self.selection
            .selected_group_ids
            .retain(|group_id| self.project.group_index(group_id).is_some());
        if self
            .selection
            .focused_object_id
            .as_ref()
            .is_some_and(|object_id| self.project.glyph_object_index(object_id).is_none())
        {
            self.selection.focused_object_id = None;
        }
        if self
            .selection
            .focused_group_id
            .as_ref()
            .is_some_and(|group_id| self.project.group_index(group_id).is_none())
        {
            self.selection.focused_group_id = None;
        }

        if let Some(context) = &self.last_group_selection_context {
            let existing_group_ids = context
                .group_ids
                .iter()
                .filter(|group_id| self.project.group_index(group_id).is_some())
                .cloned()
                .collect::<Vec<_>>();
            let existing_object_ids = context
                .object_ids
                .iter()
                .filter(|object_id| self.project.glyph_object_index(object_id).is_some())
                .cloned()
                .collect::<Vec<_>>();

            let selection_matches_context_objects = self.selection.selected_group_ids.is_empty()
                && self.selection.selected_object_ids == existing_object_ids;
            if !existing_group_ids.is_empty()
                && (selection_matches_context_objects
                    || (self.selection.selected_object_ids.is_empty()
                        && self.selection.selected_group_ids.is_empty()))
            {
                self.replace_group_selection(existing_group_ids);
                return;
            }

            if self.selection.selected_object_ids.is_empty()
                && self.selection.selected_group_ids.is_empty()
                && !existing_object_ids.is_empty()
            {
                self.replace_object_selection(existing_object_ids);
                return;
            }
        }

        if self.selection.focused_object_id.is_none() {
            self.selection.focused_object_id = self.selection.selected_object_ids.last().cloned();
        }
        if self.selection.focused_group_id.is_none() {
            self.selection.focused_group_id = self.selection.selected_group_ids.last().cloned();
        }
        if self
            .last_created_object_id
            .as_ref()
            .is_some_and(|object_id| self.project.glyph_object_index(object_id).is_none())
        {
            self.last_created_object_id = None;
        }
    }

    fn repair_auto_group_context_after_project_change(&mut self) {
        let should_clear = match &self.auto_group_context {
            Some(AutoGroupContext::Pending { object_id, .. }) => {
                self.project.glyph_object_index(object_id).is_none()
            }
            Some(AutoGroupContext::Active { group_id, .. }) => {
                self.project.group_index(group_id).is_none()
            }
            None => false,
        };
        if should_clear {
            self.auto_group_context = None;
        }
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
        self.note_auto_group_break();
        self.dirty = true;
        Ok(clear_event_id)
    }

    pub fn undo(&mut self) -> AnyhowResult<bool> {
        let changed = self.history.undo(&mut self.project)?;
        if changed {
            self.dirty = true;
            self.repair_selection_after_project_change();
            self.note_auto_group_break();
        }
        Ok(changed)
    }

    pub fn redo(&mut self) -> AnyhowResult<bool> {
        let changed = self.history.redo(&mut self.project)?;
        if changed {
            self.dirty = true;
            self.repair_selection_after_project_change();
            self.note_auto_group_break();
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

    fn resolve_media_source_path(&self, source_path: &Path) -> PathBuf {
        if source_path.is_absolute() {
            source_path.to_path_buf()
        } else {
            self.document_path
                .as_ref()
                .and_then(|path| path.parent())
                .map(|parent| parent.join(source_path))
                .unwrap_or_else(|| source_path.to_path_buf())
        }
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

fn dedupe_object_ids(ids: impl IntoIterator<Item = GlyphObjectId>) -> Vec<GlyphObjectId> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for object_id in ids {
        if seen.insert(object_id.clone()) {
            deduped.push(object_id);
        }
    }
    deduped
}

fn dedupe_group_ids(ids: impl IntoIterator<Item = GroupId>) -> Vec<GroupId> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for group_id in ids {
        if seen.insert(group_id.clone()) {
            deduped.push(group_id);
        }
    }
    deduped
}

fn auto_group_context_matches(
    context: &AutoGroupContext,
    page_index: usize,
    style: &StyleSnapshot,
    entrance: &EntranceBehavior,
) -> bool {
    match context {
        AutoGroupContext::Pending {
            page_index: context_page,
            style: context_style,
            entrance: context_entrance,
            ..
        }
        | AutoGroupContext::Active {
            page_index: context_page,
            style: context_style,
            entrance: context_entrance,
            ..
        } => *context_page == page_index && context_style == style && context_entrance == entrance,
    }
}

fn toggle_vec_membership<T>(items: &mut Vec<T>, value: T)
where
    T: Clone + PartialEq,
{
    if let Some(index) = items.iter().position(|item| item == &value) {
        items.remove(index);
    } else {
        items.push(value);
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
    use std::cell::RefCell;
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

    struct RecordingMediaProvider {
        probe: MediaProbe,
        calls: RefCell<Vec<PathBuf>>,
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

    impl MediaProvider for RecordingMediaProvider {
        fn probe(&self, source_path: &Path) -> Result<MediaProbe, MediaError> {
            self.calls.borrow_mut().push(source_path.to_path_buf());
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
    fn restore_media_from_hint_resolves_relative_path_from_project_file() {
        let temp_dir = tempdir().expect("temp dir");
        let project_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(project_dir.join("media")).expect("media dir");

        let mut session = AppSession::default();
        session.document.project.media = serde_json::json!({
            "source_path": "media/demo.mp4",
            "width": 1280,
        });
        session.document_path = Some(project_dir.join("demo.pauseink"));
        let provider = RecordingMediaProvider {
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
            calls: RefCell::new(Vec::new()),
        };

        let restored = session
            .restore_media_from_hint(&provider)
            .expect("restore should succeed");

        assert!(restored);
        assert_eq!(
            provider.calls.borrow().as_slice(),
            &[project_dir.join("media/demo.mp4")]
        );
        assert_eq!(
            session
                .imported_media
                .as_ref()
                .map(|media| media.source_path.clone()),
            Some(project_dir.join("media/demo.mp4"))
        );
        assert!(session.playback.is_some());
        assert!(!session.dirty);
        assert_eq!(
            session
                .document
                .project
                .media
                .get("source_path")
                .and_then(Value::as_str),
            Some("media/demo.mp4")
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
    fn free_ink_commit_captures_active_post_actions_into_object() {
        let mut session = AppSession::default();
        session.active_post_actions = vec![pauseink_domain::PostAction {
            timing_scope: pauseink_domain::PostActionTimingScope::AfterGlyphObject,
            action: pauseink_domain::PostActionKind::StyleChange {
                style: StyleSnapshot {
                    color: RgbaColor::new(255, 180, 90, 255),
                    opacity: 0.4,
                    ..StyleSnapshot::default()
                },
            },
        }];

        session.begin_stroke(Point2 { x: 12.0, y: 18.0 }, MediaTime::from_millis(100));
        session.append_stroke_point(Point2 { x: 36.0, y: 28.0 }, MediaTime::from_millis(120));
        let object_id = session
            .commit_stroke(false)
            .expect("commit should succeed")
            .expect("object id");
        let object = session
            .project
            .glyph_objects
            .iter()
            .find(|object| object.id == object_id)
            .expect("object should exist");

        assert_eq!(object.post_actions, session.active_post_actions);
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
    fn multi_select_style_and_entrance_apply_to_selected_objects_and_roundtrip_history() {
        let mut session = AppSession::default();
        for start_x in [0.0, 40.0] {
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 20.0,
                    y: 20.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
        }

        let object_ids = session
            .project
            .glyph_objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        session.replace_object_selection(object_ids.clone());
        session.active_style.thickness = 19.0;
        session.active_style.opacity = 0.35;
        session.active_entrance.kind = pauseink_domain::EntranceKind::PathTrace;
        session.active_entrance.speed_scalar = 1.8;

        session
            .apply_active_style_to_selection()
            .expect("style apply should succeed");
        session
            .apply_active_entrance_to_selection()
            .expect("entrance apply should succeed");

        assert_eq!(session.project.glyph_objects[0].style.thickness, 19.0);
        assert_eq!(session.project.glyph_objects[1].style.thickness, 19.0);
        assert_eq!(
            session.project.glyph_objects[0].entrance.kind,
            pauseink_domain::EntranceKind::PathTrace
        );
        assert_eq!(
            session.project.glyph_objects[1].entrance.kind,
            pauseink_domain::EntranceKind::PathTrace
        );

        assert!(session.undo().expect("undo should succeed"));
        assert!(session.undo().expect("undo should succeed"));
        assert_eq!(session.project.glyph_objects[0].style.thickness, 6.0);
        assert_eq!(session.project.glyph_objects[1].style.thickness, 6.0);
        assert_eq!(
            session.project.glyph_objects[0].entrance.kind,
            pauseink_domain::EntranceKind::Instant
        );
        assert_eq!(
            session.project.glyph_objects[1].entrance.kind,
            pauseink_domain::EntranceKind::Instant
        );
    }

    #[test]
    fn group_selected_objects_and_undo_redo_keep_selection_consistent() {
        let mut session = AppSession::default();
        for (start_x, red) in [(0.0, 255), (40.0, 128)] {
            session.active_style.color = RgbaColor::new(red, 255, 255, 255);
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 20.0,
                    y: 20.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
        }
        let object_ids = session
            .project
            .glyph_objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        session.replace_object_selection(object_ids.clone());

        let group_id = session
            .group_selected_objects()
            .expect("grouping should succeed")
            .expect("group should be created");

        assert_eq!(session.project.groups.len(), 1);
        assert_eq!(session.selected_group_ids(), vec![group_id.clone()]);
        assert!(session.selected_object_ids().is_empty());

        assert!(session.undo().expect("undo should succeed"));
        assert!(session.project.groups.is_empty());
        assert_eq!(session.selected_object_ids(), object_ids);
        assert!(session.selected_group_ids().is_empty());

        assert!(session.redo().expect("redo should succeed"));
        assert_eq!(session.selected_group_ids(), vec![group_id]);
        assert!(session.selected_object_ids().is_empty());
    }

    #[test]
    fn grouping_selected_groups_merges_members_without_nesting() {
        let mut session = AppSession::default();
        for start_x in [0.0, 40.0, 80.0, 120.0] {
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 20.0,
                    y: 20.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
        }

        let object_ids = session
            .project
            .glyph_objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        session.replace_object_selection(vec![object_ids[0].clone(), object_ids[1].clone()]);
        let first_group_id = session
            .group_selected_objects()
            .expect("first grouping should succeed")
            .expect("first group id");
        session.replace_object_selection(vec![object_ids[2].clone(), object_ids[3].clone()]);
        let second_group_id = session
            .group_selected_objects()
            .expect("second grouping should succeed")
            .expect("second group id");

        session.replace_group_selection(vec![first_group_id.clone(), second_group_id.clone()]);
        let merged_group_id = session
            .group_selected_objects()
            .expect("merge should succeed")
            .expect("merged group id");

        assert_eq!(merged_group_id, first_group_id);
        assert_eq!(session.project.groups.len(), 1);
        assert_eq!(
            session.project.groups[0].glyph_object_ids, object_ids,
            "group 同士の group 化は flat merge にしたい"
        );
        assert_eq!(session.selected_group_ids(), vec![merged_group_id]);
    }

    #[test]
    fn auto_group_groups_same_style_strokes_on_same_page() {
        let mut session = AppSession::default();

        for start_x in [0.0, 40.0, 80.0] {
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 18.0,
                    y: 20.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
        }

        assert_eq!(session.project.groups.len(), 1);
        assert_eq!(session.project.groups[0].glyph_object_ids.len(), 3);
    }

    #[test]
    fn auto_group_breaks_on_style_change_and_page_change() {
        let mut session = AppSession::default();

        for (start_x, red) in [(0.0, 255), (40.0, 128), (80.0, 128)] {
            session.active_style.color = RgbaColor::new(red, 255, 255, 255);
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 16.0,
                    y: 16.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
        }

        assert_eq!(session.project.groups.len(), 1);
        assert_eq!(
            session.project.groups[0].glyph_object_ids.len(),
            2,
            "style change 後は新しい chain として数え直したい"
        );

        session.seek(MediaTime::from_millis(500));
        session
            .insert_clear_event(ClearKind::Instant)
            .expect("clear event should insert");

        for start_x in [120.0, 160.0] {
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(600 + start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 16.0,
                    y: 16.0,
                },
                MediaTime::from_millis(610 + start_x as i64),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
        }

        assert_eq!(session.project.groups.len(), 2);
        assert_eq!(session.project.groups[1].glyph_object_ids.len(), 2);
    }

    #[test]
    fn move_selected_objects_forward_and_backward_preserves_relative_order() {
        let mut session = AppSession::default();
        for (start_x, z_index) in [(0.0, 0), (30.0, 1), (60.0, 2), (90.0, 3)] {
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 10.0,
                    y: 10.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            let object_id = session
                .commit_stroke(false)
                .expect("stroke commit should succeed")
                .expect("object id");
            session
                .project
                .glyph_objects
                .iter_mut()
                .find(|object| object.id == object_id)
                .expect("object exists")
                .ordering
                .z_index = z_index;
        }
        let object_ids = session
            .project
            .glyph_objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        session.replace_object_selection(vec![object_ids[1].clone(), object_ids[2].clone()]);

        session
            .move_selected_objects_backward_one()
            .expect("move backward should succeed");
        assert_eq!(
            session.z_ordered_object_ids(),
            vec![
                object_ids[1].clone(),
                object_ids[2].clone(),
                object_ids[0].clone(),
                object_ids[3].clone()
            ]
        );

        session
            .move_selected_objects_forward_one()
            .expect("move forward should succeed");
        assert_eq!(session.z_ordered_object_ids(), object_ids);

        session
            .move_selected_objects_to_front()
            .expect("move to front should succeed");
        assert_eq!(
            session.z_ordered_object_ids(),
            vec![
                object_ids[0].clone(),
                object_ids[3].clone(),
                object_ids[1].clone(),
                object_ids[2].clone()
            ]
        );
    }

    #[test]
    fn ungroup_selected_groups_restores_object_selection() {
        let mut session = AppSession::default();
        for (index, start_x) in [0.0, 40.0].into_iter().enumerate() {
            session.begin_stroke(
                Point2 { x: start_x, y: 0.0 },
                MediaTime::from_millis(start_x as i64),
            );
            session.append_stroke_point(
                Point2 {
                    x: start_x + 20.0,
                    y: 20.0,
                },
                MediaTime::from_millis(start_x as i64 + 10),
            );
            session
                .commit_stroke(false)
                .expect("stroke commit should succeed");
            if index == 0 {
                session.note_auto_group_break();
            }
        }
        let object_ids = session
            .project
            .glyph_objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        session.replace_object_selection(object_ids.clone());
        let group_id = session
            .group_selected_objects()
            .expect("grouping should succeed")
            .expect("group id");

        session.replace_group_selection(vec![group_id]);
        assert!(session
            .ungroup_selected_groups()
            .expect("ungroup should succeed"));

        assert!(session.project.groups.is_empty());
        assert_eq!(session.selected_object_ids(), object_ids);
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
