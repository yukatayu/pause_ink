use std::cmp::Ordering;

mod annotations;
mod history;
mod project_commands;

use serde::{Deserialize, Serialize};

pub use annotations::*;
pub use history::*;
pub use project_commands::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeBase {
    pub numerator: u32,
    pub denominator: u32,
}

impl TimeBase {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub const fn milliseconds() -> Self {
        Self::new(1, 1_000)
    }
}

impl Default for TimeBase {
    fn default() -> Self {
        Self::milliseconds()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct MediaTime {
    pub ticks: i64,
    pub time_base: TimeBase,
}

impl MediaTime {
    pub const fn new(ticks: i64, time_base: TimeBase) -> Self {
        Self { ticks, time_base }
    }

    pub const fn from_millis(value: i64) -> Self {
        Self::new(value, TimeBase::milliseconds())
    }

    fn ordering_key(self, other: Self) -> (i128, i128) {
        let left = self.ticks as i128
            * self.time_base.numerator as i128
            * other.time_base.denominator as i128;
        let right = other.ticks as i128
            * other.time_base.numerator as i128
            * self.time_base.denominator as i128;
        (left, right)
    }
}

impl Default for MediaTime {
    fn default() -> Self {
        Self::from_millis(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaDuration {
    pub ticks: i64,
    pub time_base: TimeBase,
}

impl MediaDuration {
    pub const fn new(ticks: i64, time_base: TimeBase) -> Self {
        Self { ticks, time_base }
    }

    pub const fn from_millis(value: i64) -> Self {
        Self::new(value, TimeBase::milliseconds())
    }
}

impl Default for MediaDuration {
    fn default() -> Self {
        Self::from_millis(0)
    }
}

impl PartialEq for TimeBase {
    fn eq(&self, other: &Self) -> bool {
        self.numerator == other.numerator && self.denominator == other.denominator
    }
}

impl Eq for TimeBase {}

impl PartialEq for MediaTime {
    fn eq(&self, other: &Self) -> bool {
        let (left, right) = self.ordering_key(*other);
        left == right
    }
}

impl Eq for MediaTime {}

impl PartialOrd for MediaTime {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MediaTime {
    fn cmp(&self, other: &Self) -> Ordering {
        let (left, right) = self.ordering_key(*other);
        left.cmp(&right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearKind {
    Instant,
    Ordered,
    ReverseOrdered,
    WipeOut,
    DissolveOut,
}

impl Default for ClearKind {
    fn default() -> Self {
        Self::Instant
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClearEvent {
    pub id: ClearEventId,
    pub time: MediaTime,
    pub kind: ClearKind,
    pub duration: MediaDuration,
    pub granularity: ClearTargetGranularity,
    pub ordering: ClearOrdering,
}

impl Default for ClearEvent {
    fn default() -> Self {
        Self {
            id: ClearEventId::new(""),
            time: MediaTime::default(),
            kind: ClearKind::default(),
            duration: MediaDuration::default(),
            granularity: ClearTargetGranularity::default(),
            ordering: ClearOrdering::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearTargetGranularity {
    Object,
    Group,
    Stroke,
    AllParallel,
}

impl Default for ClearTargetGranularity {
    fn default() -> Self {
        Self::AllParallel
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClearOrdering {
    Serial,
    Reverse,
    Parallel,
}

impl Default for ClearOrdering {
    fn default() -> Self {
        Self::Parallel
    }
}

pub fn page_index_for_time(clears: &[ClearEvent], time: MediaTime) -> usize {
    clears.iter().filter(|clear| clear.time <= time).count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInterval {
    pub index: usize,
    pub start_time: Option<MediaTime>,
    pub end_time: Option<MediaTime>,
}

pub fn page_interval_for_time(clears: &[ClearEvent], time: MediaTime) -> PageInterval {
    let index = page_index_for_time(clears, time);
    PageInterval {
        index,
        start_time: index
            .checked_sub(1)
            .and_then(|prior| clears.get(prior).map(|clear| clear.time)),
        end_time: clears.get(index).map(|clear| clear.time),
    }
}

pub fn page_count(clears: &[ClearEvent]) -> usize {
    clears.len() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_boundary_belongs_to_next_page_even_with_mixed_time_bases() {
        let ntsc_tick = TimeBase::new(1, 30_000);
        let clears = vec![
            ClearEvent {
                id: ClearEventId::new("clear-1"),
                time: MediaTime::new(1001, ntsc_tick),
                kind: ClearKind::Instant,
                duration: MediaDuration::from_millis(0),
                granularity: ClearTargetGranularity::AllParallel,
                ordering: ClearOrdering::Parallel,
            },
        ];

        let just_before = MediaTime::new(1000, ntsc_tick);
        let exactly_on_clear = MediaTime::new(1, TimeBase::new(1001, 30_000));

        assert_eq!(page_index_for_time(&clears, just_before), 0);
        assert_eq!(page_index_for_time(&clears, exactly_on_clear), 1);
    }

    #[test]
    fn media_time_compares_across_time_bases() {
        let one_second = MediaTime::from_millis(1_000);
        let ntsc_frame = MediaTime::new(1001, TimeBase::new(1, 30_000));

        assert!(one_second > ntsc_frame);
        assert_eq!(one_second, MediaTime::new(1, TimeBase::new(1, 1)));
    }

    #[test]
    fn history_respects_depth_limit_and_invalidates_redo() {
        let mut state = Vec::<String>::new();
        let mut history = CommandHistory::with_limit(2);

        history
            .apply(&mut state, Box::new(PushValue("first")))
            .expect("first command should apply");
        history
            .apply(&mut state, Box::new(PushValue("second")))
            .expect("second command should apply");
        history
            .apply(&mut state, Box::new(PushValue("third")))
            .expect("third command should apply");

        assert_eq!(state, vec!["first", "second", "third"]);

        assert!(history.undo(&mut state).expect("undo should succeed"));
        assert_eq!(state, vec!["first", "second"]);
        assert!(history.undo(&mut state).expect("undo should succeed"));
        assert_eq!(state, vec!["first"]);
        assert!(!history
            .undo(&mut state)
            .expect("oldest command should be evicted at depth limit"));

        history
            .apply(&mut state, Box::new(PushValue("replacement")))
            .expect("new command should apply");
        assert!(!history
            .redo(&mut state)
            .expect("redo should be invalidated after a new command"));
    }

    #[test]
    fn grouped_commands_undo_in_reverse_order() {
        let mut state = Vec::<String>::new();
        let mut history = CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH);

        history
            .apply(
                &mut state,
                Box::new(CommandBatch::new(vec![
                    Box::new(PushValue("first")),
                    Box::new(PushValue("second")),
                ])),
            )
            .expect("batched command should apply");

        assert_eq!(state, vec!["first", "second"]);
        assert!(history.undo(&mut state).expect("batch undo should succeed"));
        assert!(state.is_empty());
    }

    #[test]
    fn stroke_keeps_raw_stabilized_and_derived_layers_separate() {
        let raw_sample = StrokeSample {
            position: Point2 { x: 10.0, y: 20.0 },
            at: MediaTime::from_millis(100),
            pressure: None,
        };
        let stabilized_sample = StrokeSample {
            position: Point2 { x: 11.0, y: 20.5 },
            at: MediaTime::from_millis(100),
            pressure: None,
        };
        let stroke = Stroke {
            id: StrokeId::new("stroke-1"),
            raw_samples: vec![raw_sample.clone()],
            stabilized_samples: vec![stabilized_sample.clone()],
            derived_path: DerivedStrokePath {
                points: vec![Point2 { x: 11.0, y: 20.5 }],
            },
            style: StyleSnapshot::default(),
            created_at: MediaTime::from_millis(100),
        };

        assert_eq!(stroke.raw_samples[0], raw_sample);
        assert_eq!(stroke.stabilized_samples[0], stabilized_sample);
        assert_ne!(stroke.raw_samples[0].position, stroke.stabilized_samples[0].position);
        assert_eq!(stroke.derived_path.points.len(), 1);
    }

    #[test]
    fn glyph_object_keeps_z_order_separate_from_capture_and_reveal_order() {
        let entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            scope: EffectScope::GlyphObject,
            order: EffectOrder::Serial,
            duration_mode: EntranceDurationMode::FixedTotalDuration,
            duration: MediaDuration::from_millis(250),
            speed_scalar: 1.0,
            head_effect: None,
        };
        let object = GlyphObject {
            id: GlyphObjectId::new("object-1"),
            stroke_ids: vec![StrokeId::new("stroke-1")],
            style: StyleSnapshot::default(),
            preset_bindings: PresetBindings::default(),
            entrance,
            post_actions: vec![],
            transform: GeometryTransform::default(),
            ordering: OrderingMetadata {
                z_index: 12,
                capture_order: 1,
                reveal_order: 7,
            },
            created_at: MediaTime::from_millis(100),
        };

        assert_eq!(object.ordering.z_index, 12);
        assert_eq!(object.ordering.capture_order, 1);
        assert_eq!(object.ordering.reveal_order, 7);
        assert_ne!(object.ordering.capture_order, object.ordering.reveal_order);
    }

    #[test]
    fn group_can_reference_objects_and_loose_strokes_together() {
        let group = Group {
            id: GroupId::new("group-1"),
            name: Some("demo".into()),
            glyph_object_ids: vec![GlyphObjectId::new("object-1")],
            loose_stroke_ids: vec![StrokeId::new("stroke-free-1")],
            style_override: Some(StyleSnapshot::default()),
            preset_bindings: PresetBindings::default(),
            entrance: None,
            post_actions: vec![],
            created_at: MediaTime::from_millis(200),
        };

        assert_eq!(group.glyph_object_ids.len(), 1);
        assert_eq!(group.loose_stroke_ids.len(), 1);
    }

    #[test]
    fn clear_event_keeps_effect_scope_and_ordering() {
        let clear = ClearEvent {
            id: ClearEventId::new("clear-1"),
            time: MediaTime::from_millis(400),
            kind: ClearKind::WipeOut,
            duration: MediaDuration::from_millis(300),
            granularity: ClearTargetGranularity::Group,
            ordering: ClearOrdering::Reverse,
        };

        assert_eq!(clear.duration, MediaDuration::from_millis(300));
        assert_eq!(clear.granularity, ClearTargetGranularity::Group);
        assert_eq!(clear.ordering, ClearOrdering::Reverse);
    }

    #[test]
    fn typed_project_commands_roundtrip_through_history() {
        let mut project = AnnotationProject::default();
        let mut history = CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH);

        let stroke = Stroke {
            id: StrokeId::new("stroke-1"),
            created_at: MediaTime::from_millis(100),
            ..Stroke::default()
        };
        let object = GlyphObject {
            id: GlyphObjectId::new("object-1"),
            stroke_ids: vec![StrokeId::new("stroke-1")],
            created_at: MediaTime::from_millis(100),
            ..GlyphObject::default()
        };

        history
            .apply(
                &mut project,
                Box::new(CommandBatch::new(vec![
                    Box::new(InsertStrokeCommand {
                        stroke: stroke.clone(),
                        index: None,
                    }),
                    Box::new(InsertGlyphObjectCommand {
                        object: object.clone(),
                        index: None,
                    }),
                ])),
            )
            .expect("typed insert batch should apply");

        assert_eq!(project.strokes.len(), 1);
        assert_eq!(project.glyph_objects.len(), 1);

        assert!(history.undo(&mut project).expect("undo should succeed"));
        assert!(project.strokes.is_empty());
        assert!(project.glyph_objects.is_empty());

        assert!(history.redo(&mut project).expect("redo should succeed"));
        assert_eq!(project.strokes[0].id.0, "stroke-1");
        assert_eq!(project.glyph_objects[0].id.0, "object-1");
    }

    #[test]
    fn z_order_command_is_reversible_without_touching_capture_or_reveal_order() {
        let mut project = AnnotationProject {
            glyph_objects: vec![GlyphObject {
                id: GlyphObjectId::new("object-1"),
                ordering: OrderingMetadata {
                    z_index: 3,
                    capture_order: 10,
                    reveal_order: 2,
                },
                ..GlyphObject::default()
            }],
            ..AnnotationProject::default()
        };
        let mut history = CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH);

        history
            .apply(
                &mut project,
                Box::new(SetGlyphObjectZIndexCommand {
                    object_id: GlyphObjectId::new("object-1"),
                    from: 3,
                    to: 9,
                }),
            )
            .expect("z-order command should apply");

        assert_eq!(project.glyph_objects[0].ordering.z_index, 9);
        assert_eq!(project.glyph_objects[0].ordering.capture_order, 10);
        assert_eq!(project.glyph_objects[0].ordering.reveal_order, 2);

        assert!(history.undo(&mut project).expect("undo should succeed"));
        assert_eq!(project.glyph_objects[0].ordering.z_index, 3);
    }

    #[test]
    fn append_stroke_command_groups_multiple_strokes_under_one_object() {
        let mut project = AnnotationProject {
            strokes: vec![Stroke {
                id: StrokeId::new("stroke-1"),
                ..Stroke::default()
            }],
            glyph_objects: vec![GlyphObject {
                id: GlyphObjectId::new("object-1"),
                stroke_ids: vec![StrokeId::new("stroke-1")],
                ..GlyphObject::default()
            }],
            ..AnnotationProject::default()
        };
        let mut history = CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH);

        history
            .apply(
                &mut project,
                Box::new(CommandBatch::new(vec![
                    Box::new(InsertStrokeCommand {
                        stroke: Stroke {
                            id: StrokeId::new("stroke-2"),
                            ..Stroke::default()
                        },
                        index: None,
                    }),
                    Box::new(AppendStrokeToGlyphObjectCommand {
                        object_id: GlyphObjectId::new("object-1"),
                        stroke_id: StrokeId::new("stroke-2"),
                    }),
                ])),
            )
            .expect("append stroke batch should apply");

        assert_eq!(
            project.glyph_objects[0].stroke_ids,
            vec![StrokeId::new("stroke-1"), StrokeId::new("stroke-2")]
        );

        assert!(history.undo(&mut project).expect("undo should succeed"));
        assert_eq!(project.strokes.len(), 1);
        assert_eq!(project.glyph_objects[0].stroke_ids, vec![StrokeId::new("stroke-1")]);
    }

    #[test]
    fn group_and_clear_event_commands_attach_to_typed_project() {
        let mut project = AnnotationProject::default();
        let mut history = CommandHistory::with_limit(DEFAULT_HISTORY_DEPTH);

        history
            .apply(
                &mut project,
                Box::new(InsertGroupCommand {
                    group: Group {
                        id: GroupId::new("group-1"),
                        glyph_object_ids: vec![GlyphObjectId::new("object-1")],
                        loose_stroke_ids: vec![StrokeId::new("stroke-free-1")],
                        ..Group::default()
                    },
                    index: None,
                }),
            )
            .expect("group insert should apply");
        history
            .apply(
                &mut project,
                Box::new(InsertClearEventCommand {
                    clear_event: ClearEvent {
                        id: ClearEventId::new("clear-1"),
                        time: MediaTime::from_millis(500),
                        ..ClearEvent::default()
                    },
                    index: None,
                }),
            )
            .expect("clear insert should apply");

        assert_eq!(project.groups.len(), 1);
        assert_eq!(project.clear_events.len(), 1);
        assert_eq!(page_index_for_time(&project.clear_events, MediaTime::from_millis(600)), 1);
    }

    #[test]
    fn page_interval_tracks_previous_and_next_clear_boundaries() {
        let clears = vec![
            ClearEvent {
                id: ClearEventId::new("clear-1"),
                time: MediaTime::from_millis(1_000),
                kind: ClearKind::Instant,
                duration: MediaDuration::from_millis(0),
                granularity: ClearTargetGranularity::AllParallel,
                ordering: ClearOrdering::Parallel,
            },
            ClearEvent {
                id: ClearEventId::new("clear-2"),
                time: MediaTime::from_millis(2_500),
                kind: ClearKind::Instant,
                duration: MediaDuration::from_millis(0),
                granularity: ClearTargetGranularity::AllParallel,
                ordering: ClearOrdering::Parallel,
            },
        ];

        let interval = page_interval_for_time(&clears, MediaTime::from_millis(1_500));

        assert_eq!(interval.index, 1);
        assert_eq!(interval.start_time, Some(MediaTime::from_millis(1_000)));
        assert_eq!(interval.end_time, Some(MediaTime::from_millis(2_500)));
        assert_eq!(page_count(&clears), 3);
    }

    #[test]
    fn style_snapshot_keeps_outline_shadow_and_glow_fields() {
        let style = StyleSnapshot {
            color: RgbaColor::new(255, 240, 32, 255),
            thickness: 8.0,
            outline: OutlineStyle {
                enabled: true,
                width: 2.0,
                color: RgbaColor::new(0, 0, 0, 255),
            },
            drop_shadow: DropShadowStyle {
                enabled: true,
                offset_x: 3.0,
                offset_y: 4.0,
                blur_radius: 6.0,
                color: RgbaColor::new(32, 32, 32, 200),
            },
            glow: GlowStyle {
                enabled: true,
                blur_radius: 10.0,
                color: RgbaColor::new(255, 255, 200, 180),
            },
            ..StyleSnapshot::default()
        };

        assert_eq!(style.thickness, 8.0);
        assert!(style.outline.enabled);
        assert!(style.drop_shadow.enabled);
        assert!(style.glow.enabled);
    }

    #[test]
    fn stroke_and_glyph_page_index_follow_creation_anchor() {
        let clears = vec![ClearEvent {
            id: ClearEventId::new("clear-1"),
            time: MediaTime::from_millis(150),
            kind: ClearKind::Instant,
            duration: MediaDuration::from_millis(0),
            granularity: ClearTargetGranularity::AllParallel,
            ordering: ClearOrdering::Parallel,
        }];
        let stroke = Stroke {
            created_at: MediaTime::from_millis(100),
            ..Stroke::default()
        };
        let glyph = GlyphObject {
            created_at: MediaTime::from_millis(150),
            ..GlyphObject::default()
        };

        assert_eq!(stroke.page_index(&clears), 0);
        assert_eq!(glyph.page_index(&clears), 1);
    }

    struct PushValue(&'static str);

    impl Command<Vec<String>> for PushValue {
        fn apply(&self, state: &mut Vec<String>) -> Result<(), CommandError> {
            state.push(self.0.to_owned());
            Ok(())
        }

        fn undo(&self, state: &mut Vec<String>) -> Result<(), CommandError> {
            match state.pop() {
                Some(value) if value == self.0 => Ok(()),
                Some(value) => Err(CommandError::new(format!(
                    "unexpected undo order: expected {}, got {}",
                    self.0, value
                ))),
                None => Err(CommandError::new("state was empty during undo")),
            }
        }
    }
}
