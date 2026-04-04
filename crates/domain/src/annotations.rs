use serde::{Deserialize, Serialize};

use crate::{page_index_for_time, ClearEvent, MediaDuration, MediaTime};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }
    };
}

id_type!(StrokeId);
id_type!(GlyphObjectId);
id_type!(GroupId);
id_type!(ClearEventId);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for RgbaColor {
    fn default() -> Self {
        Self::new(255, 255, 255, 255)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Additive,
}

impl Default for BlendMode {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutlineStyle {
    pub enabled: bool,
    pub width: f32,
    pub color: RgbaColor,
}

impl Default for OutlineStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            width: 0.0,
            color: RgbaColor::new(0, 0, 0, 255),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DropShadowStyle {
    pub enabled: bool,
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub color: RgbaColor,
}

impl Default for DropShadowStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            color: RgbaColor::new(0, 0, 0, 255),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlowStyle {
    pub enabled: bool,
    pub blur_radius: f32,
    pub color: RgbaColor,
}

impl Default for GlowStyle {
    fn default() -> Self {
        Self {
            enabled: false,
            blur_radius: 0.0,
            color: RgbaColor::new(255, 255, 255, 255),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StyleSnapshot {
    pub color: RgbaColor,
    pub fill_color: Option<RgbaColor>,
    pub thickness: f32,
    pub opacity: f32,
    pub outline: OutlineStyle,
    pub drop_shadow: DropShadowStyle,
    pub glow: GlowStyle,
    pub blend_mode: BlendMode,
    pub stabilization_strength: f32,
}

impl Default for StyleSnapshot {
    fn default() -> Self {
        Self {
            color: RgbaColor::new(255, 255, 255, 255),
            fill_color: None,
            thickness: 6.0,
            opacity: 1.0,
            outline: OutlineStyle::default(),
            drop_shadow: DropShadowStyle::default(),
            glow: GlowStyle::default(),
            blend_mode: BlendMode::Normal,
            stabilization_strength: 0.5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StyleDelta {
    pub color: Option<RgbaColor>,
    pub fill_color: Option<Option<RgbaColor>>,
    pub thickness: Option<f32>,
    pub opacity: Option<f32>,
    pub outline: Option<OutlineStyle>,
    pub drop_shadow: Option<DropShadowStyle>,
    pub glow: Option<GlowStyle>,
    pub blend_mode: Option<BlendMode>,
    pub stabilization_strength: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PresetCategory {
    BaseStyle,
    Entrance,
    Clear,
    Combo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetReference {
    pub category: PresetCategory,
    pub preset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PresetBindings {
    pub style: Option<PresetReference>,
    pub entrance: Option<PresetReference>,
    pub clear: Option<PresetReference>,
    pub combo: Option<PresetReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StrokeSample {
    pub position: Point2,
    pub at: MediaTime,
    pub pressure: Option<f32>,
}

impl Default for StrokeSample {
    fn default() -> Self {
        Self {
            position: Point2::default(),
            at: MediaTime::default(),
            pressure: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DerivedStrokePath {
    pub points: Vec<Point2>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stroke {
    pub id: StrokeId,
    pub raw_samples: Vec<StrokeSample>,
    pub stabilized_samples: Vec<StrokeSample>,
    pub derived_path: DerivedStrokePath,
    pub style: StyleSnapshot,
    pub created_at: MediaTime,
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            id: StrokeId::new(""),
            raw_samples: Vec::new(),
            stabilized_samples: Vec::new(),
            derived_path: DerivedStrokePath::default(),
            style: StyleSnapshot::default(),
            created_at: MediaTime::default(),
        }
    }
}

impl Stroke {
    pub fn page_index(&self, clears: &[ClearEvent]) -> usize {
        page_index_for_time(clears, self.created_at)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntranceKind {
    PathTrace,
    Instant,
    Wipe,
    Dissolve,
}

impl Default for EntranceKind {
    fn default() -> Self {
        Self::Instant
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectScope {
    Stroke,
    GlyphObject,
    Group,
    Run,
}

impl Default for EffectScope {
    fn default() -> Self {
        Self::GlyphObject
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectOrder {
    Serial,
    Reverse,
    Parallel,
}

impl Default for EffectOrder {
    fn default() -> Self {
        Self::Parallel
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntranceDurationMode {
    ProportionalToStrokeLength,
    FixedTotalDuration,
}

impl Default for EntranceDurationMode {
    fn default() -> Self {
        Self::FixedTotalDuration
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevealHeadKind {
    SolidHead,
    GlowHead,
    CometTail,
}

impl Default for RevealHeadKind {
    fn default() -> Self {
        Self::SolidHead
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevealHeadColorSource {
    PresetAccent,
    StrokeColor,
    Custom(RgbaColor),
}

impl Default for RevealHeadColorSource {
    fn default() -> Self {
        Self::StrokeColor
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RevealHeadEffect {
    pub kind: RevealHeadKind,
    pub color_source: RevealHeadColorSource,
    pub size_multiplier: f32,
    pub blur_radius: f32,
    pub tail_length: f32,
    pub persistence: f32,
    pub blend_mode: BlendMode,
}

impl Default for RevealHeadEffect {
    fn default() -> Self {
        Self {
            kind: RevealHeadKind::default(),
            color_source: RevealHeadColorSource::default(),
            size_multiplier: 1.0,
            blur_radius: 0.0,
            tail_length: 0.0,
            persistence: 0.0,
            blend_mode: BlendMode::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EntranceBehavior {
    pub kind: EntranceKind,
    pub scope: EffectScope,
    pub order: EffectOrder,
    pub duration_mode: EntranceDurationMode,
    pub duration: MediaDuration,
    pub speed_scalar: f32,
    pub head_effect: Option<RevealHeadEffect>,
}

impl Default for EntranceBehavior {
    fn default() -> Self {
        Self {
            kind: EntranceKind::default(),
            scope: EffectScope::default(),
            order: EffectOrder::default(),
            duration_mode: EntranceDurationMode::default(),
            duration: MediaDuration::default(),
            speed_scalar: 1.0,
            head_effect: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostActionTimingScope {
    DuringReveal,
    AfterStroke,
    AfterGlyphObject,
    AfterGroup,
    AfterRun,
}

impl Default for PostActionTimingScope {
    fn default() -> Self {
        Self::AfterGlyphObject
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PostActionKind {
    NoOp,
    StyleChange {
        style: StyleSnapshot,
    },
    InterpolatedStyleChange {
        style: StyleSnapshot,
        duration: MediaDuration,
    },
    Pulse {
        cycles: u32,
        duration: MediaDuration,
    },
    Blink {
        cycles: u32,
        duration: MediaDuration,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostAction {
    pub timing_scope: PostActionTimingScope,
    pub action: PostActionKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeometryTransform {
    pub translation: Point2,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_degrees: f32,
}

impl Default for GeometryTransform {
    fn default() -> Self {
        Self {
            translation: Point2::default(),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_degrees: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OrderingMetadata {
    pub z_index: i32,
    pub capture_order: u64,
    pub reveal_order: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlyphObject {
    pub id: GlyphObjectId,
    pub stroke_ids: Vec<StrokeId>,
    pub style: StyleSnapshot,
    pub preset_bindings: PresetBindings,
    pub entrance: EntranceBehavior,
    pub post_actions: Vec<PostAction>,
    pub transform: GeometryTransform,
    pub ordering: OrderingMetadata,
    pub created_at: MediaTime,
}

impl Default for GlyphObject {
    fn default() -> Self {
        Self {
            id: GlyphObjectId::new(""),
            stroke_ids: Vec::new(),
            style: StyleSnapshot::default(),
            preset_bindings: PresetBindings::default(),
            entrance: EntranceBehavior::default(),
            post_actions: Vec::new(),
            transform: GeometryTransform::default(),
            ordering: OrderingMetadata::default(),
            created_at: MediaTime::default(),
        }
    }
}

impl GlyphObject {
    pub fn page_index(&self, clears: &[ClearEvent]) -> usize {
        page_index_for_time(clears, self.created_at)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub id: GroupId,
    pub name: Option<String>,
    pub glyph_object_ids: Vec<GlyphObjectId>,
    pub loose_stroke_ids: Vec<StrokeId>,
    pub style_override: Option<StyleSnapshot>,
    pub preset_bindings: PresetBindings,
    pub entrance: Option<EntranceBehavior>,
    pub post_actions: Vec<PostAction>,
    pub created_at: MediaTime,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            id: GroupId::new(""),
            name: None,
            glyph_object_ids: Vec::new(),
            loose_stroke_ids: Vec::new(),
            style_override: None,
            preset_bindings: PresetBindings::default(),
            entrance: None,
            post_actions: Vec::new(),
            created_at: MediaTime::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AnnotationProject {
    pub strokes: Vec<Stroke>,
    pub glyph_objects: Vec<GlyphObject>,
    pub groups: Vec<Group>,
    pub clear_events: Vec<ClearEvent>,
}

impl AnnotationProject {
    pub fn stroke_index(&self, stroke_id: &StrokeId) -> Option<usize> {
        self.strokes
            .iter()
            .position(|stroke| stroke.id == *stroke_id)
    }

    pub fn glyph_object_index(&self, object_id: &GlyphObjectId) -> Option<usize> {
        self.glyph_objects
            .iter()
            .position(|object| object.id == *object_id)
    }

    pub fn group_index(&self, group_id: &GroupId) -> Option<usize> {
        self.groups.iter().position(|group| group.id == *group_id)
    }

    pub fn clear_event_index(&self, clear_event_id: &ClearEventId) -> Option<usize> {
        self.clear_events
            .iter()
            .position(|clear_event| clear_event.id == *clear_event_id)
    }
}
