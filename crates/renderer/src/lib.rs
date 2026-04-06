use std::collections::{HashMap, HashSet};

use pauseink_domain::{
    AnnotationProject, BlendMode, ClearKind, DerivedStrokePath, GeometryTransform, GlyphObject,
    MediaTime, Point2, RgbaColor, Stroke, StrokeSample, StyleSnapshot, TimeBase,
};
use tiny_skia::{
    BlendMode as SkBlendMode, Paint, PathBuilder, Pixmap, Stroke as SkStroke, Transform,
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
    let layers = [
        StrokeRenderLayer::DropShadow,
        StrokeRenderLayer::Glow,
        StrokeRenderLayer::Outline,
        StrokeRenderLayer::Base,
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

    for layer in layers {
        for object in &objects {
            for stroke_id in &object.stroke_ids {
                let Some(stroke) = strokes_by_id.get(stroke_id.0.as_str()).copied() else {
                    continue;
                };

                let visibility = evaluate_visibility(request, Some(object), stroke, request.time);
                if !visibility.is_visible() {
                    continue;
                }

                render_stroke_layer(
                    &mut pixmap,
                    stroke,
                    &object.style,
                    &object.transform,
                    visibility,
                    render_scale,
                    layer,
                );
            }
        }
    }

    for layer in layers {
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
                stroke,
                &stroke.style,
                &GeometryTransform::default(),
                visibility,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrokeRenderLayer {
    DropShadow,
    Glow,
    Outline,
    Base,
}

fn render_stroke_layer(
    pixmap: &mut Pixmap,
    stroke: &Stroke,
    style: &StyleSnapshot,
    transform: &GeometryTransform,
    visibility: VisibilityState,
    render_scale: RenderScale,
    layer: StrokeRenderLayer,
) {
    let points = transformed_visible_points(stroke, transform, visibility.path_fraction)
        .into_iter()
        .map(|point| Point2 {
            x: point.x * render_scale.x,
            y: point.y * render_scale.y,
        })
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
            draw_stroked_path(
                pixmap,
                &path,
                (style.thickness * render_scale.stroke).max(1.0),
                style.color,
                style.opacity * visibility.alpha,
                style.blend_mode,
            );
        }
    }
}

fn transformed_visible_points(
    stroke: &Stroke,
    transform: &GeometryTransform,
    path_fraction: f32,
) -> Vec<Point2> {
    let source = if !stroke.derived_path.points.is_empty() {
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
    };

    partial_polyline(&source, path_fraction)
        .into_iter()
        .map(|point| apply_transform(point, transform))
        .collect()
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

fn partial_polyline(points: &[Point2], fraction: f32) -> Vec<Point2> {
    if points.is_empty() || fraction >= 1.0 {
        return points.to_vec();
    }

    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 {
        return points[..1].to_vec();
    }

    let total_length = polyline_length(points);
    if total_length <= f32::EPSILON {
        return points.to_vec();
    }

    let target_length = total_length * fraction;
    let mut accumulated = 0.0;
    let mut partial = vec![points[0]];

    for segment in points.windows(2) {
        let start = segment[0];
        let end = segment[1];
        let segment_length = distance(start, end);
        if accumulated + segment_length >= target_length {
            let remaining = (target_length - accumulated).max(0.0);
            let t = if segment_length <= f32::EPSILON {
                0.0
            } else {
                remaining / segment_length
            };
            partial.push(Point2 {
                x: start.x + (end.x - start.x) * t,
                y: start.y + (end.y - start.y) * t,
            });
            return partial;
        }

        accumulated += segment_length;
        partial.push(end);
    }

    points.to_vec()
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
        AnnotationProject, ClearEvent, ClearEventId, EntranceBehavior, EntranceKind, GlyphObject,
        GlyphObjectId, MediaDuration, MediaTime, Point2, Stroke, StrokeId, StrokeSample,
        StyleSnapshot,
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
}
