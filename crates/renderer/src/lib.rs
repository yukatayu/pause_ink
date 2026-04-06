use std::collections::HashSet;

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

    let mut referenced_strokes = HashSet::new();

    for object in objects {
        for layer in [
            StrokeRenderLayer::DropShadow,
            StrokeRenderLayer::Glow,
            StrokeRenderLayer::Outline,
            StrokeRenderLayer::Base,
        ] {
            for stroke_id in &object.stroke_ids {
                referenced_strokes.insert(stroke_id.0.clone());

                let Some(stroke) = request
                    .project
                    .strokes
                    .iter()
                    .find(|candidate| candidate.id == *stroke_id)
                else {
                    continue;
                };

                let visibility =
                    evaluate_visibility(request.project, Some(object), stroke, request.time);
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

    for stroke in request
        .project
        .strokes
        .iter()
        .filter(|stroke| !referenced_strokes.contains(&stroke.id.0))
    {
        let visibility = evaluate_visibility(request.project, None, stroke, request.time);
        if !visibility.is_visible() {
            continue;
        }

        for layer in [
            StrokeRenderLayer::DropShadow,
            StrokeRenderLayer::Glow,
            StrokeRenderLayer::Outline,
            StrokeRenderLayer::Base,
        ] {
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
    project: &AnnotationProject,
    object: Option<&GlyphObject>,
    stroke: &Stroke,
    time: MediaTime,
) -> VisibilityState {
    if time < stroke.created_at {
        return VisibilityState::HIDDEN;
    }

    let entrance = evaluate_entrance(project, object, time);
    if !entrance.is_visible() {
        return entrance;
    }

    let clear = evaluate_clear(project, object, stroke, time);
    VisibilityState {
        alpha: (entrance.alpha * clear.alpha).clamp(0.0, 1.0),
        path_fraction: (entrance.path_fraction * clear.path_fraction).clamp(0.0, 1.0),
    }
}

fn evaluate_entrance(
    project: &AnnotationProject,
    object: Option<&GlyphObject>,
    time: MediaTime,
) -> VisibilityState {
    let Some(object) = object else {
        return VisibilityState::FULLY_VISIBLE;
    };

    if time < object.created_at {
        return VisibilityState::HIDDEN;
    }

    let duration_seconds = entrance_duration_seconds(project, object);
    let progress = normalized_progress(object.created_at, time, duration_seconds);

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
    if duration_seconds <= f64::EPSILON {
        return 1.0;
    }

    let elapsed = (media_time_seconds(current) - media_time_seconds(start)).max(0.0);
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
}
