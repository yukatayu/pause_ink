use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderlayMode {
    Outline,
    FaintFill,
    SlotBoxOnly,
    OutlineAndSlotBox,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateSettings {
    pub font_size: f32,
    pub tracking: f32,
    pub line_height: f32,
    pub kana_scale: f32,
    pub latin_scale: f32,
    pub punctuation_scale: f32,
    pub slope_degrees: f32,
    pub underlay_mode: UnderlayMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateSlot {
    pub grapheme: String,
    pub origin: Point,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateFontMetrics {
    pub ascent_ratio: f32,
    pub descent_ratio: f32,
    pub x_height_ratio: Option<f32>,
    pub cap_height_ratio: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemplateSlotVerticalMetrics {
    pub top_offset: f32,
    pub height: f32,
}

pub fn create_template_slots(
    text: &str,
    origin: Point,
    settings: &TemplateSettings,
) -> Vec<TemplateSlot> {
    let mut slots = Vec::new();
    let mut cursor_x = origin.x;
    let mut baseline_y = origin.y;
    let slope = settings.slope_degrees.to_radians().tan();

    for grapheme in text.graphemes(true) {
        if grapheme == "\n" {
            cursor_x = origin.x;
            baseline_y += settings.font_size * settings.line_height;
            continue;
        }

        let scale = template_grapheme_scale(grapheme, settings);
        let width = settings.font_size * scale;
        let vertical = template_slot_vertical_metrics(grapheme, settings.font_size, scale, None);
        let slope_offset_y = -((cursor_x - origin.x) * slope);

        slots.push(TemplateSlot {
            grapheme: grapheme.to_owned(),
            origin: Point::new(cursor_x, baseline_y + vertical.top_offset + slope_offset_y),
            width,
            height: vertical.height,
            scale,
        });

        cursor_x += width + settings.tracking;
    }

    slots
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidePlacement {
    pub cell_width: f32,
    pub cell_height: f32,
    pub slope_degrees: f32,
    pub next_cell_origin_x: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideLineKind {
    Main,
    Helper,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideLine {
    pub start: Point,
    pub end: Point,
    pub kind: GuideLineKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuideGeometry {
    pub horizontal_lines: Vec<GuideLine>,
    pub vertical_lines: Vec<GuideLine>,
}

pub fn build_guide_geometry(origin: Point, placement: GuidePlacement) -> GuideGeometry {
    let slope = placement.slope_degrees.to_radians().tan();
    let horizontal_width = placement.cell_width * 4.0;
    let horizontal_offsets = [
        (0.0, GuideLineKind::Main),
        (placement.cell_height * 0.25, GuideLineKind::Helper),
        (placement.cell_height * 0.5, GuideLineKind::Main),
        (placement.cell_height * 0.75, GuideLineKind::Helper),
        (placement.cell_height, GuideLineKind::Main),
    ];

    let horizontal_lines = horizontal_offsets
        .into_iter()
        .map(|(offset, kind)| {
            let start = Point::new(origin.x, origin.y + offset);
            let end = Point::new(
                origin.x + horizontal_width,
                origin.y + offset - horizontal_width * slope,
            );
            GuideLine { start, end, kind }
        })
        .collect();

    let next_cell_origin_x = placement
        .next_cell_origin_x
        .unwrap_or(origin.x + placement.cell_width);
    let vertical_offsets = [
        (0.0, GuideLineKind::Main),
        (placement.cell_width * 0.25, GuideLineKind::Helper),
        (placement.cell_width * 0.5, GuideLineKind::Main),
        (placement.cell_width * 0.75, GuideLineKind::Helper),
        (placement.cell_width, GuideLineKind::Main),
    ];

    let vertical_lines = vertical_offsets
        .into_iter()
        .map(|(offset, kind)| {
            let x = next_cell_origin_x + offset;
            let top = origin.y - (x - origin.x) * slope;
            let bottom = origin.y + placement.cell_height - (x - origin.x) * slope;
            GuideLine {
                start: Point::new(x, top),
                end: Point::new(x, bottom),
                kind,
            }
        })
        .collect();

    GuideGeometry {
        horizontal_lines,
        vertical_lines,
    }
}

pub fn guide_next_cell_origin_x(next_cell_anchor_x: f32, cell_width: f32, gap_ratio: f32) -> f32 {
    next_cell_anchor_x + cell_width * gap_ratio
}

pub fn guide_fallback_advance_step(cell_width: f32, gap_ratio: f32) -> f32 {
    (cell_width * (1.0 + gap_ratio)).max((cell_width * 0.1).max(1.0))
}

pub fn template_slot_vertical_metrics(
    grapheme: &str,
    font_size: f32,
    scale: f32,
    metrics: Option<TemplateFontMetrics>,
) -> TemplateSlotVerticalMetrics {
    let clamped_scale = scale.max(0.0);
    let fallback = TemplateSlotVerticalMetrics {
        top_offset: font_size * (1.0 - clamped_scale),
        height: font_size * clamped_scale,
    };

    let Some(metrics) = metrics else {
        return fallback;
    };

    let Some(normalized) = metrics.normalize_for_line_box() else {
        return fallback;
    };
    let mut ascender_height = match grapheme_vertical_kind(grapheme) {
        GraphemeVerticalKind::LatinLowercase => normalized.x_height_ratio,
        GraphemeVerticalKind::LatinLowercaseDescender => normalized.x_height_ratio,
        GraphemeVerticalKind::LatinUppercase => normalized.cap_height_ratio,
        GraphemeVerticalKind::FallbackLowerAligned => return fallback,
        GraphemeVerticalKind::FullBox => 1.0,
    } * font_size
        * clamped_scale;

    if ascender_height <= 0.0 {
        return fallback;
    }

    let descender_height = match grapheme_vertical_kind(grapheme) {
        GraphemeVerticalKind::LatinLowercaseDescender => {
            normalized.descender_ratio * font_size * clamped_scale
        }
        _ => 0.0,
    };
    let baseline_y = normalized.baseline_ratio * font_size;
    let top_offset = baseline_y - ascender_height;
    let height = ascender_height + descender_height;

    if !top_offset.is_finite() || !height.is_finite() || height <= 0.0 {
        return fallback;
    }

    ascender_height = ascender_height.max(0.0);
    TemplateSlotVerticalMetrics {
        top_offset: top_offset.max(0.0),
        height: (ascender_height + descender_height).max(1.0),
    }
}

pub fn template_grapheme_scale(grapheme: &str, settings: &TemplateSettings) -> f32 {
    let mut chars = grapheme.chars();
    let Some(first) = chars.next() else {
        return 1.0;
    };

    if is_kana(first) {
        settings.kana_scale
    } else if is_latin(first) {
        settings.latin_scale
    } else if is_punctuation(first) {
        settings.punctuation_scale
    } else {
        1.0
    }
}

fn is_kana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF | 0x31F0..=0x31FF | 0xFF66..=0xFF9D)
}

fn is_latin(ch: char) -> bool {
    ch.is_ascii_alphabetic()
}

fn is_punctuation(ch: char) -> bool {
    ch.is_ascii_punctuation() || matches!(ch, '。' | '、' | '！' | '？' | '・' | 'ー')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GraphemeVerticalKind {
    FullBox,
    LatinLowercase,
    LatinLowercaseDescender,
    LatinUppercase,
    FallbackLowerAligned,
}

impl TemplateFontMetrics {
    fn normalize_for_line_box(self) -> Option<NormalizedTemplateFontMetrics> {
        let ascent = self.ascent_ratio.max(0.0);
        let descent = self.descent_ratio.max(0.0);
        let total = ascent + descent;
        if total <= f32::EPSILON {
            return None;
        }

        let baseline_ratio = (ascent / total).clamp(0.0, 1.0);
        let descender_ratio = (descent / total).clamp(0.0, 1.0);
        let x_height_ratio = self
            .x_height_ratio
            .map(|value| (value.max(0.0) / total).clamp(0.0, 1.0))
            .unwrap_or(baseline_ratio);
        let cap_height_ratio = self
            .cap_height_ratio
            .map(|value| (value.max(0.0) / total).clamp(0.0, 1.0))
            .unwrap_or(baseline_ratio);
        Some(NormalizedTemplateFontMetrics {
            baseline_ratio,
            descender_ratio,
            x_height_ratio,
            cap_height_ratio,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NormalizedTemplateFontMetrics {
    baseline_ratio: f32,
    descender_ratio: f32,
    x_height_ratio: f32,
    cap_height_ratio: f32,
}

fn grapheme_vertical_kind(grapheme: &str) -> GraphemeVerticalKind {
    let Some(first) = grapheme.chars().find(|ch| !ch.is_ascii_whitespace()) else {
        return GraphemeVerticalKind::FallbackLowerAligned;
    };

    if is_kana(first) || !first.is_ascii() {
        return GraphemeVerticalKind::FullBox;
    }
    if first.is_ascii_uppercase() {
        return GraphemeVerticalKind::LatinUppercase;
    }
    if matches!(first, 'g' | 'j' | 'p' | 'q' | 'y') {
        return GraphemeVerticalKind::LatinLowercaseDescender;
    }
    if first.is_ascii_lowercase() {
        return GraphemeVerticalKind::LatinLowercase;
    }
    GraphemeVerticalKind::FallbackLowerAligned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_creation_is_grapheme_aware_and_scales_by_script() {
        let settings = TemplateSettings {
            font_size: 100.0,
            tracking: 10.0,
            line_height: 1.2,
            kana_scale: 1.1,
            latin_scale: 0.8,
            punctuation_scale: 0.6,
            slope_degrees: 0.0,
            underlay_mode: UnderlayMode::Outline,
        };

        let slots = create_template_slots("あA\u{0301}。", Point::new(10.0, 20.0), &settings);

        assert_eq!(slots.len(), 3);
        assert_eq!(slots[0].grapheme, "あ");
        assert_eq!(slots[0].scale, 1.1);
        assert_eq!(slots[1].grapheme, "A\u{0301}");
        assert_eq!(slots[1].scale, 0.8);
        assert_eq!(slots[2].grapheme, "。");
        assert_eq!(slots[2].scale, 0.6);
    }

    #[test]
    fn positive_slope_moves_later_slots_upward() {
        let settings = TemplateSettings {
            font_size: 100.0,
            tracking: 0.0,
            line_height: 1.0,
            kana_scale: 1.0,
            latin_scale: 1.0,
            punctuation_scale: 1.0,
            slope_degrees: 12.0,
            underlay_mode: UnderlayMode::Outline,
        };

        let slots = create_template_slots("AB", Point::new(0.0, 0.0), &settings);

        assert!(slots[1].origin.y < slots[0].origin.y);
    }

    #[test]
    fn scaled_latin_without_metrics_is_bottom_aligned_in_line_box() {
        let settings = TemplateSettings {
            font_size: 100.0,
            tracking: 0.0,
            line_height: 1.0,
            kana_scale: 1.0,
            latin_scale: 0.6,
            punctuation_scale: 1.0,
            slope_degrees: 0.0,
            underlay_mode: UnderlayMode::Outline,
        };

        let slots = create_template_slots("A", Point::new(12.0, 24.0), &settings);

        assert!(
            (slots[0].origin.y - 64.0).abs() < 0.01,
            "metrics が無い fallback でも縮小英字は line box の下側へ寄せたい"
        );
        assert!((slots[0].height - 60.0).abs() < 0.01);
    }

    #[test]
    fn metrics_based_alignment_keeps_descender_letters_below_x_height_letters() {
        let metrics = TemplateFontMetrics {
            ascent_ratio: 0.8,
            descent_ratio: 0.2,
            x_height_ratio: Some(0.5),
            cap_height_ratio: Some(0.7),
        };
        let x = template_slot_vertical_metrics("x", 100.0, 0.6, Some(metrics));
        let y = template_slot_vertical_metrics("y", 100.0, 0.6, Some(metrics));

        assert!((x.top_offset - y.top_offset).abs() < 0.01);
        assert!(y.height > x.height);
        assert!(((x.top_offset + x.height) - 80.0).abs() < 0.01);
        assert!(((y.top_offset + y.height) - 92.0).abs() < 0.01);
    }

    #[test]
    fn guide_geometry_contains_three_main_lines_and_two_helpers_per_axis() {
        let geometry = build_guide_geometry(
            Point::new(100.0, 200.0),
            GuidePlacement {
                cell_width: 80.0,
                cell_height: 100.0,
                slope_degrees: 8.0,
                next_cell_origin_x: None,
            },
        );

        assert_eq!(geometry.horizontal_lines.len(), 5);
        assert_eq!(geometry.vertical_lines.len(), 5);
        assert_eq!(
            geometry
                .horizontal_lines
                .iter()
                .filter(|line| line.kind == GuideLineKind::Main)
                .count(),
            3
        );
        assert_eq!(
            geometry
                .vertical_lines
                .iter()
                .filter(|line| line.kind == GuideLineKind::Helper)
                .count(),
            2
        );
    }

    #[test]
    fn template_grapheme_scale_matches_script_categories() {
        let settings = TemplateSettings {
            font_size: 96.0,
            tracking: 0.0,
            line_height: 1.0,
            kana_scale: 1.2,
            latin_scale: 0.8,
            punctuation_scale: 0.6,
            slope_degrees: 0.0,
            underlay_mode: UnderlayMode::Outline,
        };

        assert_eq!(template_grapheme_scale("あ", &settings), 1.2);
        assert_eq!(template_grapheme_scale("V", &settings), 0.8);
        assert_eq!(template_grapheme_scale("。", &settings), 0.6);
        assert_eq!(template_grapheme_scale("感", &settings), 1.0);
    }

    #[test]
    fn guide_geometry_can_move_only_the_next_character_vertical_set() {
        let geometry = build_guide_geometry(
            Point::new(100.0, 200.0),
            GuidePlacement {
                cell_width: 80.0,
                cell_height: 100.0,
                slope_degrees: 10.0,
                next_cell_origin_x: Some(260.0),
            },
        );

        let first_vertical = geometry
            .vertical_lines
            .iter()
            .find(|line| line.kind == GuideLineKind::Main)
            .expect("main vertical");
        let top_horizontal = geometry.horizontal_lines.first().expect("top horizontal");

        assert!((first_vertical.start.x - 260.0).abs() < 0.01);
        assert!((top_horizontal.start.x - 100.0).abs() < 0.01);
        assert!((top_horizontal.start.y - 200.0).abs() < 0.01);
    }

    #[test]
    fn guide_gap_offset_uses_cell_width_ratio() {
        assert!((guide_next_cell_origin_x(160.0, 60.0, 0.25) - 175.0).abs() < 0.01);
        assert!((guide_next_cell_origin_x(160.0, 60.0, -0.5) - 130.0).abs() < 0.01);
    }

    #[test]
    fn guide_fallback_advance_step_stays_positive_even_for_negative_gap() {
        assert!((guide_fallback_advance_step(40.0, -0.5) - 20.0).abs() < 0.01);
        assert!(guide_fallback_advance_step(40.0, -2.0) > 0.0);
    }
}
