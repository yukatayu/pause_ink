use std::collections::{HashMap, HashSet};

use pauseink_domain::{
    AnnotationProject, BlendMode, ClearKind, ColorMode, ColorStop, DerivedStrokePath, EffectOrder,
    EffectScope, GeometryTransform, GlyphObject, GradientRepeat, GradientSpace,
    LinearGradientStyle, MediaTime, Point2, PostAction, PostActionKind, PostActionTimingScope,
    RevealHeadColorSource, RevealHeadEffect, RevealHeadKind, RgbaColor, Stroke, StrokeSample,
    StyleSnapshot, TimeBase,
};
use tiny_skia::{
    BlendMode as SkBlendMode, GradientStop as SkGradientStop, LinearGradient, Paint, PathBuilder,
    Pixmap, Point as SkPoint, Shader, SpreadMode, Stroke as SkStroke, Transform,
};

#[derive(Debug, Clone, PartialEq)]
pub struct RenderRequest<'a> {
    pub project: &'a AnnotationProject,
    pub time: MediaTime,
    pub preview_force_visible_batch: Option<MediaTime>,
    pub width: u32,
    pub height: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub background: RgbaColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedOverlay {
    pub width: u32,
    pub height: u32,
    pub rgba_pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VisibilityState {
    alpha: f32,
    path_fraction: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RenderScale {
    x: f32,
    y: f32,
    stroke: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RectBounds {
    min: Point2,
    max: Point2,
}

impl RectBounds {
    fn center(self) -> Point2 {
        Point2 {
            x: (self.min.x + self.max.x) * 0.5,
            y: (self.min.y + self.max.y) * 0.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RecentInkAccentState {
    front_fraction: f32,
    tail_length: f32,
    fade: f32,
    color: RgbaColor,
    kind: RevealHeadKind,
    size_multiplier: f32,
    blur_radius: f32,
    blend_mode: BlendMode,
}

#[derive(Debug, Clone, PartialEq)]
struct PostActionEvaluation {
    style: StyleSnapshot,
    alpha_multiplier: f32,
}

const DEFAULT_ENTRANCE_DURATION_SECONDS: f64 = 0.6;
const PROPORTIONAL_REFERENCE_LENGTH_PX: f64 = 600.0;

impl VisibilityState {
    const HIDDEN: Self = Self {
        alpha: 0.0,
        path_fraction: 0.0,
    };

    const FULLY_VISIBLE: Self = Self {
        alpha: 1.0,
        path_fraction: 1.0,
    };

    fn is_visible(self) -> bool {
        self.alpha > 0.0 && self.path_fraction > 0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderError {
    InvalidCanvasSize,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCanvasSize => f.write_str("canvas size must be non-zero"),
        }
    }
}

impl std::error::Error for RenderError {}

pub fn derive_stroke_layers(
    raw_samples: &[StrokeSample],
    stabilization_strength: u8,
) -> (Vec<StrokeSample>, DerivedStrokePath) {
    if raw_samples.len() <= 2 {
        return (
            raw_samples.to_vec(),
            DerivedStrokePath {
                points: raw_samples.iter().map(|sample| sample.position).collect(),
            },
        );
    }

    let smoothing = (stabilization_strength as f32 / 100.0).clamp(0.0, 1.0) * 0.7;
    let corner_guard_threshold = 0.65 - (stabilization_strength as f32 / 100.0) * 0.25;

    let mut stabilized = Vec::with_capacity(raw_samples.len());
    stabilized.push(raw_samples[0].clone());

    for index in 1..raw_samples.len() {
        let current = &raw_samples[index];
        let previous = stabilized.last().expect("first sample already pushed");
        let position = if is_corner(raw_samples, index, corner_guard_threshold) {
            current.position
        } else {
            Point2 {
                x: previous.position.x
                    + (current.position.x - previous.position.x) * (1.0 - smoothing),
                y: previous.position.y
                    + (current.position.y - previous.position.y) * (1.0 - smoothing),
            }
        };

        stabilized.push(StrokeSample {
            position,
            at: current.at,
            pressure: current.pressure,
        });
    }

    (
        stabilized.clone(),
        DerivedStrokePath {
            points: stabilized.iter().map(|sample| sample.position).collect(),
        },
    )
}

pub fn render_overlay_rgba(request: &RenderRequest<'_>) -> Result<RenderedOverlay, RenderError> {
    if request.width == 0
        || request.height == 0
        || request.source_width == 0
        || request.source_height == 0
    {
        return Err(RenderError::InvalidCanvasSize);
    }
    let render_scale = RenderScale {
        x: request.width as f32 / request.source_width as f32,
        y: request.height as f32 / request.source_height as f32,
        stroke: (request.width as f32 / request.source_width as f32)
            .min(request.height as f32 / request.source_height as f32),
    };

    let mut pixmap =
        Pixmap::new(request.width, request.height).ok_or(RenderError::InvalidCanvasSize)?;
    pixmap.fill(color_to_tiny(request.background, 1.0));
    let outer_layers = [
        StrokeRenderLayer::DropShadow,
        StrokeRenderLayer::Glow,
        StrokeRenderLayer::Outline,
    ];
    let body_layers = [
        StrokeRenderLayer::Base,
        StrokeRenderLayer::HeadHalo,
        StrokeRenderLayer::HeadCore,
    ];

    let mut objects = request.project.glyph_objects.iter().collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        left.ordering
            .z_index
            .cmp(&right.ordering.z_index)
            .then_with(|| {
                left.ordering
                    .capture_order
                    .cmp(&right.ordering.capture_order)
            })
            .then_with(|| left.id.0.cmp(&right.id.0))
    });

    let strokes_by_id = request
        .project
        .strokes
        .iter()
        .map(|stroke| (stroke.id.0.as_str(), stroke))
        .collect::<HashMap<_, _>>();
    let referenced_strokes = objects
        .iter()
        .flat_map(|object| {
            object
                .stroke_ids
                .iter()
                .map(|stroke_id| stroke_id.0.clone())
        })
        .collect::<HashSet<_>>();

    for layer in outer_layers {
        for object in &objects {
            for stroke_id in &object.stroke_ids {
                let Some(stroke) = strokes_by_id.get(stroke_id.0.as_str()).copied() else {
                    continue;
                };

                let visibility = evaluate_visibility(request, Some(object), stroke, request.time);
                if !visibility.is_visible() {
                    continue;
                }
                let post_action =
                    evaluate_post_actions(request.project, object, stroke, request.time);
                let mut effective_visibility = visibility;
                effective_visibility.alpha *= post_action.alpha_multiplier;
                if !effective_visibility.is_visible() {
                    continue;
                }

                render_stroke_layer(
                    &mut pixmap,
                    request,
                    Some(object),
                    stroke,
                    &post_action.style,
                    &object.transform,
                    effective_visibility,
                    recent_ink_accent_state(request, object, stroke, &post_action.style),
                    render_scale,
                    layer,
                );
            }
        }
    }

    for layer in outer_layers {
        for stroke in request
            .project
            .strokes
            .iter()
            .filter(|stroke| !referenced_strokes.contains(&stroke.id.0))
        {
            let visibility = evaluate_visibility(request, None, stroke, request.time);
            if !visibility.is_visible() {
                continue;
            }

            render_stroke_layer(
                &mut pixmap,
                request,
                None,
                stroke,
                &stroke.style,
                &GeometryTransform::default(),
                visibility,
                None,
                render_scale,
                layer,
            );
        }
    }

    for object in &objects {
        for stroke_id in &object.stroke_ids {
            let Some(stroke) = strokes_by_id.get(stroke_id.0.as_str()).copied() else {
                continue;
            };

            let visibility = evaluate_visibility(request, Some(object), stroke, request.time);
            if !visibility.is_visible() {
                continue;
            }
            let post_action = evaluate_post_actions(request.project, object, stroke, request.time);
            let mut effective_visibility = visibility;
            effective_visibility.alpha *= post_action.alpha_multiplier;
            if !effective_visibility.is_visible() {
                continue;
            }

            let accent = recent_ink_accent_state(request, object, stroke, &post_action.style);
            for layer in body_layers {
                render_stroke_layer(
                    &mut pixmap,
                    request,
                    Some(object),
                    stroke,
                    &post_action.style,
                    &object.transform,
                    effective_visibility,
                    accent,
                    render_scale,
                    layer,
                );
            }
        }
    }

    for stroke in request
        .project
        .strokes
        .iter()
        .filter(|stroke| !referenced_strokes.contains(&stroke.id.0))
    {
        let visibility = evaluate_visibility(request, None, stroke, request.time);
        if !visibility.is_visible() {
            continue;
        }

        for layer in body_layers {
            render_stroke_layer(
                &mut pixmap,
                request,
                None,
                stroke,
                &stroke.style,
                &GeometryTransform::default(),
                visibility,
                None,
                render_scale,
                layer,
            );
        }
    }

    Ok(RenderedOverlay {
        width: request.width,
        height: request.height,
        rgba_pixels: pixmap.data().to_vec(),
    })
}

fn evaluate_visibility(
    request: &RenderRequest<'_>,
    object: Option<&GlyphObject>,
    stroke: &Stroke,
    time: MediaTime,
) -> VisibilityState {
    if time < stroke.created_at {
        return VisibilityState::HIDDEN;
    }

    let entrance = evaluate_entrance(request, object, time);
    if !entrance.is_visible() {
        return entrance;
    }

    let clear = evaluate_clear(request.project, object, stroke, time);
    VisibilityState {
        alpha: (entrance.alpha * clear.alpha).clamp(0.0, 1.0),
        path_fraction: (entrance.path_fraction * clear.path_fraction).clamp(0.0, 1.0),
    }
}

fn evaluate_post_actions(
    project: &AnnotationProject,
    object: &GlyphObject,
    stroke: &Stroke,
    time: MediaTime,
) -> PostActionEvaluation {
    if object.post_actions.is_empty() {
        return PostActionEvaluation {
            style: object.style.clone(),
            alpha_multiplier: 1.0,
        };
    }

    let (reveal_start_seconds, reveal_end_seconds) = object_reveal_window_seconds(project, object);
    let current_seconds = media_time_seconds(time);
    let mut style = object.style.clone();
    let mut alpha_multiplier = 1.0_f32;

    for post_action in &object.post_actions {
        match post_action.timing_scope {
            PostActionTimingScope::DuringReveal => {
                if current_seconds < reveal_start_seconds || current_seconds > reveal_end_seconds {
                    continue;
                }
                apply_post_action_to_effective_style(
                    &mut style,
                    &mut alpha_multiplier,
                    post_action,
                    reveal_start_seconds,
                    Some(reveal_end_seconds),
                    current_seconds,
                );
            }
            PostActionTimingScope::AfterStroke => {
                let (_, stroke_end_seconds) = stroke_reveal_window_seconds(project, object, stroke);
                if current_seconds < stroke_end_seconds {
                    continue;
                }
                apply_post_action_to_effective_style(
                    &mut style,
                    &mut alpha_multiplier,
                    post_action,
                    stroke_end_seconds,
                    None,
                    current_seconds,
                );
            }
            PostActionTimingScope::AfterGlyphObject => {
                if current_seconds < reveal_end_seconds {
                    continue;
                }
                apply_post_action_to_effective_style(
                    &mut style,
                    &mut alpha_multiplier,
                    post_action,
                    reveal_end_seconds,
                    None,
                    current_seconds,
                );
            }
            PostActionTimingScope::AfterGroup => {
                let group_end_seconds = group_reveal_end_seconds(project, object);
                if current_seconds < group_end_seconds {
                    continue;
                }
                apply_post_action_to_effective_style(
                    &mut style,
                    &mut alpha_multiplier,
                    post_action,
                    group_end_seconds,
                    None,
                    current_seconds,
                );
            }
            PostActionTimingScope::AfterRun => {
                let run_end_seconds = run_reveal_end_seconds(project, object);
                if current_seconds < run_end_seconds {
                    continue;
                }
                apply_post_action_to_effective_style(
                    &mut style,
                    &mut alpha_multiplier,
                    post_action,
                    run_end_seconds,
                    None,
                    current_seconds,
                );
            }
        }
    }

    PostActionEvaluation {
        style,
        alpha_multiplier: alpha_multiplier.clamp(0.0, 1.0),
    }
}

fn evaluate_entrance(
    request: &RenderRequest<'_>,
    object: Option<&GlyphObject>,
    time: MediaTime,
) -> VisibilityState {
    let Some(object) = object else {
        return VisibilityState::FULLY_VISIBLE;
    };

    if request
        .preview_force_visible_batch
        .is_some_and(|batch_time| batch_time == object.created_at)
    {
        return VisibilityState::FULLY_VISIBLE;
    }

    let effective_start_seconds = effective_entrance_start_seconds(request.project, object);
    let current_seconds = media_time_seconds(time);
    if current_seconds < effective_start_seconds {
        return VisibilityState::HIDDEN;
    }

    let duration_seconds = entrance_duration_seconds(request.project, object);
    let progress = normalized_progress_from_seconds(
        effective_start_seconds,
        current_seconds,
        duration_seconds,
    );

    match object.entrance.kind {
        pauseink_domain::EntranceKind::Instant => VisibilityState::FULLY_VISIBLE,
        pauseink_domain::EntranceKind::PathTrace | pauseink_domain::EntranceKind::Wipe => {
            VisibilityState {
                alpha: 1.0,
                path_fraction: progress,
            }
        }
        pauseink_domain::EntranceKind::Dissolve => VisibilityState {
            alpha: progress,
            path_fraction: 1.0,
        },
    }
}

fn effective_entrance_start_seconds(project: &AnnotationProject, object: &GlyphObject) -> f64 {
    let object_page = object.page_index(&project.clear_events);
    let batch_anchor_seconds = media_time_seconds(object.created_at);
    if matches!(object.entrance.kind, pauseink_domain::EntranceKind::Instant) {
        return batch_anchor_seconds;
    }

    let mut batch_objects = project
        .glyph_objects
        .iter()
        .filter(|candidate| {
            candidate.page_index(&project.clear_events) == object_page
                && candidate.created_at == object.created_at
        })
        .collect::<Vec<_>>();
    batch_objects.sort_by(|left, right| {
        left.ordering
            .reveal_order
            .cmp(&right.ordering.reveal_order)
            .then(left.created_at.cmp(&right.created_at))
            .then(left.id.0.cmp(&right.id.0))
    });

    let mut batch_elapsed_seconds = 0.0;
    for candidate in batch_objects {
        let effective_start_seconds = batch_anchor_seconds + batch_elapsed_seconds;
        if candidate.id == object.id {
            return effective_start_seconds;
        }
        if !matches!(
            candidate.entrance.kind,
            pauseink_domain::EntranceKind::Instant
        ) {
            batch_elapsed_seconds += entrance_duration_seconds(project, candidate);
        }
    }

    batch_anchor_seconds
}

fn object_reveal_window_seconds(project: &AnnotationProject, object: &GlyphObject) -> (f64, f64) {
    let start_seconds = effective_entrance_start_seconds(project, object);
    let end_seconds = match object.entrance.kind {
        pauseink_domain::EntranceKind::Instant => start_seconds,
        _ => start_seconds + entrance_duration_seconds(project, object),
    };
    (start_seconds, end_seconds)
}

fn stroke_reveal_window_seconds(
    project: &AnnotationProject,
    object: &GlyphObject,
    stroke: &Stroke,
) -> (f64, f64) {
    let (object_start_seconds, object_end_seconds) = object_reveal_window_seconds(project, object);
    if !matches!(object.entrance.scope, EffectScope::Stroke)
        || matches!(object.entrance.kind, pauseink_domain::EntranceKind::Instant)
        || object.stroke_ids.len() <= 1
    {
        return (object_start_seconds, object_end_seconds);
    }

    if matches!(object.entrance.order, EffectOrder::Parallel) {
        return (object_start_seconds, object_end_seconds);
    }

    let total_duration_seconds = (object_end_seconds - object_start_seconds).max(0.0);
    if total_duration_seconds <= f64::EPSILON {
        return (object_start_seconds, object_end_seconds);
    }

    let mut ordered_strokes = object
        .stroke_ids
        .iter()
        .filter_map(|stroke_id| {
            project
                .strokes
                .iter()
                .find(|candidate| candidate.id == *stroke_id)
        })
        .collect::<Vec<_>>();
    if matches!(object.entrance.order, EffectOrder::Reverse) {
        ordered_strokes.reverse();
    }

    let stroke_weights = ordered_strokes
        .iter()
        .map(|candidate| stroke_duration_weight(project, object, candidate))
        .collect::<Vec<_>>();
    let total_weight = stroke_weights
        .iter()
        .copied()
        .sum::<f64>()
        .max(f64::EPSILON);

    let mut elapsed = 0.0_f64;
    for (candidate, weight) in ordered_strokes.iter().zip(stroke_weights.iter().copied()) {
        let fraction = (weight / total_weight).clamp(0.0, 1.0);
        let segment_duration = total_duration_seconds * fraction;
        let start_seconds = object_start_seconds + elapsed;
        let end_seconds = start_seconds + segment_duration;
        if candidate.id == stroke.id {
            return (start_seconds, end_seconds);
        }
        elapsed += segment_duration;
    }

    (object_start_seconds, object_end_seconds)
}

fn stroke_duration_weight(
    _project: &AnnotationProject,
    object: &GlyphObject,
    stroke: &Stroke,
) -> f64 {
    match object.entrance.duration_mode {
        pauseink_domain::EntranceDurationMode::FixedTotalDuration => 1.0,
        pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength => {
            let points = transformed_visible_points(stroke, &object.transform, 1.0);
            polyline_length(&points).max(1.0) as f64
        }
    }
}

fn group_reveal_end_seconds(project: &AnnotationProject, object: &GlyphObject) -> f64 {
    let object_page = object.page_index(&project.clear_events);
    let Some(group) = project
        .groups
        .iter()
        .find(|group| group.glyph_object_ids.contains(&object.id))
    else {
        return object_reveal_window_seconds(project, object).1;
    };

    group
        .glyph_object_ids
        .iter()
        .filter_map(|object_id| {
            project
                .glyph_objects
                .iter()
                .find(|candidate| candidate.id == *object_id)
        })
        .filter(|candidate| candidate.page_index(&project.clear_events) == object_page)
        .map(|candidate| object_reveal_window_seconds(project, candidate).1)
        .fold(
            object_reveal_window_seconds(project, object).1,
            |latest, candidate_end| latest.max(candidate_end),
        )
}

fn run_reveal_end_seconds(project: &AnnotationProject, object: &GlyphObject) -> f64 {
    let object_page = object.page_index(&project.clear_events);
    project
        .glyph_objects
        .iter()
        .filter(|candidate| {
            candidate.page_index(&project.clear_events) == object_page
                && candidate.created_at == object.created_at
        })
        .map(|candidate| object_reveal_window_seconds(project, candidate).1)
        .fold(
            object_reveal_window_seconds(project, object).1,
            |latest, candidate_end| latest.max(candidate_end),
        )
}

fn entrance_duration_seconds(project: &AnnotationProject, object: &GlyphObject) -> f64 {
    let configured_duration_seconds = media_duration_seconds(
        object.entrance.duration.ticks,
        object.entrance.duration.time_base,
    );
    let base_duration_seconds = if configured_duration_seconds <= f64::EPSILON {
        DEFAULT_ENTRANCE_DURATION_SECONDS
    } else {
        configured_duration_seconds
    };
    let speed_scalar = object.entrance.speed_scalar.max(0.05) as f64;

    match object.entrance.duration_mode {
        pauseink_domain::EntranceDurationMode::FixedTotalDuration => {
            base_duration_seconds / speed_scalar
        }
        pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength => {
            let total_length = object_total_length(project, object) as f64;
            let length_ratio = if total_length <= f64::EPSILON {
                1.0
            } else {
                total_length / PROPORTIONAL_REFERENCE_LENGTH_PX
            };
            (base_duration_seconds * length_ratio.max(0.05)) / speed_scalar
        }
    }
}

fn object_total_length(project: &AnnotationProject, object: &GlyphObject) -> f32 {
    object
        .stroke_ids
        .iter()
        .filter_map(|stroke_id| {
            project
                .strokes
                .iter()
                .find(|stroke| &stroke.id == stroke_id)
        })
        .map(|stroke| {
            let points = transformed_visible_points(stroke, &object.transform, 1.0);
            polyline_length(&points)
        })
        .sum()
}

fn evaluate_clear(
    project: &AnnotationProject,
    object: Option<&GlyphObject>,
    stroke: &Stroke,
    time: MediaTime,
) -> VisibilityState {
    let anchor = object
        .map(|item| item.created_at)
        .unwrap_or(stroke.created_at);
    let Some(clear_event) = project
        .clear_events
        .iter()
        .filter(|clear| clear.time > anchor)
        .min_by(|left, right| left.time.cmp(&right.time))
    else {
        return VisibilityState::FULLY_VISIBLE;
    };

    if time < clear_event.time {
        return VisibilityState::FULLY_VISIBLE;
    }

    let duration_seconds =
        media_duration_seconds(clear_event.duration.ticks, clear_event.duration.time_base);
    if duration_seconds <= f64::EPSILON {
        return match clear_event.kind {
            ClearKind::WipeOut => VisibilityState {
                alpha: 1.0,
                path_fraction: 0.0,
            },
            ClearKind::DissolveOut => VisibilityState {
                alpha: 0.0,
                path_fraction: 1.0,
            },
            _ => VisibilityState::HIDDEN,
        };
    }

    let progress = normalized_progress(clear_event.time, time, duration_seconds);
    match clear_event.kind {
        ClearKind::Instant | ClearKind::Ordered | ClearKind::ReverseOrdered => {
            if progress >= 1.0 {
                VisibilityState::HIDDEN
            } else {
                VisibilityState::FULLY_VISIBLE
            }
        }
        ClearKind::WipeOut => VisibilityState {
            alpha: 1.0,
            path_fraction: (1.0 - progress).clamp(0.0, 1.0),
        },
        ClearKind::DissolveOut => VisibilityState {
            alpha: (1.0 - progress).clamp(0.0, 1.0),
            path_fraction: 1.0,
        },
    }
}

fn normalized_progress(start: MediaTime, current: MediaTime, duration_seconds: f64) -> f32 {
    normalized_progress_from_seconds(
        media_time_seconds(start),
        media_time_seconds(current),
        duration_seconds,
    )
}

fn normalized_progress_from_seconds(
    start_seconds: f64,
    current_seconds: f64,
    duration_seconds: f64,
) -> f32 {
    if duration_seconds <= f64::EPSILON {
        return 1.0;
    }

    let elapsed = (current_seconds - start_seconds).max(0.0);
    (elapsed / duration_seconds).clamp(0.0, 1.0) as f32
}

fn media_time_seconds(time: MediaTime) -> f64 {
    time.ticks as f64 * time.time_base.numerator as f64 / time.time_base.denominator as f64
}

fn media_duration_seconds(ticks: i64, time_base: TimeBase) -> f64 {
    ticks as f64 * time_base.numerator as f64 / time_base.denominator as f64
}

fn apply_post_action_to_effective_style(
    style: &mut StyleSnapshot,
    alpha_multiplier: &mut f32,
    post_action: &PostAction,
    anchor_seconds: f64,
    hard_end_seconds: Option<f64>,
    current_seconds: f64,
) {
    let elapsed_seconds = (current_seconds - anchor_seconds).max(0.0);
    match &post_action.action {
        PostActionKind::NoOp => {}
        PostActionKind::StyleChange { style: target } => {
            *style = target.clone();
        }
        PostActionKind::InterpolatedStyleChange {
            style: target,
            duration,
        } => {
            let duration_seconds = media_duration_seconds(duration.ticks, duration.time_base);
            let effective_duration = hard_end_seconds
                .map(|hard_end| (hard_end - anchor_seconds).max(0.0))
                .map(|window| {
                    if duration_seconds <= f64::EPSILON {
                        window
                    } else {
                        window.min(duration_seconds)
                    }
                })
                .unwrap_or(duration_seconds);
            let progress = if effective_duration <= f64::EPSILON {
                1.0
            } else {
                (elapsed_seconds / effective_duration).clamp(0.0, 1.0) as f32
            };
            *style = interpolate_style_snapshot(style, target, progress);
        }
        PostActionKind::Pulse { cycles, duration } => {
            let duration_seconds = media_duration_seconds(duration.ticks, duration.time_base);
            let Some(progress) = action_progress(
                elapsed_seconds,
                duration_seconds,
                hard_end_seconds.map(|hard_end| (hard_end - anchor_seconds).max(0.0)),
            ) else {
                return;
            };
            let cycles = (*cycles).max(1) as f32;
            let wave = (progress * std::f32::consts::TAU * cycles).sin().abs();
            let intensity = wave.powf(1.35);
            style.opacity = (style.opacity * (1.0 + intensity * 0.18)).clamp(0.0, 1.0);
            style.glow.enabled = true;
            style.glow.blur_radius = style
                .glow
                .blur_radius
                .max(style.thickness * (0.45 + intensity * 1.6));
            style.glow.color = mix_colors(
                representative_ink_color(style),
                RgbaColor::new(255, 255, 255, 255),
                0.18 + intensity * 0.28,
            );
        }
        PostActionKind::Blink { cycles, duration } => {
            let duration_seconds = media_duration_seconds(duration.ticks, duration.time_base);
            let Some(progress) = action_progress(
                elapsed_seconds,
                duration_seconds,
                hard_end_seconds.map(|hard_end| (hard_end - anchor_seconds).max(0.0)),
            ) else {
                return;
            };
            let cycles = (*cycles).max(1) as f32;
            let phase = ((progress * cycles * 2.0).floor() as i32) % 2;
            if phase == 1 {
                *alpha_multiplier *= 0.0;
            }
        }
    }
}

fn action_progress(
    elapsed_seconds: f64,
    duration_seconds: f64,
    hard_limit_seconds: Option<f64>,
) -> Option<f32> {
    let effective_duration = hard_limit_seconds
        .map(|window| {
            if duration_seconds <= f64::EPSILON {
                window
            } else {
                window.min(duration_seconds)
            }
        })
        .unwrap_or(duration_seconds);
    if effective_duration <= f64::EPSILON {
        return Some(1.0);
    }
    if elapsed_seconds > effective_duration {
        return None;
    }
    Some((elapsed_seconds / effective_duration).clamp(0.0, 1.0) as f32)
}

fn interpolate_style_snapshot(
    from: &StyleSnapshot,
    to: &StyleSnapshot,
    progress: f32,
) -> StyleSnapshot {
    let t = progress.clamp(0.0, 1.0);
    let mut blended = from.clone();
    blended.color = mix_colors(from.color, to.color, t);
    blended.thickness = lerp_f32(from.thickness, to.thickness, t);
    blended.opacity = lerp_f32(from.opacity, to.opacity, t);
    blended.stabilization_strength =
        lerp_f32(from.stabilization_strength, to.stabilization_strength, t);
    blended.outline.width = lerp_f32(from.outline.width, to.outline.width, t);
    blended.outline.color = mix_colors(from.outline.color, to.outline.color, t);
    blended.outline.enabled = if t >= 1.0 {
        to.outline.enabled
    } else {
        from.outline.enabled
    };
    blended.drop_shadow.offset_x = lerp_f32(from.drop_shadow.offset_x, to.drop_shadow.offset_x, t);
    blended.drop_shadow.offset_y = lerp_f32(from.drop_shadow.offset_y, to.drop_shadow.offset_y, t);
    blended.drop_shadow.blur_radius =
        lerp_f32(from.drop_shadow.blur_radius, to.drop_shadow.blur_radius, t);
    blended.drop_shadow.color = mix_colors(from.drop_shadow.color, to.drop_shadow.color, t);
    blended.drop_shadow.enabled = if t >= 1.0 {
        to.drop_shadow.enabled
    } else {
        from.drop_shadow.enabled
    };
    blended.glow.blur_radius = lerp_f32(from.glow.blur_radius, to.glow.blur_radius, t);
    blended.glow.color = mix_colors(from.glow.color, to.glow.color, t);
    blended.glow.enabled = if t >= 1.0 {
        to.glow.enabled
    } else {
        from.glow.enabled
    };
    if t >= 1.0 {
        blended.color_mode = to.color_mode;
        blended.gradient = to.gradient.clone();
        blended.blend_mode = to.blend_mode;
    }
    blended
}

fn lerp_f32(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn mix_colors(from: RgbaColor, to: RgbaColor, t: f32) -> RgbaColor {
    RgbaColor::new(
        lerp_f32(from.r as f32, to.r as f32, t)
            .round()
            .clamp(0.0, 255.0) as u8,
        lerp_f32(from.g as f32, to.g as f32, t)
            .round()
            .clamp(0.0, 255.0) as u8,
        lerp_f32(from.b as f32, to.b as f32, t)
            .round()
            .clamp(0.0, 255.0) as u8,
        lerp_f32(from.a as f32, to.a as f32, t)
            .round()
            .clamp(0.0, 255.0) as u8,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrokeRenderLayer {
    DropShadow,
    Glow,
    Outline,
    Base,
    HeadHalo,
    HeadCore,
}

fn render_stroke_layer(
    pixmap: &mut Pixmap,
    request: &RenderRequest<'_>,
    object: Option<&GlyphObject>,
    stroke: &Stroke,
    style: &StyleSnapshot,
    transform: &GeometryTransform,
    visibility: VisibilityState,
    accent: Option<RecentInkAccentState>,
    render_scale: RenderScale,
    layer: StrokeRenderLayer,
) {
    let full_points = transformed_visible_points(stroke, transform, 1.0);
    let points = partial_polyline_range(&full_points, 0.0, visibility.path_fraction)
        .into_iter()
        .map(|point| scale_point(point, render_scale))
        .collect::<Vec<_>>();
    if points.len() < 2 {
        return;
    }

    let Some(path) = build_polyline_path(&points) else {
        return;
    };

    match layer {
        StrokeRenderLayer::DropShadow => {
            if style.drop_shadow.enabled {
                let mut shadow_points = points.clone();
                for point in &mut shadow_points {
                    point.x += style.drop_shadow.offset_x * render_scale.x;
                    point.y += style.drop_shadow.offset_y * render_scale.y;
                }
                if let Some(shadow_path) = build_polyline_path(&shadow_points) {
                    draw_stroked_path(
                        pixmap,
                        &shadow_path,
                        (style.thickness + style.drop_shadow.blur_radius.max(0.0))
                            * render_scale.stroke,
                        style.drop_shadow.color,
                        style.opacity * 0.45 * visibility.alpha,
                        style.blend_mode,
                    );
                }
            }
        }
        StrokeRenderLayer::Glow => {
            if style.glow.enabled {
                draw_stroked_path(
                    pixmap,
                    &path,
                    (style.thickness + style.glow.blur_radius.max(0.0) * 1.8) * render_scale.stroke,
                    style.glow.color,
                    style.opacity * 0.35 * visibility.alpha,
                    BlendMode::Screen,
                );
            }
        }
        StrokeRenderLayer::Outline => {
            if style.outline.enabled && style.outline.width > 0.0 {
                draw_stroked_path(
                    pixmap,
                    &path,
                    (style.thickness + style.outline.width * 2.0) * render_scale.stroke,
                    style.outline.color,
                    style.opacity * visibility.alpha,
                    style.blend_mode,
                );
            }
        }
        StrokeRenderLayer::Base => {
            if let Some(gradient) = style
                .gradient
                .as_ref()
                .filter(|_| matches!(style.color_mode, ColorMode::LinearGradient))
            {
                let stroke_bounds = bounds_from_points(&full_points);
                let object_bounds = object
                    .and_then(|object| object_bounds_in_source_space(request.project, object));
                let canvas_bounds = RectBounds {
                    min: Point2 { x: 0.0, y: 0.0 },
                    max: Point2 {
                        x: request.source_width as f32,
                        y: request.source_height as f32,
                    },
                };
                if let Some(shader) = gradient_shader(
                    style,
                    gradient,
                    stroke_bounds,
                    object_bounds,
                    canvas_bounds,
                    style.opacity * visibility.alpha,
                    render_scale,
                ) {
                    draw_stroked_path_with_shader(
                        pixmap,
                        &path,
                        (style.thickness * render_scale.stroke).max(1.0),
                        shader,
                        style.blend_mode,
                    );
                } else {
                    draw_stroked_path(
                        pixmap,
                        &path,
                        (style.thickness * render_scale.stroke).max(1.0),
                        representative_ink_color(style),
                        style.opacity * visibility.alpha,
                        style.blend_mode,
                    );
                }
            } else {
                draw_stroked_path(
                    pixmap,
                    &path,
                    (style.thickness * render_scale.stroke).max(1.0),
                    representative_ink_color(style),
                    style.opacity * visibility.alpha,
                    style.blend_mode,
                );
            }
        }
        StrokeRenderLayer::HeadHalo => {
            render_recent_ink_accent_layer(
                pixmap,
                stroke,
                style,
                transform,
                accent,
                render_scale,
                RecentInkAccentLayer::Halo,
            );
        }
        StrokeRenderLayer::HeadCore => {
            render_recent_ink_accent_layer(
                pixmap,
                stroke,
                style,
                transform,
                accent,
                render_scale,
                RecentInkAccentLayer::Core,
            );
        }
    }
}

fn transformed_visible_points(
    stroke: &Stroke,
    transform: &GeometryTransform,
    path_fraction: f32,
) -> Vec<Point2> {
    transformed_visible_points_range(stroke, transform, 0.0, path_fraction)
}

fn transformed_visible_points_range(
    stroke: &Stroke,
    transform: &GeometryTransform,
    start_fraction: f32,
    end_fraction: f32,
) -> Vec<Point2> {
    partial_polyline_range(&stroke_source_points(stroke), start_fraction, end_fraction)
        .into_iter()
        .map(|point| apply_transform(point, transform))
        .collect()
}

fn stroke_source_points(stroke: &Stroke) -> Vec<Point2> {
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
}

fn apply_transform(point: Point2, transform: &GeometryTransform) -> Point2 {
    let scaled_x = point.x * transform.scale_x;
    let scaled_y = point.y * transform.scale_y;
    let radians = transform.rotation_degrees.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();

    Point2 {
        x: scaled_x * cos - scaled_y * sin + transform.translation.x,
        y: scaled_x * sin + scaled_y * cos + transform.translation.y,
    }
}

fn bounds_from_points(points: &[Point2]) -> Option<RectBounds> {
    let mut iter = points.iter().copied();
    let first = iter.next()?;
    let mut min = first;
    let mut max = first;
    for point in iter {
        min.x = min.x.min(point.x);
        min.y = min.y.min(point.y);
        max.x = max.x.max(point.x);
        max.y = max.y.max(point.y);
    }
    Some(RectBounds { min, max })
}

fn object_bounds_in_source_space(
    project: &AnnotationProject,
    object: &GlyphObject,
) -> Option<RectBounds> {
    let mut points = Vec::new();
    for stroke_id in &object.stroke_ids {
        let Some(stroke) = project
            .strokes
            .iter()
            .find(|stroke| stroke.id == *stroke_id)
        else {
            continue;
        };
        points.extend(transformed_visible_points(stroke, &object.transform, 1.0));
    }
    bounds_from_points(&points)
}

fn representative_ink_color(style: &StyleSnapshot) -> RgbaColor {
    if !matches!(style.color_mode, ColorMode::LinearGradient) {
        return style.color;
    }

    style
        .gradient
        .as_ref()
        .map(sample_representative_gradient_color)
        .unwrap_or(style.color)
}

fn sample_representative_gradient_color(gradient: &LinearGradientStyle) -> RgbaColor {
    let stops = normalized_color_stops(gradient, RgbaColor::default());
    sample_color_stops(&stops, 0.5)
}

fn normalized_color_stops(gradient: &LinearGradientStyle, fallback: RgbaColor) -> Vec<ColorStop> {
    let mut stops = gradient
        .stops
        .iter()
        .map(|stop| ColorStop {
            position: stop.position.clamp(0.0, 1.0),
            color: stop.color,
        })
        .collect::<Vec<_>>();
    if stops.is_empty() {
        return vec![
            ColorStop {
                position: 0.0,
                color: fallback,
            },
            ColorStop {
                position: 1.0,
                color: fallback,
            },
        ];
    }
    stops.sort_by(|left, right| {
        left.position
            .partial_cmp(&right.position)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if stops.first().is_some_and(|stop| stop.position > 0.0) {
        let color = stops[0].color;
        stops.insert(
            0,
            ColorStop {
                position: 0.0,
                color,
            },
        );
    }
    if stops.last().is_some_and(|stop| stop.position < 1.0) {
        let color = stops.last().map(|stop| stop.color).unwrap_or(fallback);
        stops.push(ColorStop {
            position: 1.0,
            color,
        });
    }
    if stops.len() == 1 {
        stops.push(ColorStop {
            position: 1.0,
            color: stops[0].color,
        });
    }
    stops
}

fn sample_color_stops(stops: &[ColorStop], position: f32) -> RgbaColor {
    let position = position.clamp(0.0, 1.0);
    let Some(first) = stops.first() else {
        return RgbaColor::default();
    };
    if position <= first.position {
        return first.color;
    }
    for segment in stops.windows(2) {
        let left = &segment[0];
        let right = &segment[1];
        if position > right.position {
            continue;
        }
        let span = (right.position - left.position).max(f32::EPSILON);
        let t = ((position - left.position) / span).clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| -> u8 {
            ((a as f32) + ((b as f32) - (a as f32)) * t)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        return RgbaColor::new(
            lerp(left.color.r, right.color.r),
            lerp(left.color.g, right.color.g),
            lerp(left.color.b, right.color.b),
            lerp(left.color.a, right.color.a),
        );
    }
    stops.last().map(|stop| stop.color).unwrap_or(first.color)
}

fn gradient_shader(
    style: &StyleSnapshot,
    gradient: &LinearGradientStyle,
    stroke_bounds: Option<RectBounds>,
    object_bounds: Option<RectBounds>,
    canvas_bounds: RectBounds,
    opacity_multiplier: f32,
    render_scale: RenderScale,
) -> Option<Shader<'static>> {
    let bounds = match gradient.scope {
        GradientSpace::Stroke => stroke_bounds.or(object_bounds).unwrap_or(canvas_bounds),
        GradientSpace::GlyphObject => object_bounds.or(stroke_bounds).unwrap_or(canvas_bounds),
        GradientSpace::Canvas => canvas_bounds,
    };
    let radians = gradient.angle_degrees.to_radians();
    let direction = Point2 {
        x: radians.cos(),
        y: radians.sin(),
    };
    let perpendicular = Point2 {
        x: -direction.y,
        y: direction.x,
    };
    let center = bounds.center();
    let corners = [
        Point2 {
            x: bounds.min.x,
            y: bounds.min.y,
        },
        Point2 {
            x: bounds.max.x,
            y: bounds.min.y,
        },
        Point2 {
            x: bounds.min.x,
            y: bounds.max.y,
        },
        Point2 {
            x: bounds.max.x,
            y: bounds.max.y,
        },
    ];
    let projected_length = (corners
        .iter()
        .map(|corner| dot(*corner, direction))
        .fold(f32::NEG_INFINITY, f32::max)
        - corners
            .iter()
            .map(|corner| dot(*corner, direction))
            .fold(f32::INFINITY, f32::min))
    .max(1.0);
    let span = (projected_length * gradient.span_ratio.max(0.01)).max(1.0);
    let min_projection = corners
        .iter()
        .map(|corner| dot(*corner, direction))
        .fold(f32::INFINITY, f32::min);
    let start_projection = min_projection + projected_length * gradient.offset_ratio;
    let perpendicular_center = dot(center, perpendicular);
    let start = scale_point(
        Point2 {
            x: direction.x * start_projection + perpendicular.x * perpendicular_center,
            y: direction.y * start_projection + perpendicular.y * perpendicular_center,
        },
        render_scale,
    );
    let end = scale_point(
        Point2 {
            x: direction.x * (start_projection + span) + perpendicular.x * perpendicular_center,
            y: direction.y * (start_projection + span) + perpendicular.y * perpendicular_center,
        },
        render_scale,
    );
    let stops = normalized_color_stops(gradient, representative_ink_color(style))
        .into_iter()
        .map(|stop| {
            SkGradientStop::new(
                stop.position,
                tiny_skia::Color::from_rgba8(
                    stop.color.r,
                    stop.color.g,
                    stop.color.b,
                    ((stop.color.a as f32) * opacity_multiplier.clamp(0.0, 1.0)).round() as u8,
                ),
            )
        })
        .collect::<Vec<_>>();
    LinearGradient::new(
        SkPoint::from_xy(start.x, start.y),
        SkPoint::from_xy(end.x, end.y),
        stops,
        match gradient.repeat {
            GradientRepeat::None => SpreadMode::Pad,
            GradientRepeat::Repeat => SpreadMode::Repeat,
            GradientRepeat::Mirror => SpreadMode::Reflect,
        },
        Transform::identity(),
    )
}

fn partial_polyline_range(
    points: &[Point2],
    start_fraction: f32,
    end_fraction: f32,
) -> Vec<Point2> {
    if points.is_empty() {
        return Vec::new();
    }

    let start_fraction = start_fraction.clamp(0.0, 1.0);
    let end_fraction = end_fraction.clamp(0.0, 1.0);
    if end_fraction <= start_fraction {
        return points[..1].to_vec();
    }
    if start_fraction <= 0.0 && end_fraction >= 1.0 {
        return points.to_vec();
    }

    let total_length = polyline_length(points);
    if total_length <= f32::EPSILON {
        return points.to_vec();
    }

    let start_length = total_length * start_fraction;
    let end_length = total_length * end_fraction;
    let mut accumulated = 0.0;
    let mut partial = Vec::new();

    for segment in points.windows(2) {
        let start = segment[0];
        let end = segment[1];
        let segment_length = distance(start, end);
        if segment_length <= f32::EPSILON {
            continue;
        }

        let segment_start_length = accumulated;
        let segment_end_length = accumulated + segment_length;
        if segment_end_length < start_length {
            accumulated = segment_end_length;
            continue;
        }
        if segment_start_length > end_length {
            break;
        }

        let overlap_start = start_length.max(segment_start_length);
        let overlap_end = end_length.min(segment_end_length);
        if overlap_end < overlap_start {
            accumulated = segment_end_length;
            continue;
        }

        let start_t = ((overlap_start - segment_start_length) / segment_length).clamp(0.0, 1.0);
        let end_t = ((overlap_end - segment_start_length) / segment_length).clamp(0.0, 1.0);
        push_unique_point(
            &mut partial,
            Point2 {
                x: start.x + (end.x - start.x) * start_t,
                y: start.y + (end.y - start.y) * start_t,
            },
        );
        push_unique_point(
            &mut partial,
            Point2 {
                x: start.x + (end.x - start.x) * end_t,
                y: start.y + (end.y - start.y) * end_t,
            },
        );

        accumulated = segment_end_length;
    }

    if partial.is_empty() {
        points[..1].to_vec()
    } else {
        partial
    }
}

fn partial_polyline_length_range(
    points: &[Point2],
    start_length: f32,
    end_length: f32,
) -> Vec<Point2> {
    let total_length = polyline_length(points);
    if total_length <= f32::EPSILON {
        return points.to_vec();
    }

    let start_length = start_length.clamp(0.0, total_length);
    let end_length = end_length.clamp(0.0, total_length);
    partial_polyline_range(
        points,
        start_length / total_length,
        end_length / total_length,
    )
}

fn push_unique_point(points: &mut Vec<Point2>, point: Point2) {
    let should_push = points
        .last()
        .is_none_or(|last| distance(*last, point) > 0.01);
    if should_push {
        points.push(point);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecentInkAccentLayer {
    Halo,
    Core,
}

fn render_recent_ink_accent_layer(
    pixmap: &mut Pixmap,
    stroke: &Stroke,
    style: &StyleSnapshot,
    transform: &GeometryTransform,
    accent: Option<RecentInkAccentState>,
    render_scale: RenderScale,
    layer: RecentInkAccentLayer,
) {
    let Some(accent) = accent else {
        return;
    };

    let (profiles, base_color, width_multiplier, blur_scale, blend_mode) = match layer {
        RecentInkAccentLayer::Halo => (
            halo_profiles(accent.kind),
            accent.color,
            1.0 + (accent.size_multiplier.max(0.5) - 1.0) * 0.35,
            1.0,
            BlendMode::Screen,
        ),
        RecentInkAccentLayer::Core => (
            core_profiles(accent.kind),
            mix_towards_white(accent.color, core_white_mix(accent.kind)),
            1.0 + (accent.size_multiplier.max(0.5) - 1.0) * 0.18,
            0.35,
            accent.blend_mode,
        ),
    };

    let visible_points = transformed_visible_points(stroke, transform, accent.front_fraction)
        .into_iter()
        .map(|point| scale_point(point, render_scale))
        .collect::<Vec<_>>();
    if visible_points.len() < 2 {
        return;
    }
    let total_visible_length = polyline_length(&visible_points);
    let stroke_width = style.thickness * width_multiplier * render_scale.stroke;
    let tail_length = accent
        .tail_length
        .max(style.thickness * render_scale.stroke)
        .min(total_visible_length);
    if tail_length <= f32::EPSILON {
        return;
    }

    for (coverage, opacity) in profiles {
        let segment_length = (tail_length * coverage).clamp(0.0, total_visible_length);
        let points = partial_polyline_length_range(
            &visible_points,
            (total_visible_length - segment_length).max(0.0),
            total_visible_length,
        );
        if points.len() < 2 {
            continue;
        }
        let Some(path) = build_polyline_path(&points) else {
            continue;
        };
        let blur_width = accent.blur_radius.max(0.0) * blur_scale;
        let width = stroke_width + blur_width;
        let alpha = style.opacity * accent.fade * opacity;
        draw_stroked_path(
            pixmap,
            &path,
            width.max(style.thickness * render_scale.stroke),
            base_color,
            alpha,
            blend_mode,
        );
    }
}

fn recent_ink_accent_state(
    request: &RenderRequest<'_>,
    object: &GlyphObject,
    stroke: &Stroke,
    style: &StyleSnapshot,
) -> Option<RecentInkAccentState> {
    if request
        .preview_force_visible_batch
        .is_some_and(|batch_time| batch_time == object.created_at)
    {
        return None;
    }

    let head = object.entrance.head_effect.as_ref()?;
    if !matches!(
        object.entrance.kind,
        pauseink_domain::EntranceKind::PathTrace | pauseink_domain::EntranceKind::Wipe
    ) {
        return None;
    }

    let current_seconds = media_time_seconds(request.time);
    let start_seconds = effective_entrance_start_seconds(request.project, object);
    if current_seconds < start_seconds {
        return None;
    }
    let duration_seconds = entrance_duration_seconds(request.project, object);
    let end_seconds = start_seconds + duration_seconds;
    let persistence_seconds = head.persistence.max(0.0) as f64;
    if current_seconds > end_seconds + persistence_seconds {
        return None;
    }

    let front_fraction = normalized_progress_from_seconds(
        start_seconds,
        current_seconds.min(end_seconds),
        duration_seconds,
    );
    if front_fraction <= 0.0 {
        return None;
    }

    let fade = if current_seconds <= end_seconds || persistence_seconds <= f64::EPSILON {
        1.0
    } else {
        let linger =
            (((current_seconds - end_seconds) / persistence_seconds) as f32).clamp(0.0, 1.0);
        1.0 - smoothstep(linger)
    };

    let total_length = polyline_length(&transformed_visible_points(stroke, &object.transform, 1.0));
    if total_length <= f32::EPSILON {
        return None;
    }
    let tail_length = resolve_head_tail_length(head, style, total_length);

    Some(RecentInkAccentState {
        front_fraction,
        tail_length,
        fade,
        color: resolve_head_effect_color(style, head),
        kind: head.kind,
        size_multiplier: head.size_multiplier.max(0.5),
        blur_radius: head.blur_radius.max(0.0),
        blend_mode: head.blend_mode,
    })
}

fn resolve_head_tail_length(
    head: &RevealHeadEffect,
    style: &StyleSnapshot,
    total_length: f32,
) -> f32 {
    let base_tail = if head.tail_length > 0.0 {
        head.tail_length
    } else {
        style.thickness.max(1.0) * 4.0
    };
    let kind_scale = match head.kind {
        RevealHeadKind::SolidHead => 0.8,
        RevealHeadKind::GlowHead => 1.15,
        RevealHeadKind::CometTail => 1.8,
    };
    (base_tail * head.size_multiplier.max(0.5) * kind_scale)
        .clamp(style.thickness.max(1.0), total_length)
}

fn resolve_head_effect_color(style: &StyleSnapshot, head: &RevealHeadEffect) -> RgbaColor {
    match &head.color_source {
        RevealHeadColorSource::StrokeColor => representative_ink_color(style),
        RevealHeadColorSource::Custom(color) => *color,
        RevealHeadColorSource::PresetAccent => {
            if style.glow.enabled {
                style.glow.color
            } else if style.outline.enabled {
                style.outline.color
            } else if style.drop_shadow.enabled {
                style.drop_shadow.color
            } else {
                representative_ink_color(style)
            }
        }
    }
}

fn halo_profiles(kind: RevealHeadKind) -> &'static [(f32, f32)] {
    match kind {
        RevealHeadKind::SolidHead => &[(1.0, 0.08), (0.6, 0.15), (0.28, 0.24)],
        RevealHeadKind::GlowHead => &[(1.0, 0.14), (0.7, 0.24), (0.35, 0.36)],
        RevealHeadKind::CometTail => &[(1.0, 0.10), (0.78, 0.18), (0.42, 0.28)],
    }
}

fn core_profiles(kind: RevealHeadKind) -> &'static [(f32, f32)] {
    match kind {
        RevealHeadKind::SolidHead => &[(0.48, 0.24), (0.22, 0.38)],
        RevealHeadKind::GlowHead => &[(0.42, 0.22), (0.20, 0.34)],
        RevealHeadKind::CometTail => &[(0.36, 0.18), (0.16, 0.28)],
    }
}

fn core_white_mix(kind: RevealHeadKind) -> f32 {
    match kind {
        RevealHeadKind::SolidHead => 0.50,
        RevealHeadKind::GlowHead => 0.38,
        RevealHeadKind::CometTail => 0.28,
    }
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn mix_towards_white(color: RgbaColor, amount: f32) -> RgbaColor {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |channel: u8| -> u8 {
        ((channel as f32) + (255.0 - channel as f32) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    RgbaColor::new(mix(color.r), mix(color.g), mix(color.b), color.a)
}

fn scale_point(point: Point2, render_scale: RenderScale) -> Point2 {
    Point2 {
        x: point.x * render_scale.x,
        y: point.y * render_scale.y,
    }
}

fn polyline_length(points: &[Point2]) -> f32 {
    points
        .windows(2)
        .map(|segment| distance(segment[0], segment[1]))
        .sum()
}

fn distance(left: Point2, right: Point2) -> f32 {
    let dx = right.x - left.x;
    let dy = right.y - left.y;
    (dx * dx + dy * dy).sqrt()
}

fn dot(left: Point2, right: Point2) -> f32 {
    left.x * right.x + left.y * right.y
}

fn is_corner(raw_samples: &[StrokeSample], index: usize, cosine_threshold: f32) -> bool {
    if index == 0 || index + 1 >= raw_samples.len() {
        return false;
    }

    let previous = raw_samples[index - 1].position;
    let current = raw_samples[index].position;
    let next = raw_samples[index + 1].position;

    let vector_a = (current.x - previous.x, current.y - previous.y);
    let vector_b = (next.x - current.x, next.y - current.y);
    let length_a = (vector_a.0.powi(2) + vector_a.1.powi(2)).sqrt();
    let length_b = (vector_b.0.powi(2) + vector_b.1.powi(2)).sqrt();

    if length_a <= f32::EPSILON || length_b <= f32::EPSILON {
        return false;
    }

    let cosine = (vector_a.0 * vector_b.0 + vector_a.1 * vector_b.1) / (length_a * length_b);
    cosine < cosine_threshold
}

fn build_polyline_path(points: &[Point2]) -> Option<tiny_skia::Path> {
    let first = points.first()?;
    let mut builder = PathBuilder::new();
    builder.move_to(first.x, first.y);
    for point in &points[1..] {
        builder.line_to(point.x, point.y);
    }
    builder.finish()
}

fn draw_stroked_path(
    pixmap: &mut Pixmap,
    path: &tiny_skia::Path,
    width: f32,
    color: RgbaColor,
    opacity_multiplier: f32,
    blend_mode: BlendMode,
) {
    let mut paint = Paint::default();
    let alpha =
        ((color.a as f32 / 255.0) * opacity_multiplier.clamp(0.0, 1.0) * 255.0).round() as u8;
    paint.set_color_rgba8(color.r, color.g, color.b, alpha);
    paint.anti_alias = true;
    paint.blend_mode = to_tiny_blend_mode(blend_mode);

    let stroke = SkStroke {
        width: width.max(1.0),
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..SkStroke::default()
    };

    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn draw_stroked_path_with_shader(
    pixmap: &mut Pixmap,
    path: &tiny_skia::Path,
    width: f32,
    shader: Shader<'static>,
    blend_mode: BlendMode,
) {
    let mut paint = Paint::default();
    paint.shader = shader;
    paint.anti_alias = true;
    paint.blend_mode = to_tiny_blend_mode(blend_mode);

    let stroke = SkStroke {
        width: width.max(1.0),
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..SkStroke::default()
    };

    pixmap.stroke_path(path, &paint, &stroke, Transform::identity(), None);
}

fn color_to_tiny(color: RgbaColor, alpha_multiplier: f32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(
        color.r,
        color.g,
        color.b,
        ((color.a as f32) * alpha_multiplier.clamp(0.0, 1.0)).round() as u8,
    )
}

fn to_tiny_blend_mode(blend_mode: BlendMode) -> SkBlendMode {
    match blend_mode {
        BlendMode::Normal => SkBlendMode::SourceOver,
        BlendMode::Multiply => SkBlendMode::Multiply,
        BlendMode::Screen => SkBlendMode::Screen,
        BlendMode::Additive => SkBlendMode::Plus,
    }
}

#[cfg(test)]
mod tests {
    use pauseink_domain::{
        AnnotationProject, BlendMode, ClearEvent, ClearEventId, ColorMode, ColorStop,
        EntranceBehavior, EntranceKind, GlyphObject, GlyphObjectId, GradientRepeat, GradientSpace,
        LinearGradientStyle, MediaDuration, MediaTime, Point2, RevealHeadColorSource,
        RevealHeadEffect, RevealHeadKind, RgbaColor, Stroke, StrokeId, StrokeSample, StyleSnapshot,
    };

    use super::*;

    fn demo_stroke(id: &str, created_at_ms: i64) -> Stroke {
        Stroke {
            id: StrokeId::new(id),
            raw_samples: vec![
                StrokeSample {
                    position: Point2 { x: 10.0, y: 20.0 },
                    at: MediaTime::from_millis(created_at_ms),
                    pressure: None,
                },
                StrokeSample {
                    position: Point2 { x: 110.0, y: 20.0 },
                    at: MediaTime::from_millis(created_at_ms + 100),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(created_at_ms),
            ..Stroke::default()
        }
    }

    fn demo_object(stroke_id: &str, created_at_ms: i64) -> GlyphObject {
        GlyphObject {
            id: GlyphObjectId::new(format!("obj-{stroke_id}")),
            stroke_ids: vec![StrokeId::new(stroke_id)],
            style: StyleSnapshot {
                color: RgbaColor::new(255, 255, 255, 255),
                thickness: 8.0,
                ..StyleSnapshot::default()
            },
            created_at: MediaTime::from_millis(created_at_ms),
            ..GlyphObject::default()
        }
    }

    fn line_stroke(id: &str, created_at_ms: i64, start: Point2, end: Point2) -> Stroke {
        Stroke {
            id: StrokeId::new(id),
            raw_samples: vec![
                StrokeSample {
                    position: start,
                    at: MediaTime::from_millis(created_at_ms),
                    pressure: None,
                },
                StrokeSample {
                    position: end,
                    at: MediaTime::from_millis(created_at_ms + 100),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(created_at_ms),
            ..Stroke::default()
        }
    }

    fn entrance_object(
        id: &str,
        stroke_id: &str,
        created_at_ms: i64,
        reveal_order: u64,
        entrance_kind: EntranceKind,
        duration_ms: i64,
    ) -> GlyphObject {
        GlyphObject {
            id: GlyphObjectId::new(id),
            stroke_ids: vec![StrokeId::new(stroke_id)],
            style: StyleSnapshot {
                color: RgbaColor::new(255, 255, 255, 255),
                thickness: 8.0,
                ..StyleSnapshot::default()
            },
            entrance: EntranceBehavior {
                kind: entrance_kind,
                duration: MediaDuration::from_millis(duration_ms),
                ..EntranceBehavior::default()
            },
            ordering: pauseink_domain::OrderingMetadata {
                z_index: 0,
                capture_order: reveal_order,
                reveal_order,
            },
            created_at: MediaTime::from_millis(created_at_ms),
            ..GlyphObject::default()
        }
    }

    fn alpha_at(image: &RenderedOverlay, x: usize, y: usize) -> u8 {
        image.rgba_pixels[(y * image.width as usize + x) * 4 + 3]
    }

    fn rgba_at(image: &RenderedOverlay, x: usize, y: usize) -> [u8; 4] {
        let index = (y * image.width as usize + x) * 4;
        [
            image.rgba_pixels[index],
            image.rgba_pixels[index + 1],
            image.rgba_pixels[index + 2],
            image.rgba_pixels[index + 3],
        ]
    }

    fn rgb_energy_at(image: &RenderedOverlay, x: usize, y: usize) -> u16 {
        let rgba = rgba_at(image, x, y);
        rgba[0] as u16 + rgba[1] as u16 + rgba[2] as u16
    }

    fn red_channel_at(image: &RenderedOverlay, x: usize, y: usize) -> u8 {
        rgba_at(image, x, y)[0]
    }

    fn entrance_visibility(
        project: &AnnotationProject,
        object: &GlyphObject,
        time: MediaTime,
    ) -> VisibilityState {
        evaluate_entrance(
            &RenderRequest {
                project,
                time,
                preview_force_visible_batch: None,
                width: 128,
                height: 48,
                source_width: 128,
                source_height: 48,
                background: RgbaColor::new(0, 0, 0, 0),
            },
            Some(object),
            time,
        )
    }

    fn preview_visibility(
        project: &AnnotationProject,
        object: &GlyphObject,
        time: MediaTime,
        force_visible_batch: MediaTime,
    ) -> VisibilityState {
        evaluate_entrance(
            &RenderRequest {
                project,
                time,
                preview_force_visible_batch: Some(force_visible_batch),
                width: 128,
                height: 48,
                source_width: 128,
                source_height: 48,
                background: RgbaColor::new(0, 0, 0, 0),
            },
            Some(object),
            time,
        )
    }

    #[test]
    fn visible_stroke_renders_non_zero_alpha_pixels() {
        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![demo_object("stroke-1", 0)],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(100),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert!(alpha_at(&image, 20, 20) > 0);
    }

    #[test]
    fn derive_layers_preserve_corner_positions_reasonably() {
        let raw = vec![
            StrokeSample {
                position: Point2 { x: 10.0, y: 10.0 },
                at: MediaTime::from_millis(0),
                pressure: None,
            },
            StrokeSample {
                position: Point2 { x: 60.0, y: 10.0 },
                at: MediaTime::from_millis(10),
                pressure: None,
            },
            StrokeSample {
                position: Point2 { x: 60.0, y: 60.0 },
                at: MediaTime::from_millis(20),
                pressure: None,
            },
            StrokeSample {
                position: Point2 { x: 60.0, y: 110.0 },
                at: MediaTime::from_millis(30),
                pressure: None,
            },
        ];

        let (stabilized, derived) = derive_stroke_layers(&raw, 80);

        assert_eq!(stabilized.len(), raw.len());
        assert_eq!(derived.points.len(), raw.len());
        assert!((stabilized[1].position.x - raw[1].position.x).abs() < 1.0);
        assert!(stabilized[2].position.y > raw[1].position.y);
        assert!(stabilized[2].position.y < raw[2].position.y);
    }

    #[test]
    fn path_trace_only_reveals_front_part_before_completion() {
        let mut object = demo_object("stroke-1", 0);
        object.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };

        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![object],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert!(alpha_at(&image, 35, 20) > 0);
        assert_eq!(alpha_at(&image, 95, 20), 0);
    }

    #[test]
    fn reveal_head_effect_accents_recent_front_segment_without_tinting_old_ink() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let mut object = entrance_object("obj-1", "stroke-1", 0, 0, EntranceKind::PathTrace, 1_000);
        object.style.color = RgbaColor::new(32, 96, 224, 255);
        object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::GlowHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 160, 120, 255)),
            size_multiplier: 1.45,
            blur_radius: 10.0,
            tail_length: 18.0,
            persistence: 0.15,
            blend_mode: BlendMode::Screen,
        });

        let project = AnnotationProject {
            strokes: vec![stroke],
            glyph_objects: vec![object],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 160,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let old_ink = rgba_at(&image, 24, 20);
        let recent_front = rgba_at(&image, 58, 20);

        assert!(
            recent_front[0] > old_ink[0] + 18,
            "expected recent front segment to gain warm highlight, old={old_ink:?} recent={recent_front:?}"
        );
        assert!(
            recent_front[1] >= old_ink[1],
            "expected accent not to darken recent ink, old={old_ink:?} recent={recent_front:?}"
        );
    }

    #[test]
    fn reveal_head_effect_stays_visible_when_preview_is_downscaled() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 100.0, y: 200.0 },
            Point2 {
                x: 1_100.0,
                y: 200.0,
            },
        );
        let mut object = entrance_object("obj-1", "stroke-1", 0, 0, EntranceKind::PathTrace, 1_000);
        object.style.color = RgbaColor::new(32, 96, 224, 255);
        object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::GlowHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 160, 120, 255)),
            size_multiplier: 1.45,
            blur_radius: 10.0,
            tail_length: 18.0,
            persistence: 0.15,
            blend_mode: BlendMode::Screen,
        });

        let project = AnnotationProject {
            strokes: vec![stroke.clone()],
            glyph_objects: vec![object.clone()],
            ..AnnotationProject::default()
        };
        let without_head = AnnotationProject {
            strokes: vec![stroke],
            glyph_objects: vec![GlyphObject {
                entrance: EntranceBehavior {
                    head_effect: None,
                    ..object.entrance.clone()
                },
                ..object
            }],
            ..AnnotationProject::default()
        };

        let with_head = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 1_600,
            source_height: 480,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");
        let without_head = render_overlay_rgba(&RenderRequest {
            project: &without_head,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 1_600,
            source_height: 480,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert!(
            rgb_energy_at(&with_head, 58, 20) > rgb_energy_at(&without_head, 58, 20) + 8,
            "preview downscale 後も先端アクセントが recent front で見える必要がある"
        );
    }

    #[test]
    fn reveal_head_effect_tail_length_remains_usable_in_downscaled_preview() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 100.0, y: 200.0 },
            Point2 {
                x: 1_100.0,
                y: 200.0,
            },
        );
        let mut object = entrance_object("obj-1", "stroke-1", 0, 0, EntranceKind::PathTrace, 1_000);
        object.style.color = RgbaColor::new(32, 96, 224, 255);
        object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::GlowHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 160, 120, 255)),
            size_multiplier: 1.45,
            blur_radius: 10.0,
            tail_length: 18.0,
            persistence: 0.15,
            blend_mode: BlendMode::Screen,
        });

        let project = AnnotationProject {
            strokes: vec![stroke.clone()],
            glyph_objects: vec![object.clone()],
            ..AnnotationProject::default()
        };
        let without_head = AnnotationProject {
            strokes: vec![stroke],
            glyph_objects: vec![GlyphObject {
                entrance: EntranceBehavior {
                    head_effect: None,
                    ..object.entrance.clone()
                },
                ..object
            }],
            ..AnnotationProject::default()
        };

        let with_head = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 1_600,
            source_height: 480,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");
        let without_head = render_overlay_rgba(&RenderRequest {
            project: &without_head,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 1_600,
            source_height: 480,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert!(
            rgb_energy_at(&with_head, 46, 20) > rgb_energy_at(&without_head, 46, 20) + 8,
            "追従長 px は preview 縮小後も先端から少し後ろまで見える長さとして効いてほしい"
        );
    }

    #[test]
    fn reveal_head_effect_does_not_apply_to_instant_entrance() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let mut object = entrance_object("obj-1", "stroke-1", 0, 0, EntranceKind::Instant, 1_000);
        object.style.color = RgbaColor::new(32, 96, 224, 255);
        object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::GlowHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 160, 120, 255)),
            size_multiplier: 1.45,
            blur_radius: 10.0,
            tail_length: 18.0,
            persistence: 0.15,
            blend_mode: BlendMode::Screen,
        });

        let project = AnnotationProject {
            strokes: vec![stroke],
            glyph_objects: vec![object],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 160,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert_eq!(rgba_at(&image, 58, 20), [32, 96, 224, 255]);
    }

    #[test]
    fn reveal_head_effect_persists_briefly_after_completion() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let mut object = entrance_object("obj-1", "stroke-1", 0, 0, EntranceKind::PathTrace, 1_000);
        object.style.color = RgbaColor::new(96, 180, 255, 255);
        object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::GlowHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 255, 255, 255)),
            size_multiplier: 1.35,
            blur_radius: 8.0,
            tail_length: 18.0,
            persistence: 0.20,
            blend_mode: BlendMode::Screen,
        });

        let project = AnnotationProject {
            strokes: vec![stroke],
            glyph_objects: vec![object],
            ..AnnotationProject::default()
        };

        let lingering = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(1_100),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 160,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");
        let expired = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(1_260),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 160,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert!(
            rgb_energy_at(&lingering, 107, 20) > rgb_energy_at(&expired, 107, 20) + 20,
            "persistence 中は終端近傍の accent が残り、期限後は base に戻るべき"
        );
    }

    #[test]
    fn reveal_head_effect_does_not_draw_over_higher_z_base_stroke() {
        let low_stroke = line_stroke(
            "stroke-low",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let high_stroke = line_stroke(
            "stroke-high",
            0,
            Point2 { x: 60.0, y: 4.0 },
            Point2 { x: 60.0, y: 36.0 },
        );
        let mut low_object = entrance_object(
            "obj-low",
            "stroke-low",
            0,
            0,
            EntranceKind::PathTrace,
            1_000,
        );
        low_object.style.color = RgbaColor::new(40, 80, 220, 255);
        low_object.style.thickness = 12.0;
        low_object.ordering.z_index = 0;
        low_object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::GlowHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 180, 100, 255)),
            size_multiplier: 1.4,
            blur_radius: 8.0,
            tail_length: 24.0,
            persistence: 0.15,
            blend_mode: BlendMode::Screen,
        });
        let mut high_object =
            entrance_object("obj-high", "stroke-high", 0, 1, EntranceKind::Instant, 0);
        high_object.style.color = RgbaColor::new(20, 220, 60, 255);
        high_object.style.thickness = 14.0;
        high_object.ordering.z_index = 1;

        let project = AnnotationProject {
            strokes: vec![low_stroke, high_stroke],
            glyph_objects: vec![low_object, high_object],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 160,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let crossing = rgba_at(&image, 60, 20);
        assert!(
            crossing[1] > crossing[0] + 40,
            "高 z object の body が head accent より前に出てほしい, rgba={crossing:?}"
        );
    }

    #[test]
    fn reveal_head_effect_tail_length_uses_transformed_screen_space() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let mut object = entrance_object("obj-1", "stroke-1", 0, 0, EntranceKind::PathTrace, 1_000);
        object.style.color = RgbaColor::new(32, 96, 224, 255);
        object.style.thickness = 12.0;
        object.transform.scale_x = 2.0;
        object.entrance.head_effect = Some(RevealHeadEffect {
            kind: RevealHeadKind::SolidHead,
            color_source: RevealHeadColorSource::Custom(RgbaColor::new(255, 160, 120, 255)),
            size_multiplier: 1.25,
            blur_radius: 0.0,
            tail_length: 20.0,
            persistence: 0.0,
            blend_mode: BlendMode::Screen,
        });
        let project = AnnotationProject {
            strokes: vec![stroke],
            glyph_objects: vec![object.clone()],
            ..AnnotationProject::default()
        };
        let accent = recent_ink_accent_state(
            &RenderRequest {
                project: &project,
                time: MediaTime::from_millis(500),
                preview_force_visible_batch: None,
                width: 240,
                height: 48,
                source_width: 240,
                source_height: 48,
                background: RgbaColor::new(0, 0, 0, 0),
            },
            &object,
            &project.strokes[0],
            &object.style,
        )
        .expect("head accent should exist");

        assert!(
            (accent.tail_length - 20.0).abs() < 0.02,
            "tail_length=20px なら transform 後 length でも UI 指定どおり 20px を保ってほしい, got {:?}",
            accent
        );
    }

    #[test]
    fn preset_accent_color_prefers_glow_then_outline_then_shadow_then_stroke() {
        let mut style = StyleSnapshot {
            color: RgbaColor::new(10, 20, 30, 255),
            ..StyleSnapshot::default()
        };
        let head = RevealHeadEffect {
            color_source: RevealHeadColorSource::PresetAccent,
            ..RevealHeadEffect::default()
        };

        style.drop_shadow.enabled = true;
        style.drop_shadow.color = RgbaColor::new(40, 50, 60, 255);
        assert_eq!(
            resolve_head_effect_color(&style, &head),
            RgbaColor::new(40, 50, 60, 255)
        );

        style.outline.enabled = true;
        style.outline.color = RgbaColor::new(70, 80, 90, 255);
        assert_eq!(
            resolve_head_effect_color(&style, &head),
            RgbaColor::new(70, 80, 90, 255)
        );

        style.glow.enabled = true;
        style.glow.color = RgbaColor::new(100, 110, 120, 255);
        assert_eq!(
            resolve_head_effect_color(&style, &head),
            RgbaColor::new(100, 110, 120, 255)
        );
    }

    #[test]
    fn stroke_color_head_effect_prefers_gradient_representative_color() {
        let style = StyleSnapshot {
            color: RgbaColor::new(20, 240, 40, 255),
            color_mode: ColorMode::LinearGradient,
            gradient: Some(LinearGradientStyle {
                scope: GradientSpace::GlyphObject,
                repeat: GradientRepeat::None,
                angle_degrees: 0.0,
                span_ratio: 1.0,
                offset_ratio: 0.0,
                stops: vec![
                    ColorStop {
                        position: 0.0,
                        color: RgbaColor::new(255, 0, 0, 255),
                    },
                    ColorStop {
                        position: 1.0,
                        color: RgbaColor::new(0, 0, 255, 255),
                    },
                ],
            }),
            ..StyleSnapshot::default()
        };
        let head = RevealHeadEffect {
            color_source: RevealHeadColorSource::StrokeColor,
            ..RevealHeadEffect::default()
        };

        assert_eq!(
            resolve_head_effect_color(&style, &head),
            RgbaColor::new(128, 0, 128, 255),
            "gradient 有効時は stale な solid color ではなく中点色を使いたい"
        );
    }

    #[test]
    fn fixed_duration_speed_scalar_changes_reveal_progress() {
        let mut fast_object = demo_object("stroke-1", 0);
        fast_object.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            speed_scalar: 2.0,
            ..EntranceBehavior::default()
        };
        let mut slow_object = demo_object("stroke-1", 0);
        slow_object.id = GlyphObjectId::new("obj-slow");
        slow_object.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            speed_scalar: 0.5,
            ..EntranceBehavior::default()
        };

        let fast_image = render_overlay_rgba(&RenderRequest {
            project: &AnnotationProject {
                strokes: vec![demo_stroke("stroke-1", 0)],
                glyph_objects: vec![fast_object],
                ..AnnotationProject::default()
            },
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("fast render");
        let slow_image = render_overlay_rgba(&RenderRequest {
            project: &AnnotationProject {
                strokes: vec![demo_stroke("stroke-1", 0)],
                glyph_objects: vec![slow_object],
                ..AnnotationProject::default()
            },
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("slow render");

        assert!(
            alpha_at(&fast_image, 95, 20) > 0,
            "speed_scalar を大きくすると同じ時刻でより先まで見えるべき"
        );
        assert_eq!(
            alpha_at(&slow_image, 95, 20),
            0,
            "speed_scalar を小さくすると同じ時刻ではまだ終端まで届かないべき"
        );
    }

    #[test]
    fn timed_entrances_wait_for_prior_timed_reveals_but_instant_objects_stay_visible() {
        let project = AnnotationProject {
            strokes: vec![
                line_stroke(
                    "stroke-a",
                    0,
                    Point2 { x: 10.0, y: 12.0 },
                    Point2 { x: 110.0, y: 12.0 },
                ),
                line_stroke(
                    "stroke-b",
                    0,
                    Point2 { x: 20.0, y: 4.0 },
                    Point2 { x: 20.0, y: 44.0 },
                ),
                line_stroke(
                    "stroke-c",
                    0,
                    Point2 { x: 10.0, y: 36.0 },
                    Point2 { x: 110.0, y: 36.0 },
                ),
            ],
            glyph_objects: vec![
                entrance_object("object-a", "stroke-a", 0, 1, EntranceKind::PathTrace, 1_000),
                entrance_object("object-b", "stroke-b", 0, 2, EntranceKind::Instant, 1),
                entrance_object("object-c", "stroke-c", 0, 3, EntranceKind::PathTrace, 1_000),
            ],
            ..AnnotationProject::default()
        };

        let midway = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("midway render");

        assert!(alpha_at(&midway, 35, 12) > 0, "A は途中まで見えるべき");
        assert_eq!(alpha_at(&midway, 95, 12), 0, "A はまだ終端まで届かないべき");
        assert!(
            alpha_at(&midway, 20, 24) > 0,
            "Instant B はすでに表示されるべき"
        );
        assert_eq!(
            alpha_at(&midway, 25, 36),
            0,
            "C は A 完了待ちでまだ始まらないべき"
        );

        let after_a = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(1_100),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("after A render");

        assert!(alpha_at(&after_a, 95, 12) > 0, "A は完了しているべき");
        assert!(alpha_at(&after_a, 20, 24) > 0, "Instant B は維持されるべき");
        assert!(
            alpha_at(&after_a, 18, 36) > 0,
            "C は A 完了後に開始するべき"
        );
        assert_eq!(alpha_at(&after_a, 50, 36), 0, "C はまだ途中のはず");
    }

    #[test]
    fn later_pause_batch_does_not_wait_for_earlier_batchs_timed_queue() {
        let project = AnnotationProject {
            strokes: vec![
                line_stroke(
                    "stroke-a",
                    0,
                    Point2 { x: 10.0, y: 12.0 },
                    Point2 { x: 110.0, y: 12.0 },
                ),
                line_stroke(
                    "stroke-b",
                    0,
                    Point2 { x: 10.0, y: 24.0 },
                    Point2 { x: 110.0, y: 24.0 },
                ),
                line_stroke(
                    "stroke-c",
                    120,
                    Point2 { x: 10.0, y: 36.0 },
                    Point2 { x: 110.0, y: 36.0 },
                ),
            ],
            glyph_objects: vec![
                entrance_object("object-a", "stroke-a", 0, 1, EntranceKind::PathTrace, 1_000),
                entrance_object("object-b", "stroke-b", 0, 2, EntranceKind::PathTrace, 1_000),
                entrance_object(
                    "object-c",
                    "stroke-c",
                    120,
                    3,
                    EntranceKind::PathTrace,
                    1_000,
                ),
            ],
            ..AnnotationProject::default()
        };

        let a_mid = entrance_visibility(
            &project,
            &project.glyph_objects[0],
            MediaTime::from_millis(500),
        );
        let b_mid = entrance_visibility(
            &project,
            &project.glyph_objects[1],
            MediaTime::from_millis(500),
        );
        let c_early = entrance_visibility(
            &project,
            &project.glyph_objects[2],
            MediaTime::from_millis(500),
        );

        assert!(a_mid.path_fraction > 0.45 && a_mid.path_fraction < 0.55);
        assert_eq!(b_mid, VisibilityState::HIDDEN);
        assert!(
            c_early.path_fraction > 0.35 && c_early.path_fraction < 0.39,
            "後で pause した batch の C は、A/B batch 完了待ちではなく自分の created_at から進み始めるべき: {c_early:?}"
        );
    }

    #[test]
    fn timed_entrance_queue_resets_after_clear_page_boundary() {
        let project = AnnotationProject {
            strokes: vec![
                line_stroke(
                    "stroke-a",
                    0,
                    Point2 { x: 10.0, y: 12.0 },
                    Point2 { x: 110.0, y: 12.0 },
                ),
                line_stroke(
                    "stroke-d",
                    700,
                    Point2 { x: 10.0, y: 36.0 },
                    Point2 { x: 110.0, y: 36.0 },
                ),
            ],
            glyph_objects: vec![
                entrance_object("object-a", "stroke-a", 0, 1, EntranceKind::PathTrace, 1_000),
                entrance_object(
                    "object-d",
                    "stroke-d",
                    700,
                    2,
                    EntranceKind::PathTrace,
                    1_000,
                ),
            ],
            clear_events: vec![ClearEvent {
                id: ClearEventId::new("clear-1"),
                time: MediaTime::from_millis(600),
                kind: ClearKind::Instant,
                ..ClearEvent::default()
            }],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(800),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("page 2 render");

        assert_eq!(
            alpha_at(&image, 35, 12),
            0,
            "page 1 の A は clear 後に消えているべき"
        );
        assert!(
            alpha_at(&image, 18, 36) > 0,
            "page 2 の timed entrance は page 1 完了待ちを引き継がず開始するべき"
        );
        assert_eq!(alpha_at(&image, 50, 36), 0, "page 2 の D はまだ途中のはず");
    }

    #[test]
    fn timed_entrance_waits_for_previous_timed_reveal_even_with_instant_between() {
        let mut object_a = demo_object("stroke-a", 0);
        object_a.ordering.reveal_order = 1;
        object_a.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_b = demo_object("stroke-b", 0);
        object_b.ordering.reveal_order = 2;
        object_b.entrance = EntranceBehavior {
            kind: EntranceKind::Instant,
            ..EntranceBehavior::default()
        };
        let mut object_c = demo_object("stroke-c", 0);
        object_c.ordering.reveal_order = 3;
        object_c.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };

        let project = AnnotationProject {
            strokes: vec![
                demo_stroke("stroke-a", 0),
                demo_stroke("stroke-b", 0),
                demo_stroke("stroke-c", 0),
            ],
            glyph_objects: vec![object_a.clone(), object_b.clone(), object_c.clone()],
            ..AnnotationProject::default()
        };

        let a_mid = entrance_visibility(&project, &object_a, MediaTime::from_millis(500));
        let b_mid = entrance_visibility(&project, &object_b, MediaTime::from_millis(500));
        let c_mid = entrance_visibility(&project, &object_c, MediaTime::from_millis(500));
        let c_after_a = entrance_visibility(&project, &object_c, MediaTime::from_millis(1_200));

        assert!(a_mid.path_fraction > 0.45 && a_mid.path_fraction < 0.55);
        assert_eq!(b_mid, VisibilityState::FULLY_VISIBLE);
        assert_eq!(c_mid, VisibilityState::HIDDEN);
        assert!(c_after_a.path_fraction > 0.15 && c_after_a.path_fraction < 0.25);
    }

    #[test]
    fn later_paused_batch_starts_in_parallel_with_first_timed_object_of_page() {
        let mut object_a = demo_object("stroke-a", 0);
        object_a.ordering.reveal_order = 1;
        object_a.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_b = demo_object("stroke-b", 0);
        object_b.ordering.reveal_order = 2;
        object_b.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_c = demo_object("stroke-c", 200);
        object_c.ordering.reveal_order = 3;
        object_c.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };

        let project = AnnotationProject {
            strokes: vec![
                demo_stroke("stroke-a", 0),
                demo_stroke("stroke-b", 0),
                demo_stroke("stroke-c", 200),
            ],
            glyph_objects: vec![object_a.clone(), object_b.clone(), object_c.clone()],
            ..AnnotationProject::default()
        };

        let a_mid = entrance_visibility(&project, &object_a, MediaTime::from_millis(200));
        let b_mid = entrance_visibility(&project, &object_b, MediaTime::from_millis(200));
        let c_mid = entrance_visibility(&project, &object_c, MediaTime::from_millis(200));
        let b_after_a = entrance_visibility(&project, &object_b, MediaTime::from_millis(1_200));
        let c_after_a = entrance_visibility(&project, &object_c, MediaTime::from_millis(1_200));

        assert!(a_mid.path_fraction > 0.19 && a_mid.path_fraction < 0.21);
        assert_eq!(b_mid, VisibilityState::HIDDEN);
        assert!(
            c_mid.path_fraction >= 0.0 && c_mid.path_fraction < 0.01,
            "後から追加した paused batch の先頭 timed object は、自分の created_at で開始待ちに入るべき: {c_mid:?}"
        );
        assert!(
            b_after_a.path_fraction > 0.19 && b_after_a.path_fraction < 0.21,
            "同じ paused batch 内の 2 つ目 timed object は 1 つ目の完了後に始まるべき"
        );
        assert_eq!(c_after_a, VisibilityState::FULLY_VISIBLE);
    }

    #[test]
    fn paused_preview_forces_current_batch_fully_visible_without_releasing_previous_batch_queue() {
        let mut object_a = demo_object("stroke-a", 0);
        object_a.ordering.reveal_order = 1;
        object_a.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_b = demo_object("stroke-b", 0);
        object_b.ordering.reveal_order = 2;
        object_b.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_c = demo_object("stroke-c", 200);
        object_c.ordering.reveal_order = 3;
        object_c.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };

        let project = AnnotationProject {
            strokes: vec![
                demo_stroke("stroke-a", 0),
                demo_stroke("stroke-b", 0),
                demo_stroke("stroke-c", 200),
            ],
            glyph_objects: vec![object_a.clone(), object_b.clone(), object_c.clone()],
            ..AnnotationProject::default()
        };

        let a_preview = preview_visibility(
            &project,
            &object_a,
            MediaTime::from_millis(200),
            MediaTime::from_millis(200),
        );
        let b_preview = preview_visibility(
            &project,
            &object_b,
            MediaTime::from_millis(200),
            MediaTime::from_millis(200),
        );
        let c_preview = preview_visibility(
            &project,
            &object_c,
            MediaTime::from_millis(200),
            MediaTime::from_millis(200),
        );

        assert!(a_preview.path_fraction > 0.19 && a_preview.path_fraction < 0.21);
        assert_eq!(b_preview, VisibilityState::HIDDEN);
        assert_eq!(c_preview, VisibilityState::FULLY_VISIBLE);
    }

    #[test]
    fn timed_entrance_on_next_page_does_not_wait_for_previous_page_reveal() {
        let mut object_a = demo_object("stroke-a", 0);
        object_a.ordering.reveal_order = 1;
        object_a.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_c = demo_object("stroke-c", 200);
        object_c.ordering.reveal_order = 1;
        object_c.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };

        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-a", 0), demo_stroke("stroke-c", 200)],
            glyph_objects: vec![object_a, object_c.clone()],
            clear_events: vec![ClearEvent {
                id: ClearEventId::new("clear-1"),
                time: MediaTime::from_millis(150),
                kind: ClearKind::Instant,
                ..ClearEvent::default()
            }],
            ..AnnotationProject::default()
        };

        let c_page_two = entrance_visibility(&project, &object_c, MediaTime::from_millis(300));
        assert!(
            c_page_two.path_fraction > 0.09 && c_page_two.path_fraction < 0.11,
            "page clear 後の 2 page 目 timed reveal は clear 時刻ではなく object.created_at から進むべき: {c_page_two:?}"
        );
    }

    #[test]
    fn dissolve_entrance_waits_for_previous_path_trace_reveal() {
        let mut object_a = demo_object("stroke-a", 0);
        object_a.ordering.reveal_order = 1;
        object_a.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };
        let mut object_b = demo_object("stroke-b", 0);
        object_b.ordering.reveal_order = 2;
        object_b.entrance = EntranceBehavior {
            kind: EntranceKind::Dissolve,
            duration: MediaDuration::from_millis(1_000),
            ..EntranceBehavior::default()
        };

        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-a", 0), demo_stroke("stroke-b", 0)],
            glyph_objects: vec![object_a, object_b.clone()],
            ..AnnotationProject::default()
        };

        let before_queue_release =
            entrance_visibility(&project, &object_b, MediaTime::from_millis(500));
        let after_queue_release =
            entrance_visibility(&project, &object_b, MediaTime::from_millis(1_300));

        assert_eq!(before_queue_release, VisibilityState::HIDDEN);
        assert!(after_queue_release.alpha > 0.25 && after_queue_release.alpha < 0.35);
    }

    #[test]
    fn post_action_style_change_applies_after_glyph_object_reveal() {
        let mut object = demo_object("stroke-1", 0);
        object.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(600),
            ..EntranceBehavior::default()
        };
        object.style.color = RgbaColor::new(40, 180, 255, 255);
        object.post_actions = vec![pauseink_domain::PostAction {
            timing_scope: pauseink_domain::PostActionTimingScope::AfterGlyphObject,
            action: pauseink_domain::PostActionKind::StyleChange {
                style: StyleSnapshot {
                    color: RgbaColor::new(255, 96, 64, 255),
                    thickness: 15.0,
                    opacity: 0.38,
                    ..object.style.clone()
                },
            },
        }];

        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![object.clone()],
            ..AnnotationProject::default()
        };

        let before_finish = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(300),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render before finish");
        let after_finish = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(900),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render after finish");

        assert!(
            red_channel_at(&after_finish, 32, 20) > red_channel_at(&before_finish, 32, 20) + 30,
            "reveal 完了後は post-action style change の暖色寄りへ変化してほしい"
        );
        assert!(
            alpha_at(&after_finish, 32, 20) < alpha_at(&before_finish, 32, 20),
            "opacity を下げた style change が reveal 完了後に反映されてほしい"
        );
    }

    #[test]
    fn post_action_interpolated_style_change_progresses_after_glyph_object_reveal() {
        let mut object = demo_object("stroke-1", 0);
        object.entrance = EntranceBehavior {
            kind: EntranceKind::PathTrace,
            duration: MediaDuration::from_millis(400),
            ..EntranceBehavior::default()
        };
        object.style.color = RgbaColor::new(40, 180, 255, 255);
        object.post_actions = vec![pauseink_domain::PostAction {
            timing_scope: pauseink_domain::PostActionTimingScope::AfterGlyphObject,
            action: pauseink_domain::PostActionKind::InterpolatedStyleChange {
                style: StyleSnapshot {
                    color: RgbaColor::new(255, 220, 120, 255),
                    opacity: 0.28,
                    ..object.style.clone()
                },
                duration: MediaDuration::from_millis(800),
            },
        }];

        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![object],
            ..AnnotationProject::default()
        };

        let early = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(500),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render early");
        let late = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(1_100),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render late");

        assert!(
            red_channel_at(&late, 32, 20) > red_channel_at(&early, 32, 20) + 20,
            "interpolated style change は reveal 完了後に徐々に target color へ近づいてほしい"
        );
        assert!(
            alpha_at(&late, 32, 20) < alpha_at(&early, 32, 20),
            "interpolated style change の target opacity が時間とともに効いてほしい"
        );
    }

    #[test]
    fn post_action_after_group_waits_for_last_group_member_reveal() {
        let mut object_a =
            entrance_object("object-a", "stroke-a", 0, 1, EntranceKind::PathTrace, 400);
        object_a.post_actions = vec![pauseink_domain::PostAction {
            timing_scope: pauseink_domain::PostActionTimingScope::AfterGroup,
            action: pauseink_domain::PostActionKind::StyleChange {
                style: StyleSnapshot {
                    color: RgbaColor::new(255, 96, 64, 255),
                    ..object_a.style.clone()
                },
            },
        }];
        let object_b = entrance_object("object-b", "stroke-b", 0, 2, EntranceKind::PathTrace, 600);
        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-a", 0), demo_stroke("stroke-b", 0)],
            glyph_objects: vec![object_a.clone(), object_b],
            groups: vec![pauseink_domain::Group {
                id: pauseink_domain::GroupId::new("group-1"),
                glyph_object_ids: vec![object_a.id.clone(), GlyphObjectId::new("object-b")],
                ..pauseink_domain::Group::default()
            }],
            ..AnnotationProject::default()
        };
        let stroke = project
            .strokes
            .iter()
            .find(|stroke| stroke.id == StrokeId::new("stroke-a"))
            .expect("stroke-a");

        let before_group_end =
            evaluate_post_actions(&project, &object_a, stroke, MediaTime::from_millis(900));
        let after_group_end =
            evaluate_post_actions(&project, &object_a, stroke, MediaTime::from_millis(1_100));

        assert_eq!(before_group_end.style.color, object_a.style.color);
        assert_eq!(
            after_group_end.style.color,
            RgbaColor::new(255, 96, 64, 255),
            "group 内最後の timed reveal 完了後にだけ style change が効いてほしい"
        );
    }

    #[test]
    fn post_action_after_run_waits_for_last_batch_member_reveal() {
        let mut object_a =
            entrance_object("object-a", "stroke-a", 0, 1, EntranceKind::PathTrace, 400);
        object_a.post_actions = vec![pauseink_domain::PostAction {
            timing_scope: pauseink_domain::PostActionTimingScope::AfterRun,
            action: pauseink_domain::PostActionKind::StyleChange {
                style: StyleSnapshot {
                    color: RgbaColor::new(255, 210, 120, 255),
                    ..object_a.style.clone()
                },
            },
        }];
        let object_b = entrance_object("object-b", "stroke-b", 0, 2, EntranceKind::PathTrace, 500);
        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-a", 0), demo_stroke("stroke-b", 0)],
            glyph_objects: vec![object_a.clone(), object_b],
            ..AnnotationProject::default()
        };
        let stroke = project
            .strokes
            .iter()
            .find(|stroke| stroke.id == StrokeId::new("stroke-a"))
            .expect("stroke-a");

        let before_run_end =
            evaluate_post_actions(&project, &object_a, stroke, MediaTime::from_millis(700));
        let after_run_end =
            evaluate_post_actions(&project, &object_a, stroke, MediaTime::from_millis(950));

        assert_eq!(before_run_end.style.color, object_a.style.color);
        assert_eq!(
            after_run_end.style.color,
            RgbaColor::new(255, 210, 120, 255),
            "同一 batch run 内最後の timed reveal 完了後に style change が効いてほしい"
        );
    }

    #[test]
    fn post_action_after_stroke_uses_stroke_scope_windows() {
        let stroke_a = line_stroke(
            "stroke-a",
            0,
            Point2 { x: 12.0, y: 20.0 },
            Point2 { x: 62.0, y: 20.0 },
        );
        let stroke_b = line_stroke(
            "stroke-b",
            0,
            Point2 { x: 12.0, y: 28.0 },
            Point2 { x: 108.0, y: 28.0 },
        );
        let style = StyleSnapshot {
            color: RgbaColor::new(64, 180, 255, 255),
            thickness: 8.0,
            ..StyleSnapshot::default()
        };
        let object = GlyphObject {
            id: GlyphObjectId::new("object-1"),
            stroke_ids: vec![stroke_a.id.clone(), stroke_b.id.clone()],
            style: style.clone(),
            entrance: EntranceBehavior {
                kind: EntranceKind::PathTrace,
                scope: pauseink_domain::EffectScope::Stroke,
                order: pauseink_domain::EffectOrder::Serial,
                duration: MediaDuration::from_millis(600),
                ..EntranceBehavior::default()
            },
            post_actions: vec![pauseink_domain::PostAction {
                timing_scope: pauseink_domain::PostActionTimingScope::AfterStroke,
                action: pauseink_domain::PostActionKind::StyleChange {
                    style: StyleSnapshot {
                        color: RgbaColor::new(255, 120, 96, 255),
                        ..style.clone()
                    },
                },
            }],
            ..GlyphObject::default()
        };
        let project = AnnotationProject {
            strokes: vec![stroke_a.clone(), stroke_b.clone()],
            glyph_objects: vec![object.clone()],
            ..AnnotationProject::default()
        };

        let first_after_first_end =
            evaluate_post_actions(&project, &object, &stroke_a, MediaTime::from_millis(350));
        let second_before_second_end =
            evaluate_post_actions(&project, &object, &stroke_b, MediaTime::from_millis(350));

        assert_eq!(
            first_after_first_end.style.color,
            RgbaColor::new(255, 120, 96, 255),
            "先頭 stroke は自分の reveal window 終了後に post-action を始めてほしい"
        );
        assert_eq!(
            second_before_second_end.style.color, style.color,
            "後続 stroke は自分の reveal window が終わるまで post-action を待ってほしい"
        );
    }

    #[test]
    fn clear_boundary_hides_previous_page_objects() {
        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![demo_object("stroke-1", 0)],
            clear_events: vec![ClearEvent {
                id: ClearEventId::new("clear-1"),
                time: MediaTime::from_millis(700),
                kind: ClearKind::Instant,
                ..ClearEvent::default()
            }],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(900),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        assert_eq!(alpha_at(&image, 20, 20), 0);
    }

    #[test]
    fn dissolve_clear_reduces_alpha_during_clear_window() {
        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![demo_object("stroke-1", 0)],
            clear_events: vec![ClearEvent {
                id: ClearEventId::new("clear-1"),
                time: MediaTime::from_millis(700),
                kind: ClearKind::DissolveOut,
                duration: MediaDuration::from_millis(1_000),
                ..ClearEvent::default()
            }],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(1_200),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let alpha = alpha_at(&image, 20, 20);
        assert!(alpha > 0);
        assert!(alpha < 255);
    }

    #[test]
    fn render_request_scales_project_coordinates_into_preview_canvas() {
        let project = AnnotationProject {
            strokes: vec![demo_stroke("stroke-1", 0)],
            glyph_objects: vec![demo_object("stroke-1", 0)],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(100),
            preview_force_visible_batch: None,
            width: 64,
            height: 24,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("scaled preview render should succeed");

        assert!(alpha_at(&image, 6, 10) > 0);
        assert!(alpha_at(&image, 54, 10) > 0);
        assert_eq!(alpha_at(&image, image.width as usize - 1, 10), 0);
    }

    #[test]
    fn later_stroke_outline_stays_behind_earlier_stroke_body_within_same_object() {
        let horizontal = Stroke {
            id: StrokeId::new("stroke-horizontal"),
            raw_samples: vec![
                StrokeSample {
                    position: Point2 { x: 12.0, y: 24.0 },
                    at: MediaTime::from_millis(0),
                    pressure: None,
                },
                StrokeSample {
                    position: Point2 { x: 116.0, y: 24.0 },
                    at: MediaTime::from_millis(10),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(0),
            ..Stroke::default()
        };
        let vertical = Stroke {
            id: StrokeId::new("stroke-vertical"),
            raw_samples: vec![
                StrokeSample {
                    position: Point2 { x: 64.0, y: 8.0 },
                    at: MediaTime::from_millis(20),
                    pressure: None,
                },
                StrokeSample {
                    position: Point2 { x: 64.0, y: 40.0 },
                    at: MediaTime::from_millis(30),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(20),
            ..Stroke::default()
        };
        let style = StyleSnapshot {
            color: RgbaColor::new(255, 64, 32, 255),
            thickness: 6.0,
            outline: pauseink_domain::OutlineStyle {
                enabled: true,
                width: 4.0,
                color: RgbaColor::new(0, 0, 0, 255),
            },
            ..StyleSnapshot::default()
        };
        let project = AnnotationProject {
            strokes: vec![horizontal, vertical],
            glyph_objects: vec![GlyphObject {
                id: GlyphObjectId::new("object-cross"),
                stroke_ids: vec![
                    StrokeId::new("stroke-horizontal"),
                    StrokeId::new("stroke-vertical"),
                ],
                style,
                ..GlyphObject::default()
            }],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(100),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let rgba = rgba_at(&image, 69, 24);
        assert!(
            rgba[0] > 180,
            "expected horizontal body to stay visible, got {rgba:?}"
        );
        assert!(
            rgba[1] < 120,
            "expected non-outline dominant pixel, got {rgba:?}"
        );
        assert!(
            rgba[2] < 120,
            "expected non-outline dominant pixel, got {rgba:?}"
        );
        assert!(rgba[3] > 0, "expected visible pixel, got {rgba:?}");
    }

    #[test]
    fn later_object_outline_and_shadow_stay_behind_earlier_object_body() {
        let horizontal = Stroke {
            id: StrokeId::new("stroke-horizontal"),
            raw_samples: vec![
                StrokeSample {
                    position: Point2 { x: 12.0, y: 24.0 },
                    at: MediaTime::from_millis(0),
                    pressure: None,
                },
                StrokeSample {
                    position: Point2 { x: 116.0, y: 24.0 },
                    at: MediaTime::from_millis(10),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(0),
            ..Stroke::default()
        };
        let vertical = Stroke {
            id: StrokeId::new("stroke-vertical"),
            raw_samples: vec![
                StrokeSample {
                    position: Point2 { x: 64.0, y: 8.0 },
                    at: MediaTime::from_millis(20),
                    pressure: None,
                },
                StrokeSample {
                    position: Point2 { x: 64.0, y: 40.0 },
                    at: MediaTime::from_millis(30),
                    pressure: None,
                },
            ],
            created_at: MediaTime::from_millis(20),
            ..Stroke::default()
        };
        let earlier_style = StyleSnapshot {
            color: RgbaColor::new(255, 96, 32, 255),
            thickness: 6.0,
            ..StyleSnapshot::default()
        };
        let later_style = StyleSnapshot {
            color: RgbaColor::new(240, 240, 255, 255),
            thickness: 6.0,
            outline: pauseink_domain::OutlineStyle {
                enabled: true,
                width: 4.0,
                color: RgbaColor::new(0, 0, 0, 255),
            },
            drop_shadow: pauseink_domain::DropShadowStyle {
                enabled: true,
                offset_x: 0.0,
                offset_y: 0.0,
                blur_radius: 2.0,
                color: RgbaColor::new(0, 0, 0, 255),
            },
            ..StyleSnapshot::default()
        };
        let project = AnnotationProject {
            strokes: vec![horizontal, vertical],
            glyph_objects: vec![
                GlyphObject {
                    id: GlyphObjectId::new("object-horizontal"),
                    stroke_ids: vec![StrokeId::new("stroke-horizontal")],
                    style: earlier_style,
                    ordering: pauseink_domain::OrderingMetadata {
                        z_index: 0,
                        capture_order: 1,
                        reveal_order: 1,
                    },
                    ..GlyphObject::default()
                },
                GlyphObject {
                    id: GlyphObjectId::new("object-vertical"),
                    stroke_ids: vec![StrokeId::new("stroke-vertical")],
                    style: later_style,
                    ordering: pauseink_domain::OrderingMetadata {
                        z_index: 0,
                        capture_order: 2,
                        reveal_order: 2,
                    },
                    ..GlyphObject::default()
                },
            ],
            ..AnnotationProject::default()
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &project,
            time: MediaTime::from_millis(100),
            preview_force_visible_batch: None,
            width: 128,
            height: 48,
            source_width: 128,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let rgba = rgba_at(&image, 69, 24);
        assert!(
            rgba[0] > 180,
            "later object outline/shadow が earlier object body より前に来てはいけない: {rgba:?}"
        );
        assert!(
            rgba[1] < 170 && rgba[2] < 170,
            "outline/shadow の黒成分で earlier body が潰れてはいけない: {rgba:?}"
        );
    }

    #[test]
    fn object_scope_gradient_moves_with_object_translation() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let gradient = LinearGradientStyle {
            scope: GradientSpace::GlyphObject,
            repeat: GradientRepeat::None,
            angle_degrees: 0.0,
            span_ratio: 1.0,
            offset_ratio: 0.0,
            stops: vec![
                ColorStop {
                    position: 0.0,
                    color: RgbaColor::new(255, 64, 64, 255),
                },
                ColorStop {
                    position: 1.0,
                    color: RgbaColor::new(64, 96, 255, 255),
                },
            ],
        };

        let mut left = demo_object("stroke-1", 0);
        left.style.color_mode = ColorMode::LinearGradient;
        left.style.gradient = Some(gradient.clone());

        let mut right = left.clone();
        right.id = GlyphObjectId::new("obj-2");
        right.transform.translation.x = 240.0;

        let image = render_overlay_rgba(&RenderRequest {
            project: &AnnotationProject {
                strokes: vec![stroke],
                glyph_objects: vec![left, right],
                ..AnnotationProject::default()
            },
            time: MediaTime::from_millis(0),
            preview_force_visible_batch: None,
            width: 480,
            height: 48,
            source_width: 480,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let left_start = rgba_at(&image, 18, 20);
        let right_start = rgba_at(&image, 258, 20);
        assert!(
            (left_start[0] as u16) > (left_start[2] as u16) + 50,
            "left object start should be warm, got {left_start:?}"
        );
        assert!(
            (right_start[0] as u16) > (right_start[2] as u16) + 50,
            "object scope should move gradient with object, got {right_start:?}"
        );
    }

    #[test]
    fn canvas_scope_gradient_remains_global_when_object_moves() {
        let stroke = line_stroke(
            "stroke-1",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let gradient = LinearGradientStyle {
            scope: GradientSpace::Canvas,
            repeat: GradientRepeat::None,
            angle_degrees: 0.0,
            span_ratio: 1.0,
            offset_ratio: 0.0,
            stops: vec![
                ColorStop {
                    position: 0.0,
                    color: RgbaColor::new(255, 64, 64, 255),
                },
                ColorStop {
                    position: 1.0,
                    color: RgbaColor::new(64, 96, 255, 255),
                },
            ],
        };

        let mut left = demo_object("stroke-1", 0);
        left.style.color_mode = ColorMode::LinearGradient;
        left.style.gradient = Some(gradient.clone());

        let mut right = left.clone();
        right.id = GlyphObjectId::new("obj-2");
        right.transform.translation.x = 240.0;

        let image = render_overlay_rgba(&RenderRequest {
            project: &AnnotationProject {
                strokes: vec![stroke],
                glyph_objects: vec![left, right],
                ..AnnotationProject::default()
            },
            time: MediaTime::from_millis(0),
            preview_force_visible_batch: None,
            width: 480,
            height: 48,
            source_width: 480,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let left_start = rgba_at(&image, 18, 20);
        let right_start = rgba_at(&image, 258, 20);
        assert!(
            (left_start[0] as u16) > (left_start[2] as u16) + 50,
            "left object start should still be warm, got {left_start:?}"
        );
        assert!(
            (right_start[2] as u16) > (left_start[2] as u16) + 40
                && (right_start[0] as u16) + 20 < (left_start[0] as u16),
            "canvas scope should keep global gradient so translated object becomes cooler than the left one, left={left_start:?} right={right_start:?}"
        );
    }

    #[test]
    fn repeat_and_mirror_gradient_modes_change_sampling_pattern() {
        let stroke_repeat = line_stroke(
            "stroke-repeat",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let stroke_mirror = line_stroke(
            "stroke-mirror",
            0,
            Point2 { x: 10.0, y: 34.0 },
            Point2 { x: 110.0, y: 34.0 },
        );
        let make_object = |id: &str, stroke_id: &str, repeat: GradientRepeat| {
            let mut object = demo_object(stroke_id, 0);
            object.id = GlyphObjectId::new(id);
            object.style.color_mode = ColorMode::LinearGradient;
            object.style.gradient = Some(LinearGradientStyle {
                scope: GradientSpace::GlyphObject,
                repeat,
                angle_degrees: 0.0,
                span_ratio: 0.2,
                offset_ratio: 0.0,
                stops: vec![
                    ColorStop {
                        position: 0.0,
                        color: RgbaColor::new(255, 64, 64, 255),
                    },
                    ColorStop {
                        position: 1.0,
                        color: RgbaColor::new(64, 96, 255, 255),
                    },
                ],
            });
            object
        };

        let repeat_object = make_object("repeat", "stroke-repeat", GradientRepeat::Repeat);
        let mirror_object = make_object("mirror", "stroke-mirror", GradientRepeat::Mirror);

        let image = render_overlay_rgba(&RenderRequest {
            project: &AnnotationProject {
                strokes: vec![stroke_repeat, stroke_mirror],
                glyph_objects: vec![repeat_object, mirror_object],
                ..AnnotationProject::default()
            },
            time: MediaTime::from_millis(0),
            preview_force_visible_batch: None,
            width: 160,
            height: 60,
            source_width: 160,
            source_height: 60,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let repeat_a = rgba_at(&image, 12, 20);
        let repeat_b = rgba_at(&image, 52, 20);
        assert!(
            (repeat_a[0] as u16) > (repeat_a[2] as u16) + 50
                && (repeat_b[0] as u16) > (repeat_b[2] as u16) + 50,
            "repeat は 1 周後に同じ色相へ戻ってほしい: {repeat_a:?} {repeat_b:?}"
        );

        let mirror_a = rgba_at(&image, 12, 34);
        let mirror_b = rgba_at(&image, 32, 34);
        assert!(
            (mirror_a[0] as u16) > (mirror_a[2] as u16) + 50
                && (mirror_b[2] as u16) > (mirror_b[0] as u16) + 20,
            "mirror は折り返し後に逆側の色相へ行ってほしい: {mirror_a:?} {mirror_b:?}"
        );
    }

    #[test]
    fn gradient_affects_base_stroke_but_effect_layers_remain_solid() {
        let stroke = line_stroke(
            "stroke-gradient-outline",
            0,
            Point2 { x: 10.0, y: 20.0 },
            Point2 { x: 110.0, y: 20.0 },
        );
        let mut object = demo_object("stroke-gradient-outline", 0);
        object.style.color_mode = ColorMode::LinearGradient;
        object.style.gradient = Some(LinearGradientStyle {
            scope: GradientSpace::GlyphObject,
            repeat: GradientRepeat::None,
            angle_degrees: 0.0,
            span_ratio: 1.0,
            offset_ratio: 0.0,
            stops: vec![
                ColorStop {
                    position: 0.0,
                    color: RgbaColor::new(255, 64, 64, 255),
                },
                ColorStop {
                    position: 1.0,
                    color: RgbaColor::new(64, 96, 255, 255),
                },
            ],
        });
        object.style.outline = pauseink_domain::OutlineStyle {
            enabled: true,
            width: 4.0,
            color: RgbaColor::new(12, 12, 12, 255),
        };

        let image = render_overlay_rgba(&RenderRequest {
            project: &AnnotationProject {
                strokes: vec![stroke],
                glyph_objects: vec![object],
                ..AnnotationProject::default()
            },
            time: MediaTime::from_millis(0),
            preview_force_visible_batch: None,
            width: 160,
            height: 48,
            source_width: 160,
            source_height: 48,
            background: RgbaColor::new(0, 0, 0, 0),
        })
        .expect("render should succeed");

        let outline_rgba = rgba_at(&image, 60, 14);
        let body_left = rgba_at(&image, 20, 20);
        let body_right = rgba_at(&image, 100, 20);
        assert!(
            outline_rgba[0] < 60 && outline_rgba[1] < 60 && outline_rgba[2] < 60,
            "outline は gradient ではなく単色のままでいてほしい: {outline_rgba:?}"
        );
        assert!(
            (body_left[0] as u16) > (body_left[2] as u16) + 30,
            "body 左側は warm tone の gradient を保ってほしい: {body_left:?}"
        );
        assert!(
            (body_right[2] as u16) > (body_right[0] as u16) + 30,
            "body 右側は cool tone の gradient を保ってほしい: {body_right:?}"
        );
    }
}
