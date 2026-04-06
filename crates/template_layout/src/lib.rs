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
        let slope_offset_y = -((cursor_x - origin.x) * slope);

        slots.push(TemplateSlot {
            grapheme: grapheme.to_owned(),
            origin: Point::new(cursor_x, baseline_y + slope_offset_y),
            width,
            height: settings.font_size * scale,
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
}
