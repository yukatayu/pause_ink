use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke as EguiStroke, Vec2};
use pauseink_app::AppSession;
use pauseink_export::{
    execute_export_with_settings, plan_export, ConcreteExportSettings, ExportOutputRequest,
    ExportRequest,
};
use pauseink_fonts::{
    discover_local_font_families, fetch_google_font_to_cache, google_font_cache_file,
    google_font_is_cached, load_font_family, load_ui_font_candidates,
};
use pauseink_media::{
    canvas_point_to_frame, default_platform_id, discover_runtime, fit_frame_to_canvas,
    frame_point_to_canvas, sidecar_runtime_dir, CanvasRect, CanvasSize, FfprobeMediaProvider,
    MediaProvider, MediaRuntime, PreviewFrame, RuntimeCapabilities,
};
use pauseink_portable_fs::{
    clear_directory_contents, directory_size, load_settings_or_default, portable_root_from_env,
    save_settings_to_file, PortablePaths, Settings,
};
use pauseink_presets_core::{
    load_base_style_presets_from_dir, BaseStylePreset, ExportCatalog, OutputKind, RuntimeTier,
};
use pauseink_renderer::{render_overlay_rgba, RenderRequest};
use pauseink_template_layout::{
    build_guide_geometry, template_grapheme_scale, GuideGeometry, GuideLineKind, GuidePlacement,
    Point, TemplateSettings, UnderlayMode,
};
use unicode_segmentation::UnicodeSegmentation;

const SYSTEM_DEFAULT_FONT_FAMILY_LABEL: &str = "システム既定";

fn main() -> Result<()> {
    let executable_dir = std::env::current_exe()?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().expect("current dir should resolve"));
    let portable_paths = PortablePaths::from_root(portable_root_from_env(&executable_dir));
    portable_paths.ensure_exists()?;

    let settings = load_settings_or_default(&portable_paths)?;

    let runtime = discover_runtime(&portable_paths.runtime_dir, &default_platform_id(), true).ok();
    let options = eframe::NativeOptions::default();
    let portable_paths_for_app = portable_paths.clone();
    let settings_for_app = settings.clone();
    let runtime_for_app = runtime.clone();

    eframe::run_native(
        "PauseInk",
        options,
        Box::new(move |cc| {
            configure_egui_fonts(
                &cc.egui_ctx,
                &portable_paths_for_app,
                &settings_for_app,
                None,
            );
            Ok(Box::new(DesktopApp::new(
                portable_paths_for_app.clone(),
                settings_for_app.clone(),
                runtime_for_app.clone(),
            )))
        }),
    )?;
    Ok(())
}

fn font_data_key(prefix: &str, family_name: &str) -> String {
    let mut key = String::with_capacity(prefix.len() + family_name.len() + 1);
    key.push_str(prefix);
    key.push('-');
    for ch in family_name.chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch.to_ascii_lowercase());
        } else {
            key.push('_');
        }
    }
    key
}

fn configure_egui_fonts(
    ctx: &egui::Context,
    portable_paths: &PortablePaths,
    settings: &Settings,
    template_font_family: Option<&str>,
) {
    let mut font_dirs = vec![portable_paths.google_fonts_cache_dir()];
    font_dirs.extend(settings.local_font_dirs.clone());

    let mut definitions = egui::FontDefinitions::default();
    if let Some(ui_font) = load_ui_font_candidates(&font_dirs, &settings.google_fonts.families, 1)
        .into_iter()
        .next()
    {
        let font_name = font_data_key("pauseink-ui", &ui_font.family_name);
        definitions.font_data.insert(
            font_name.clone(),
            egui::FontData::from_owned(ui_font.bytes).into(),
        );

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            if let Some(entries) = definitions.families.get_mut(&family) {
                entries.retain(|entry| entry != &font_name);
                entries.insert(0, font_name.clone());
            }
        }
    }

    if let Some(template_family) = template_font_family
        .filter(|family| !family.trim().is_empty() && *family != SYSTEM_DEFAULT_FONT_FAMILY_LABEL)
    {
        if let Some(loaded_font) = load_font_family(&font_dirs, template_family) {
            let font_name = font_data_key("pauseink-template", &loaded_font.family_name);
            definitions.font_data.insert(
                font_name.clone(),
                egui::FontData::from_owned(loaded_font.bytes).into(),
            );
            definitions.families.insert(
                egui::FontFamily::Name(loaded_font.family_name.clone().into()),
                vec![font_name],
            );
        }
    }

    ctx.set_fonts(definitions);
}

fn frame_canvas_rect(frame_rect: Rect) -> CanvasRect {
    CanvasRect {
        x: 0.0,
        y: 0.0,
        width: frame_rect.width(),
        height: frame_rect.height(),
    }
}

fn pointer_position_to_frame_point(
    pointer_position: Pos2,
    frame_rect: Rect,
    frame_width: u32,
    frame_height: u32,
) -> Option<pauseink_domain::Point2> {
    canvas_point_to_frame(
        pauseink_domain::Point2 {
            x: pointer_position.x - frame_rect.left(),
            y: pointer_position.y - frame_rect.top(),
        },
        frame_canvas_rect(frame_rect),
        frame_width,
        frame_height,
    )
}

fn frame_point_to_screen_position(
    frame_point: pauseink_domain::Point2,
    frame_rect: Rect,
    frame_width: u32,
    frame_height: u32,
) -> Option<Pos2> {
    let local = frame_point_to_canvas(
        frame_point,
        frame_canvas_rect(frame_rect),
        frame_width,
        frame_height,
    )?;
    Some(Pos2::new(
        frame_rect.left() + local.x,
        frame_rect.top() + local.y,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BottomTab {
    Outline,
    PageEvents,
    ExportQueue,
    Logs,
}

#[derive(Debug, Clone)]
struct TemplatePreviewState {
    text: String,
    settings: TemplateSettings,
    font_family: String,
    placement_armed: bool,
    placed_slots: Option<Vec<pauseink_template_layout::TemplateSlot>>,
    current_slot_index: usize,
    slot_object_ids: Vec<Option<pauseink_domain::GlyphObjectId>>,
}

impl Default for TemplatePreviewState {
    fn default() -> Self {
        Self {
            text: "あA。".to_owned(),
            settings: TemplateSettings {
                font_size: 96.0,
                tracking: 16.0,
                line_height: 1.2,
                kana_scale: 1.0,
                latin_scale: 0.85,
                punctuation_scale: 0.7,
                slope_degrees: 0.0,
                underlay_mode: UnderlayMode::OutlineAndSlotBox,
            },
            font_family: SYSTEM_DEFAULT_FONT_FAMILY_LABEL.to_owned(),
            placement_armed: false,
            placed_slots: None,
            current_slot_index: 0,
            slot_object_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GuideOverlayState {
    horizontal_origin: Point,
    cell_width: f32,
    cell_height: f32,
    next_cell_origin_x: f32,
}

impl GuideOverlayState {
    fn from_reference_bounds(min: pauseink_domain::Point2, max: pauseink_domain::Point2) -> Self {
        let cell_width = (max.x - min.x).max(40.0);
        let cell_height = (max.y - min.y).max(48.0);
        Self {
            horizontal_origin: Point::new(min.x, min.y),
            cell_width,
            cell_height,
            next_cell_origin_x: min.x + cell_width,
        }
    }

    fn build_geometry(&self, slope_degrees: f32) -> GuideGeometry {
        build_guide_geometry(
            self.horizontal_origin,
            GuidePlacement {
                cell_width: self.cell_width,
                cell_height: self.cell_height,
                slope_degrees,
                next_cell_origin_x: Some(self.next_cell_origin_x),
            },
        )
    }

    fn advance_to_next_from_bounds(
        &mut self,
        bounds: Option<(pauseink_domain::Point2, pauseink_domain::Point2)>,
    ) {
        if let Some((min, max)) = bounds {
            self.cell_width = (max.x - min.x).max(40.0);
            self.next_cell_origin_x = min.x + self.cell_width;
        } else {
            self.next_cell_origin_x += self.cell_width;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TemplateGraphemeLayout {
    grapheme: String,
    natural_start_x: f32,
    width: f32,
    scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportOutputMode {
    Composite,
    Transparent,
}

#[derive(Debug, Clone)]
struct ExportJobRecord {
    summary: String,
    output_path: PathBuf,
    status: String,
    software_fallback_used: bool,
}

struct PendingExportJob {
    receiver: Receiver<Result<pauseink_export::ExportExecutionResult, String>>,
    summary: String,
    output_path: PathBuf,
}

#[derive(Debug, Clone)]
struct ExportState {
    catalog: Option<ExportCatalog>,
    family_id: String,
    profile_id: String,
    output_mode: ExportOutputMode,
    custom_settings: Option<ConcreteExportSettings>,
    jobs: Vec<ExportJobRecord>,
}

impl Default for ExportState {
    fn default() -> Self {
        Self {
            catalog: None,
            family_id: String::new(),
            profile_id: String::new(),
            output_mode: ExportOutputMode::Composite,
            custom_settings: None,
            jobs: Vec::new(),
        }
    }
}

struct DesktopApp {
    session: AppSession,
    portable_paths: PortablePaths,
    settings: Settings,
    runtime: Option<MediaRuntime>,
    provider: Option<FfprobeMediaProvider>,
    runtime_capabilities: Option<RuntimeCapabilities>,
    runtime_status: String,
    logs: Vec<String>,
    local_font_families: Vec<String>,
    style_presets: Vec<BaseStylePreset>,
    selected_style_preset_id: String,
    template: TemplatePreviewState,
    export: ExportState,
    guide_state: Option<GuideOverlayState>,
    guide_geometry: Option<GuideGeometry>,
    last_committed_object_bounds: Option<(pauseink_domain::Point2, pauseink_domain::Point2)>,
    bottom_tab: BottomTab,
    preview_texture: Option<egui::TextureHandle>,
    preview_key: Option<(PathBuf, i64, u32, u32)>,
    overlay_texture: Option<egui::TextureHandle>,
    overlay_key: Option<(i64, usize, usize, u32, u32, u32, u32)>,
    canvas_drag_active: bool,
    guide_capture_armed: bool,
    guide_modifier_was_down: bool,
    guide_modifier_used_for_stroke: bool,
    recovery_prompt_open: bool,
    preferences_open: bool,
    cache_manager_open: bool,
    runtime_diagnostics_open: bool,
    font_config_dirty: bool,
    google_font_input: String,
    pending_export: Option<PendingExportJob>,
    last_update_at: Instant,
    last_autosave_at: Instant,
}

impl DesktopApp {
    fn new(
        portable_paths: PortablePaths,
        settings: Settings,
        runtime: Option<MediaRuntime>,
    ) -> Self {
        let recovery_prompt_open = portable_paths.autosave_file("recovery_latest").exists();
        let provider = runtime.clone().map(FfprobeMediaProvider::new);
        let runtime_capabilities = provider
            .as_ref()
            .and_then(|provider| provider.capabilities().ok());
        let runtime_status = runtime
            .as_ref()
            .map(|runtime| {
                format!(
                    "ランタイム: {} ({:?})",
                    runtime
                        .build_summary
                        .clone()
                        .unwrap_or_else(|| runtime.ffmpeg_path.display().to_string()),
                    runtime.origin
                )
            })
            .unwrap_or_else(|| "ランタイム: 未検出".to_owned());
        let mut font_dirs = vec![portable_paths.google_fonts_cache_dir()];
        font_dirs.extend(settings.local_font_dirs.clone());
        let local_font_families = discover_local_font_families(&font_dirs);
        let export_catalog = ExportCatalog::load_builtin_from_dir(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/export_profiles"),
        )
        .ok();
        let style_presets = load_base_style_presets_from_dir(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/style_presets"),
        )
        .unwrap_or_default();
        let mut session = AppSession::with_history_limit(settings.history_depth);
        session.active_style.stabilization_strength =
            (settings.stroke_stabilization_default as f32 / 100.0).clamp(0.0, 1.0);
        let mut export = ExportState {
            catalog: export_catalog,
            ..ExportState::default()
        };
        initialize_export_selection(&mut export);

        Self {
            session,
            portable_paths,
            settings,
            runtime,
            provider,
            runtime_capabilities,
            runtime_status,
            logs: Vec::new(),
            local_font_families,
            selected_style_preset_id: style_presets
                .first()
                .map(|preset| preset.id.clone())
                .unwrap_or_default(),
            style_presets,
            template: TemplatePreviewState::default(),
            export,
            guide_state: None,
            guide_geometry: None,
            last_committed_object_bounds: None,
            bottom_tab: BottomTab::Outline,
            preview_texture: None,
            preview_key: None,
            overlay_texture: None,
            overlay_key: None,
            canvas_drag_active: false,
            guide_capture_armed: false,
            guide_modifier_was_down: false,
            guide_modifier_used_for_stroke: false,
            recovery_prompt_open,
            preferences_open: false,
            cache_manager_open: false,
            runtime_diagnostics_open: false,
            font_config_dirty: true,
            google_font_input: String::new(),
            pending_export: None,
            last_update_at: Instant::now(),
            last_autosave_at: Instant::now(),
        }
    }

    fn push_log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 200 {
            let overflow = self.logs.len() - 200;
            self.logs.drain(0..overflow);
        }
    }

    fn rebuild_local_font_families(&mut self) {
        let previous_selection = self.template.font_family.clone();
        self.local_font_families = discover_local_font_families(&self.available_font_dirs());
        if previous_selection != SYSTEM_DEFAULT_FONT_FAMILY_LABEL
            && !self
                .local_font_families
                .iter()
                .any(|family| family == &previous_selection)
        {
            self.template.font_family = SYSTEM_DEFAULT_FONT_FAMILY_LABEL.to_owned();
            self.push_log(format!(
                "選択中のテンプレート font `{previous_selection}` が見つからないため、システム既定へ戻しました。"
            ));
        }
        self.font_config_dirty = true;
    }

    fn available_font_dirs(&self) -> Vec<PathBuf> {
        let mut font_dirs = vec![self.portable_paths.google_fonts_cache_dir()];
        font_dirs.extend(self.settings.local_font_dirs.clone());
        font_dirs
    }

    fn maybe_apply_egui_fonts(&mut self, ctx: &egui::Context) {
        if !self.font_config_dirty {
            return;
        }
        configure_egui_fonts(
            ctx,
            &self.portable_paths,
            &self.settings,
            Some(&self.template.font_family),
        );
        self.font_config_dirty = false;
    }

    fn template_font_id(&self, size: f32) -> egui::FontId {
        if self.template.font_family == SYSTEM_DEFAULT_FONT_FAMILY_LABEL {
            egui::FontId::proportional(size)
        } else {
            egui::FontId::new(
                size,
                egui::FontFamily::Name(self.template.font_family.clone().into()),
            )
        }
    }

    fn layout_template_line(&self, ctx: &egui::Context, line: &str) -> Vec<TemplateGraphemeLayout> {
        let graphemes = line.graphemes(true).collect::<Vec<_>>();
        if graphemes.is_empty() {
            return Vec::new();
        }

        let mut job = egui::text::LayoutJob::default();
        job.text = line.to_owned();
        job.wrap.max_width = f32::INFINITY;
        job.wrap.max_rows = 1;

        let mut byte_offset = 0usize;
        let mut run_start = 0usize;
        while run_start < graphemes.len() {
            let run_scale = template_grapheme_scale(graphemes[run_start], &self.template.settings);
            let mut run_end = run_start + 1;
            let mut run_bytes = graphemes[run_start].len();

            while run_end < graphemes.len() {
                let next_scale =
                    template_grapheme_scale(graphemes[run_end], &self.template.settings);
                if (next_scale - run_scale).abs() > f32::EPSILON {
                    break;
                }
                run_bytes += graphemes[run_end].len();
                run_end += 1;
            }

            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: byte_offset..(byte_offset + run_bytes),
                format: egui::TextFormat {
                    font_id: self.template_font_id(self.template.settings.font_size * run_scale),
                    color: Color32::PLACEHOLDER,
                    ..Default::default()
                },
            });
            byte_offset += run_bytes;
            run_start = run_end;
        }

        let galley = ctx.fonts_mut(|fonts| fonts.layout_job(job));
        let Some(row) = galley.rows.first() else {
            return Vec::new();
        };

        let mut glyph_cursor = 0usize;
        let mut layouts = Vec::with_capacity(graphemes.len());
        for grapheme in graphemes {
            let glyph_count = grapheme.chars().count();
            if glyph_count == 0 || glyph_cursor >= row.glyphs.len() {
                continue;
            }
            let glyph_end = (glyph_cursor + glyph_count).min(row.glyphs.len());
            let glyphs = &row.glyphs[glyph_cursor..glyph_end];
            if glyphs.is_empty() {
                continue;
            }
            let start_x = glyphs.first().map(|glyph| glyph.pos.x).unwrap_or_default();
            let end_x = glyphs
                .iter()
                .fold(start_x, |max_x, glyph| max_x.max(glyph.max_x()));
            layouts.push(TemplateGraphemeLayout {
                grapheme: grapheme.to_owned(),
                natural_start_x: start_x,
                width: (end_x - start_x).max(1.0),
                scale: template_grapheme_scale(grapheme, &self.template.settings),
            });
            glyph_cursor = glyph_end;
        }

        if let Some(first_start_x) = layouts.first().map(|layout| layout.natural_start_x) {
            for layout in &mut layouts {
                layout.natural_start_x -= first_start_x;
            }
        }

        layouts
    }

    fn template_slots_at_origin(
        &self,
        ctx: &egui::Context,
        origin: Point,
    ) -> Vec<pauseink_template_layout::TemplateSlot> {
        let mut slots = Vec::new();
        let slope = self.template.settings.slope_degrees.to_radians().tan();
        let mut baseline_y = origin.y;

        for line in self.template.text.split('\n') {
            for (index, layout) in self.layout_template_line(ctx, line).into_iter().enumerate() {
                let slot_origin_x = origin.x
                    + layout.natural_start_x
                    + self.template.settings.tracking * index as f32;
                let slope_offset_y = -((slot_origin_x - origin.x) * slope);
                slots.push(pauseink_template_layout::TemplateSlot {
                    grapheme: layout.grapheme,
                    origin: Point::new(slot_origin_x, baseline_y + slope_offset_y),
                    width: layout.width.max(12.0),
                    height: (self.template.settings.font_size * layout.scale).max(12.0),
                    scale: layout.scale,
                });
            }
            baseline_y += self.template.settings.font_size * self.template.settings.line_height;
        }

        slots
    }

    fn refresh_guide_geometry(&mut self) {
        self.guide_geometry = self
            .guide_state
            .map(|guide_state| guide_state.build_geometry(self.settings.guide_slope_degrees));
    }

    fn capture_guide_from_object(&mut self, object_id: &pauseink_domain::GlyphObjectId) {
        if let Some((min, max)) = self.session.object_bounds(object_id) {
            self.last_committed_object_bounds = Some((min, max));
            self.guide_state = Some(GuideOverlayState::from_reference_bounds(min, max));
            self.refresh_guide_geometry();
            self.push_log("ガイド基準を更新しました。");
        }
    }

    fn advance_guide_to_next_character(&mut self) {
        let Some(guide_state) = &mut self.guide_state else {
            return;
        };
        guide_state.advance_to_next_from_bounds(self.last_committed_object_bounds);
        self.refresh_guide_geometry();
        self.push_log("ガイド縦線を次文字位置へ進めました。");
    }

    fn handle_guide_modifier_tap(&mut self, ctx: &egui::Context) {
        let modifier_active = self.guide_modifier_active(ctx);
        if modifier_active && !self.guide_modifier_was_down {
            self.guide_modifier_used_for_stroke = false;
        }
        if !modifier_active
            && self.guide_modifier_was_down
            && !self.guide_modifier_used_for_stroke
            && !ctx.egui_wants_keyboard_input()
            && !self.template.placement_armed
        {
            self.advance_guide_to_next_character();
        }
        self.guide_modifier_was_down = modifier_active;
    }

    fn perform_undo(&mut self) {
        if let Err(error) = self.session.undo() {
            self.push_log(format!("undo 失敗: {error:#}"));
        }
    }

    fn perform_redo(&mut self) {
        if let Err(error) = self.session.redo() {
            self.push_log(format!("redo 失敗: {error:#}"));
        }
    }

    fn handle_global_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
        let redo_shift_shortcut = egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            egui::Key::Z,
        );
        let redo_y_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Y);

        let undo = ctx.input_mut(|input| input.consume_shortcut(&undo_shortcut));
        let redo_shift = ctx.input_mut(|input| input.consume_shortcut(&redo_shift_shortcut));
        let redo_y = ctx.input_mut(|input| input.consume_shortcut(&redo_y_shortcut));

        if undo {
            self.perform_undo();
        }
        if redo_shift || redo_y {
            self.perform_redo();
        }
    }

    fn apply_selected_style_preset(&mut self) {
        let Some(preset) = self
            .style_presets
            .iter()
            .find(|preset| preset.id == self.selected_style_preset_id)
            .cloned()
        else {
            return;
        };

        if let Some(thickness) = preset.thickness {
            self.session.active_style.thickness = thickness;
        }
        if let Some(color) = preset.color_rgba {
            self.session.active_style.color =
                pauseink_domain::RgbaColor::new(color[0], color[1], color[2], color[3]);
            self.session.active_style.opacity = 1.0;
        }
        self.push_log(format!("style preset 適用: {}", preset.display_name));
    }

    fn refresh_runtime_capabilities(&mut self) {
        let Some(provider) = self.provider.as_ref() else {
            self.runtime_capabilities = None;
            return;
        };

        match provider.capabilities() {
            Ok(capabilities) => {
                self.runtime_capabilities = Some(capabilities.clone());
                self.push_log(format!(
                    "runtime capability 更新: video={} / audio={} / muxer={} / hwaccel={}",
                    capabilities.video_encoders.len(),
                    capabilities.audio_encoders.len(),
                    capabilities.muxers.len(),
                    capabilities.hwaccels.len()
                ));
            }
            Err(error) => self.push_log(format!("runtime capability 取得失敗: {error}")),
        }
    }

    fn sync_export_state(&mut self) {
        initialize_export_selection(&mut self.export);
        let Some(catalog) = self.export.catalog.as_ref() else {
            return;
        };
        let Some(family) = catalog.family(&self.export.family_id) else {
            return;
        };

        if matches!(family.output_kind, OutputKind::CompositeOnly) {
            self.export.output_mode = ExportOutputMode::Composite;
        }
        if !family.supports_alpha
            && matches!(self.export.output_mode, ExportOutputMode::Transparent)
        {
            self.export.output_mode = ExportOutputMode::Composite;
        }
    }

    fn planned_export_settings(&mut self) -> Result<ConcreteExportSettings, String> {
        self.sync_export_state();
        let catalog = self
            .export
            .catalog
            .as_ref()
            .ok_or_else(|| "export catalog を読み込めません".to_owned())?;
        let snapshot = self.session.build_export_snapshot();
        let mut settings = plan_export(
            catalog,
            &ExportRequest {
                family_id: self.export.family_id.clone(),
                profile_id: self.export.profile_id.clone(),
                width: snapshot.width,
                height: snapshot.height,
                frame_rate: snapshot.frame_rate,
                has_audio: snapshot.has_audio,
                requires_alpha: matches!(self.export.output_mode, ExportOutputMode::Transparent),
            },
            self.runtime_capabilities.as_ref(),
        )
        .map_err(|error| error.to_string())?;

        if settings.profile.id == "custom" {
            let needs_seed = self
                .export
                .custom_settings
                .as_ref()
                .map(|custom| {
                    custom.family.id != settings.family.id
                        || custom.selected_bucket_id != settings.selected_bucket_id
                        || custom.audio_enabled != settings.audio_enabled
                })
                .unwrap_or(true);
            if needs_seed {
                self.export.custom_settings = Some(settings.clone());
            }
            if let Some(custom) = &self.export.custom_settings {
                settings = custom.clone();
            }
        }

        Ok(settings)
    }

    fn poll_pending_export(&mut self) {
        let Some(pending) = self.pending_export.as_ref() else {
            return;
        };

        match pending.receiver.try_recv() {
            Ok(result) => {
                let summary = pending.summary.clone();
                let output_path = pending.output_path.clone();
                let record = match result {
                    Ok(result) => {
                        self.push_log(format!("export 完了: {}", output_path.display()));
                        ExportJobRecord {
                            summary,
                            output_path,
                            status: "完了".to_owned(),
                            software_fallback_used: result.software_fallback_used,
                        }
                    }
                    Err(error) => {
                        self.push_log(format!("export 失敗: {error}"));
                        ExportJobRecord {
                            summary,
                            output_path,
                            status: format!("失敗: {error}"),
                            software_fallback_used: false,
                        }
                    }
                };
                self.export.jobs.insert(0, record);
                if self.export.jobs.len() > 20 {
                    self.export.jobs.truncate(20);
                }
                self.pending_export = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.push_log("export worker が切断されました。");
                self.pending_export = None;
            }
        }
    }

    fn start_export(&mut self) {
        if self.pending_export.is_some() {
            self.push_log("export はすでに実行中です。");
            return;
        }

        let Some(runtime) = self.runtime.clone() else {
            self.push_log("runtime 未検出のため export を開始できません。");
            return;
        };
        let capabilities = self.runtime_capabilities.clone().unwrap_or_default();
        let settings = match self.planned_export_settings() {
            Ok(settings) => settings,
            Err(error) => {
                self.push_log(format!("export 設定エラー: {error}"));
                return;
            }
        };
        let output_path = match self.pick_export_output_path(&settings) {
            Some(path) => path,
            None => return,
        };

        let snapshot = self.session.build_export_snapshot();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let working_directory = self
            .portable_paths
            .temp_dir
            .join(format!("export_job_{timestamp}"));
        let request = ExportOutputRequest {
            output_path: output_path.clone(),
            transparent: matches!(self.export.output_mode, ExportOutputMode::Transparent),
            working_directory: working_directory.clone(),
            prefer_hardware: self.settings.media_hwaccel_enabled,
        };
        let summary = format!(
            "{} / {} / {}",
            settings.family.display_name,
            settings.profile.display_name,
            output_path.display()
        );
        let (sender, receiver) = mpsc::channel();

        std::thread::spawn(move || {
            let result = execute_export_with_settings(
                &runtime,
                &capabilities,
                &snapshot,
                &settings,
                &request,
            )
            .map_err(|error| error.to_string());
            let _ = fs::remove_dir_all(&working_directory);
            let _ = sender.send(result);
        });

        self.push_log(format!("export 開始: {}", output_path.display()));
        self.pending_export = Some(PendingExportJob {
            receiver,
            summary,
            output_path,
        });
        self.bottom_tab = BottomTab::ExportQueue;
    }

    fn pick_export_output_path(&self, settings: &ConcreteExportSettings) -> Option<PathBuf> {
        if settings.family.id == "png_sequence_rgba" {
            return rfd::FileDialog::new()
                .set_directory(self.portable_paths.root.clone())
                .pick_folder();
        }

        let filename =
            default_export_filename(&self.session.project_title(), &settings.family.container);
        rfd::FileDialog::new()
            .set_directory(self.portable_paths.root.clone())
            .set_file_name(&filename)
            .add_filter(
                &settings.family.display_name,
                &[settings.family.container.as_str()],
            )
            .save_file()
    }

    fn import_media(&mut self, path: PathBuf) {
        let Some(provider) = self.provider.as_ref() else {
            self.push_log("FFmpeg runtime が見つからないためメディアを読込できません。");
            self.runtime_diagnostics_open = true;
            return;
        };

        match self.session.import_media(provider, &path) {
            Ok(()) => {
                self.preview_key = None;
                self.overlay_key = None;
                self.push_log(format!("メディアを読込: {}", path.display()));
            }
            Err(error) => self.push_log(format!("メディア読込失敗: {error}")),
        }
    }

    fn open_project(&mut self, path: PathBuf) {
        match AppSession::load_project_from_path(&path) {
            Ok(mut session) => {
                session.set_history_limit(self.settings.history_depth);
                session.active_style.stabilization_strength =
                    (self.settings.stroke_stabilization_default as f32 / 100.0).clamp(0.0, 1.0);
                self.session = session;
                self.preview_key = None;
                self.overlay_key = None;
                self.push_log(format!("プロジェクトを読込: {}", path.display()));
            }
            Err(error) => self.push_log(format!("プロジェクト読込失敗: {error:#}")),
        }
    }

    fn save_project(&mut self, path: PathBuf) {
        match self.session.save_project_to_path(&path) {
            Ok(()) => {
                let autosave_path = self.portable_paths.autosave_file("recovery_latest");
                if autosave_path.exists() {
                    let _ = fs::remove_file(&autosave_path);
                }
                self.push_log(format!("プロジェクトを保存: {}", path.display()))
            }
            Err(error) => self.push_log(format!("保存失敗: {error:#}")),
        }
    }

    fn guide_modifier_active(&self, ctx: &egui::Context) -> bool {
        ctx.input(|input| match self.settings.guide_modifier.as_str() {
            "ctrl" => input.modifiers.ctrl,
            "alt" => input.modifiers.alt,
            "shift" => input.modifiers.shift,
            _ => {
                if cfg!(target_os = "macos") {
                    input.modifiers.alt
                } else {
                    input.modifiers.ctrl
                }
            }
        })
    }

    fn advance_playback(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        let delta = now.saturating_duration_since(self.last_update_at);
        self.last_update_at = now;

        if let Some(playback) = &self.session.playback {
            if playback.is_playing {
                let next = playback.current_time.ticks + delta.as_millis() as i64;
                self.session
                    .seek(pauseink_domain::MediaTime::from_millis(next));
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
    }

    fn frame_dimensions(&self) -> (u32, u32) {
        if let Some(imported) = &self.session.imported_media {
            (
                imported.probe.width.unwrap_or(1280),
                imported.probe.height.unwrap_or(720),
            )
        } else {
            (1280, 720)
        }
    }

    fn refresh_preview_texture(
        &mut self,
        ctx: &egui::Context,
        target_width: u32,
        target_height: u32,
    ) {
        if !self.settings.gpu_preview_enabled {
            self.preview_key = None;
            return;
        }
        let Some(provider) = self.provider.as_ref() else {
            return;
        };
        let Some(imported_media) = &self.session.imported_media else {
            return;
        };
        let time_bucket = self.session.current_time().ticks / 100;
        let key = (
            imported_media.source_path.clone(),
            time_bucket,
            target_width,
            target_height,
        );
        if self.preview_key.as_ref() == Some(&key) {
            return;
        }

        match provider.preview_frame(
            &imported_media.source_path,
            self.session.current_time(),
            target_width,
            target_height,
        ) {
            Ok(frame) => {
                let image = preview_frame_to_color_image(&frame);
                if let Some(texture) = &mut self.preview_texture {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.preview_texture = Some(ctx.load_texture(
                        "pauseink-preview",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                self.preview_key = Some(key);
            }
            Err(error) => self.push_log(format!("preview 抽出失敗: {error}")),
        }
    }

    fn refresh_overlay_texture(
        &mut self,
        ctx: &egui::Context,
        target_width: u32,
        target_height: u32,
        source_width: u32,
        source_height: u32,
    ) {
        let key = (
            self.session.current_time().ticks,
            self.session.project.strokes.len(),
            self.session.project.clear_events.len(),
            target_width,
            target_height,
            source_width,
            source_height,
        );
        if self.overlay_key.as_ref() == Some(&key) {
            return;
        }

        match render_overlay_rgba(&RenderRequest {
            project: &self.session.project,
            time: self.session.current_time(),
            width: target_width.max(1),
            height: target_height.max(1),
            source_width: source_width.max(1),
            source_height: source_height.max(1),
            background: pauseink_domain::RgbaColor::new(0, 0, 0, 0),
        }) {
            Ok(overlay) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [overlay.width as usize, overlay.height as usize],
                    &overlay.rgba_pixels,
                );
                if let Some(texture) = &mut self.overlay_texture {
                    texture.set(image, egui::TextureOptions::LINEAR);
                } else {
                    self.overlay_texture = Some(ctx.load_texture(
                        "pauseink-overlay",
                        image,
                        egui::TextureOptions::LINEAR,
                    ));
                }
                self.overlay_key = Some(key);
            }
            Err(error) => self.push_log(format!("overlay 描画失敗: {error}")),
        }
    }

    fn handle_canvas_input(
        &mut self,
        response: &egui::Response,
        frame_rect: Rect,
        frame_width: u32,
        frame_height: u32,
        ctx: &egui::Context,
    ) {
        let pointer_position = response.interact_pointer_pos();

        if self.template.placement_armed {
            if response.clicked() {
                if let Some(pointer_position) = pointer_position {
                    let relative = Pos2::new(
                        pointer_position.x - frame_rect.left(),
                        pointer_position.y - frame_rect.top(),
                    );
                    let slots =
                        self.template_slots_at_origin(ctx, Point::new(relative.x, relative.y));
                    self.template.current_slot_index = 0;
                    self.template.slot_object_ids = vec![None; slots.len()];
                    self.template.placed_slots = Some(slots);
                    self.template.placement_armed = false;
                    self.push_log("テンプレート配置を確定しました。");
                }
            }
            return;
        }

        if response.drag_started() {
            self.guide_capture_armed = self.guide_modifier_active(ctx);
            if self.guide_capture_armed {
                self.guide_modifier_used_for_stroke = true;
            }
            if let Some(pointer_position) = pointer_position {
                if let Some(frame_point) = pointer_position_to_frame_point(
                    pointer_position,
                    frame_rect,
                    frame_width,
                    frame_height,
                ) {
                    self.session
                        .begin_stroke(frame_point, self.session.current_time());
                    self.canvas_drag_active = true;
                }
            }
        }

        if self.canvas_drag_active {
            if let Some(pointer_position) = pointer_position {
                if let Some(frame_point) = pointer_position_to_frame_point(
                    pointer_position,
                    frame_rect,
                    frame_width,
                    frame_height,
                ) {
                    self.session
                        .append_stroke_point(frame_point, self.session.current_time());
                }
            }

            let pointer_down = ctx.input(|input| input.pointer.primary_down());
            if !pointer_down {
                let target_object = self
                    .template
                    .placed_slots
                    .as_ref()
                    .and_then(|slots| slots.get(self.template.current_slot_index))
                    .and_then(|_| {
                        self.template
                            .slot_object_ids
                            .get(self.template.current_slot_index)
                            .cloned()
                            .flatten()
                    });

                match self.session.commit_stroke_into_object(target_object) {
                    Ok(Some(object_id)) => {
                        self.last_committed_object_bounds = self.session.object_bounds(&object_id);
                        if self.guide_capture_armed {
                            self.capture_guide_from_object(&object_id);
                        }

                        if self.template.placed_slots.is_some() {
                            if let Some(slot_object) = self
                                .template
                                .slot_object_ids
                                .get_mut(self.template.current_slot_index)
                            {
                                *slot_object = Some(object_id);
                            }
                            if self.template.current_slot_index + 1
                                < self.template.slot_object_ids.len()
                            {
                                self.template.current_slot_index += 1;
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => self.push_log(format!("stroke 確定失敗: {error:#}")),
                }
                self.canvas_drag_active = false;
                self.guide_capture_armed = false;
            }
        }
    }

    fn draw_template_preview(
        &self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        frame_rect: Rect,
        response: &egui::Response,
    ) {
        let hovered_origin = response.interact_pointer_pos().map(|position| {
            Point::new(
                position.x - frame_rect.left(),
                position.y - frame_rect.top(),
            )
        });

        let slots = if let Some(slots) = &self.template.placed_slots {
            Some(slots.clone())
        } else if self.template.placement_armed {
            hovered_origin.map(|origin| self.template_slots_at_origin(ctx, origin))
        } else {
            None
        };

        if let Some(slots) = slots {
            let angle = -self.template.settings.slope_degrees.to_radians();
            for (index, slot) in slots.iter().enumerate() {
                let rect = Rect::from_min_size(
                    Pos2::new(
                        frame_rect.left() + slot.origin.x,
                        frame_rect.top() + slot.origin.y,
                    ),
                    Vec2::new(slot.width.max(12.0), slot.height.max(12.0)),
                );
                let highlight = self.template.placed_slots.is_some()
                    && index == self.template.current_slot_index;
                let stroke = if highlight {
                    EguiStroke::new(2.0, Color32::from_rgb(255, 200, 60))
                } else {
                    EguiStroke::new(1.0, Color32::from_rgba_unmultiplied(180, 220, 255, 160))
                };

                if matches!(
                    self.template.settings.underlay_mode,
                    UnderlayMode::SlotBoxOnly | UnderlayMode::OutlineAndSlotBox
                ) {
                    painter.add(
                        egui::epaint::RectShape::stroke(
                            rect,
                            0.0,
                            stroke,
                            egui::StrokeKind::Middle,
                        )
                        .with_angle_and_pivot(angle, rect.left_top()),
                    );
                }
                if matches!(
                    self.template.settings.underlay_mode,
                    UnderlayMode::Outline
                        | UnderlayMode::OutlineAndSlotBox
                        | UnderlayMode::FaintFill
                ) {
                    let galley = ctx.fonts_mut(|fonts| {
                        fonts.layout_no_wrap(
                            slot.grapheme.clone(),
                            self.template_font_id(
                                (self.template.settings.font_size * slot.scale).max(14.0),
                            ),
                            Color32::from_rgba_unmultiplied(220, 220, 240, 180),
                        )
                    });
                    painter.add(
                        egui::epaint::TextShape::new(
                            rect.left_top(),
                            galley,
                            Color32::from_rgba_unmultiplied(220, 220, 240, 180),
                        )
                        .with_angle(angle),
                    );
                }
                if matches!(
                    self.template.settings.underlay_mode,
                    UnderlayMode::FaintFill
                ) {
                    painter.add(
                        egui::epaint::RectShape::filled(
                            rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(180, 200, 255, 32),
                        )
                        .with_angle_and_pivot(angle, rect.left_top()),
                    );
                }
            }
        }
    }

    fn draw_guide_overlay(
        &self,
        painter: &egui::Painter,
        frame_rect: Rect,
        frame_width: u32,
        frame_height: u32,
    ) {
        let Some(guide) = &self.guide_geometry else {
            return;
        };

        for line in guide
            .horizontal_lines
            .iter()
            .chain(guide.vertical_lines.iter())
        {
            let stroke = match line.kind {
                GuideLineKind::Main => {
                    EguiStroke::new(1.5, Color32::from_rgba_unmultiplied(120, 200, 255, 180))
                }
                GuideLineKind::Helper => {
                    EguiStroke::new(1.0, Color32::from_rgba_unmultiplied(120, 200, 255, 80))
                }
            };
            let Some(start) = frame_point_to_screen_position(
                pauseink_domain::Point2 {
                    x: line.start.x,
                    y: line.start.y,
                },
                frame_rect,
                frame_width,
                frame_height,
            ) else {
                continue;
            };
            let Some(end) = frame_point_to_screen_position(
                pauseink_domain::Point2 {
                    x: line.end.x,
                    y: line.end.y,
                },
                frame_rect,
                frame_width,
                frame_height,
            ) else {
                continue;
            };
            painter.line_segment([start, end], stroke);
        }
    }

    fn save_settings(&mut self) {
        match save_settings_to_file(&self.portable_paths, &self.settings) {
            Ok(()) => {}
            Err(error) => self.push_log(format!("settings 保存失敗: {error}")),
        }
    }

    fn maybe_autosave(&mut self) {
        if !self.session.dirty
            || self.last_autosave_at.elapsed()
                < Duration::from_secs(self.settings.autosave_interval_seconds.max(1))
        {
            return;
        }

        match self.session.save_project_to_string() {
            Ok(serialized) => {
                let autosave_path = self.portable_paths.autosave_file("recovery_latest");
                match fs::write(&autosave_path, serialized) {
                    Ok(()) => {
                        self.last_autosave_at = Instant::now();
                        self.push_log(format!("autosave 更新: {}", autosave_path.display()));
                    }
                    Err(error) => self.push_log(format!("autosave 保存失敗: {error}")),
                }
            }
            Err(error) => self.push_log(format!("autosave 直列化失敗: {error:#}")),
        }
    }

    fn recover_latest_autosave(&mut self) {
        let autosave_path = self.portable_paths.autosave_file("recovery_latest");
        match fs::read_to_string(&autosave_path)
            .ok()
            .and_then(|source| AppSession::load_project_from_str(&source).ok())
        {
            Some(mut session) => {
                session.set_history_limit(self.settings.history_depth);
                session.active_style.stabilization_strength =
                    (self.settings.stroke_stabilization_default as f32 / 100.0).clamp(0.0, 1.0);
                self.session = session;
                self.preview_key = None;
                self.overlay_key = None;
                self.recovery_prompt_open = false;
                self.push_log(format!("autosave から復旧: {}", autosave_path.display()));
            }
            None => self.push_log("autosave 復旧に失敗しました。"),
        }
    }

    fn draw_preferences_window(&mut self, ctx: &egui::Context) {
        if !self.preferences_open {
            return;
        }

        let mut open = self.preferences_open;
        egui::Window::new("設定")
            .open(&mut open)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("基本設定");
                if ui
                    .add(
                        egui::DragValue::new(&mut self.settings.history_depth)
                            .range(16..=4096)
                            .speed(1.0),
                    )
                    .changed()
                {
                    self.session.set_history_limit(self.settings.history_depth);
                }
                ui.label("元に戻す履歴深さ");

                egui::ComboBox::from_label("ガイド修飾キー")
                    .selected_text(match self.settings.guide_modifier.as_str() {
                        "ctrl" => "Ctrl",
                        "alt" => "Alt",
                        "shift" => "Shift",
                        _ => "プラットフォーム既定",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.settings.guide_modifier,
                            "platform_default".to_owned(),
                            "プラットフォーム既定",
                        );
                        ui.selectable_value(
                            &mut self.settings.guide_modifier,
                            "ctrl".to_owned(),
                            "Ctrl",
                        );
                        ui.selectable_value(
                            &mut self.settings.guide_modifier,
                            "alt".to_owned(),
                            "Alt",
                        );
                        ui.selectable_value(
                            &mut self.settings.guide_modifier,
                            "shift".to_owned(),
                            "Shift",
                        );
                    });
                if ui
                    .add(
                        egui::Slider::new(&mut self.settings.guide_slope_degrees, -20.0..=20.0)
                            .text("ガイド傾き"),
                    )
                    .changed()
                {
                    self.refresh_guide_geometry();
                }
                ui.checkbox(
                    &mut self.settings.gpu_preview_enabled,
                    "プレビュー GPU を有効",
                );
                ui.checkbox(
                    &mut self.settings.media_hwaccel_enabled,
                    "メディア HW accel を試行",
                );
                ui.add(
                    egui::Slider::new(&mut self.settings.autosave_interval_seconds, 5..=300)
                        .text("自動保存間隔 秒"),
                );

                ui.separator();
                ui.heading("Google Fonts");
                ui.checkbox(
                    &mut self.settings.google_fonts.enabled,
                    "Google Fonts を有効",
                );
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.google_font_input);
                    if ui.button("追加").clicked() {
                        let family = self.google_font_input.trim().to_owned();
                        if !family.is_empty()
                            && !self
                                .settings
                                .google_fonts
                                .families
                                .iter()
                                .any(|existing| existing == &family)
                        {
                            self.settings.google_fonts.families.push(family);
                            self.settings.google_fonts.families.sort();
                            self.google_font_input.clear();
                        }
                    }
                });

                let families = self.settings.google_fonts.families.clone();
                let mut remove_families = Vec::new();
                for family in families {
                    ui.horizontal(|ui| {
                        ui.label(&family);
                        ui.label(
                            if google_font_is_cached(
                                &self.portable_paths.google_fonts_cache_dir(),
                                &family,
                            ) {
                                "cache あり"
                            } else {
                                "cache なし"
                            },
                        );
                        if ui.button("取得").clicked() {
                            match fetch_google_font_to_cache(
                                &self.portable_paths.google_fonts_cache_dir(),
                                &family,
                            ) {
                                Ok(path) => {
                                    self.push_log(format!(
                                        "Google Fonts を取得: {} -> {}",
                                        family,
                                        path.display()
                                    ));
                                    self.rebuild_local_font_families();
                                }
                                Err(error) => self.push_log(format!(
                                    "Google Fonts 取得失敗: {} / {}",
                                    family, error
                                )),
                            }
                        }
                        if ui.button("キャッシュ削除").clicked() {
                            let path = google_font_cache_file(
                                &self.portable_paths.google_fonts_cache_dir(),
                                &family,
                            );
                            match fs::remove_file(&path) {
                                Ok(()) => {
                                    self.push_log(format!("Google Fonts cache 削除: {}", family));
                                    self.rebuild_local_font_families();
                                }
                                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                                Err(error) => self.push_log(format!(
                                    "Google Fonts cache 削除失敗: {} / {}",
                                    family, error
                                )),
                            }
                        }
                        if ui.button("一覧から外す").clicked() {
                            remove_families.push(family.clone());
                        }
                    });
                }
                if !remove_families.is_empty() {
                    self.settings
                        .google_fonts
                        .families
                        .retain(|family| !remove_families.contains(family));
                }

                ui.separator();
                ui.heading("ローカルフォント");
                let local_dirs = self.settings.local_font_dirs.clone();
                let mut remove_dirs = Vec::new();
                for directory in local_dirs {
                    ui.horizontal(|ui| {
                        ui.label(directory.display().to_string());
                        if ui.button("削除").clicked() {
                            remove_dirs.push(directory.clone());
                        }
                    });
                }
                if !remove_dirs.is_empty() {
                    self.settings
                        .local_font_dirs
                        .retain(|directory| !remove_dirs.contains(directory));
                    self.rebuild_local_font_families();
                }
                if ui.button("フォルダ追加").clicked() {
                    if let Some(directory) = rfd::FileDialog::new().pick_folder() {
                        if !self.settings.local_font_dirs.contains(&directory) {
                            self.settings.local_font_dirs.push(directory);
                            self.rebuild_local_font_families();
                        }
                    }
                }

                ui.separator();
                if ui.button("設定を保存").clicked() {
                    self.save_settings();
                }
            });
        self.preferences_open = open;
    }

    fn draw_cache_manager_window(&mut self, ctx: &egui::Context) {
        if !self.cache_manager_open {
            return;
        }

        let mut open = self.cache_manager_open;
        egui::Window::new("キャッシュ管理")
            .open(&mut open)
            .resizable(true)
            .show(ctx, |ui| {
                let categories = vec![
                    ("Google Fonts", self.portable_paths.google_fonts_cache_dir()),
                    ("フォント索引", self.portable_paths.font_index_cache_dir()),
                    ("メディア解析", self.portable_paths.media_probe_cache_dir()),
                    ("サムネイル", self.portable_paths.thumbnail_cache_dir()),
                    ("一時ファイル", self.portable_paths.temp_dir.clone()),
                ];

                for (label, path) in categories {
                    ui.horizontal(|ui| {
                        ui.label(label);
                        let size_label = directory_size(&path)
                            .map(format_bytes)
                            .unwrap_or_else(|_| "計測失敗".to_owned());
                        ui.label(size_label);
                        if ui.button("削除").clicked() {
                            match clear_directory_contents(&path) {
                                Ok(()) => {
                                    self.push_log(format!(
                                        "cache 削除: {} / {}",
                                        label,
                                        path.display()
                                    ));
                                    if label == "Google Fonts" {
                                        self.rebuild_local_font_families();
                                    }
                                }
                                Err(error) => {
                                    self.push_log(format!("cache 削除失敗: {} / {}", label, error))
                                }
                            }
                        }
                    });
                }
            });
        self.cache_manager_open = open;
    }

    fn draw_runtime_diagnostics_window(&mut self, ctx: &egui::Context) {
        if !self.runtime_diagnostics_open {
            return;
        }

        let mut open = self.runtime_diagnostics_open;
        egui::Window::new("ランタイム診断")
            .open(&mut open)
            .resizable(true)
            .show(ctx, |ui| {
                if ui.button("診断を再取得").clicked() {
                    self.refresh_runtime_capabilities();
                }
                ui.separator();
                if let Some(runtime) = &self.runtime {
                    ui.label(format!("ランタイム由来: {:?}", runtime.origin));
                    ui.label(format!("ffmpeg パス: {}", runtime.ffmpeg_path.display()));
                    ui.label(format!("ffprobe パス: {}", runtime.ffprobe_path.display()));
                    if let Some(summary) = &runtime.build_summary {
                        ui.label(format!("ビルド情報: {summary}"));
                    }
                    if let Some(summary) = &runtime.license_summary {
                        ui.label(format!("ライセンス情報: {summary}"));
                    }
                } else {
                    ui.label("runtime は未検出です。");
                }

                ui.separator();
                if let Some(capabilities) = &self.runtime_capabilities {
                    ui.label(format!(
                        "映像エンコーダ: {}",
                        capabilities.video_encoders.join(", ")
                    ));
                    ui.label(format!(
                        "音声エンコーダ: {}",
                        capabilities.audio_encoders.join(", ")
                    ));
                    ui.label(format!("muxer: {}", capabilities.muxers.join(", ")));
                    ui.label(format!("HW accel: {}", capabilities.hwaccels.join(", ")));
                } else {
                    ui.label("capability 情報はまだありません。");
                }

                ui.separator();
                ui.heading("Windows で runtime が見つからないとき");
                for line in ffmpeg_runtime_windows_help(&self.portable_paths.runtime_dir) {
                    ui.label(line);
                }
            });
        self.runtime_diagnostics_open = open;
    }

    fn draw_export_panel(&mut self, ui: &mut egui::Ui) {
        ui.label("書き出し");
        self.sync_export_state();
        let Some(catalog) = self.export.catalog.as_ref() else {
            ui.label("export catalog の読込に失敗しました。");
            return;
        };

        let family_choices = catalog
            .families_for_tier(RuntimeTier::Mainline)
            .into_iter()
            .map(|family| {
                (
                    family.id.clone(),
                    family.display_name.clone(),
                    family.output_kind,
                    family.supports_alpha,
                )
            })
            .collect::<Vec<_>>();
        let profile_choices = catalog
            .profiles_for_family(&self.export.family_id)
            .into_iter()
            .map(|profile| {
                (
                    profile.id.clone(),
                    profile.display_name.clone(),
                    profile.notes.clone(),
                )
            })
            .collect::<Vec<_>>();

        egui::ComboBox::from_label("出力 family")
            .selected_text(
                family_choices
                    .iter()
                    .find(|(id, _, _, _)| id == &self.export.family_id)
                    .map(|(_, label, _, _)| label.clone())
                    .unwrap_or_else(|| "未選択".to_owned()),
            )
            .show_ui(ui, |ui| {
                for (id, label, _, _) in &family_choices {
                    ui.selectable_value(&mut self.export.family_id, id.clone(), label);
                }
            });

        egui::ComboBox::from_label("配布 profile")
            .selected_text(
                profile_choices
                    .iter()
                    .find(|(id, _, _)| id == &self.export.profile_id)
                    .map(|(_, label, _)| label.clone())
                    .unwrap_or_else(|| "未選択".to_owned()),
            )
            .show_ui(ui, |ui| {
                for (id, label, _) in &profile_choices {
                    ui.selectable_value(&mut self.export.profile_id, id.clone(), label);
                }
            });

        if let Some((_, _, notes)) = profile_choices
            .iter()
            .find(|(id, _, _)| id == &self.export.profile_id)
        {
            ui.label(notes);
        }

        if let Some((_, _, output_kind, supports_alpha)) = family_choices
            .iter()
            .find(|(id, _, _, _)| id == &self.export.family_id)
        {
            match output_kind {
                OutputKind::CompositeOnly => {
                    self.export.output_mode = ExportOutputMode::Composite;
                    ui.label("この family は合成出力のみです。");
                }
                _ if self.export.family_id == "png_sequence_rgba" => {
                    ui.label("PNG Sequence は注釈オーバーレイ連番を書き出します。");
                }
                _ if *supports_alpha => {
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.export.output_mode,
                            ExportOutputMode::Composite,
                            "合成",
                        );
                        ui.selectable_value(
                            &mut self.export.output_mode,
                            ExportOutputMode::Transparent,
                            "透過",
                        );
                    });
                }
                _ => {
                    self.export.output_mode = ExportOutputMode::Composite;
                }
            }
        }

        match self.planned_export_settings() {
            Ok(mut settings) => {
                ui.label(format!("設定バケット: {}", settings.selected_bucket_id));
                ui.label(format!(
                    "音声: {}",
                    if settings.audio_enabled {
                        "あり"
                    } else {
                        "なし"
                    }
                ));
                let editable = settings.profile.id == "custom";
                if editable {
                    self.export.custom_settings = Some(settings.clone());
                }
                draw_optional_u32_field(
                    ui,
                    "映像 target kbps",
                    &mut settings.target_video_bitrate_kbps,
                    editable,
                );
                draw_optional_u32_field(
                    ui,
                    "映像 max kbps",
                    &mut settings.max_video_bitrate_kbps,
                    editable,
                );
                draw_optional_u32_field(
                    ui,
                    "音声 kbps",
                    &mut settings.audio_bitrate_kbps,
                    editable && settings.audio_enabled,
                );
                draw_optional_u32_field(
                    ui,
                    "サンプルレート Hz",
                    &mut settings.sample_rate_hz,
                    editable && settings.audio_enabled,
                );
                draw_optional_u32_field(
                    ui,
                    "keyframe 秒",
                    &mut settings.keyframe_interval_seconds,
                    editable,
                );
                if editable {
                    self.export.custom_settings = Some(settings.clone());
                }

                let start_clicked = ui
                    .add_enabled(
                        self.pending_export.is_none(),
                        egui::Button::new(
                            if matches!(self.export.output_mode, ExportOutputMode::Transparent) {
                                "透過書き出し"
                            } else {
                                "書き出し開始"
                            },
                        ),
                    )
                    .clicked();
                if start_clicked {
                    self.start_export();
                }
            }
            Err(error) => {
                ui.label(format!("設定計算失敗: {error}"));
            }
        }

        if let Some(pending) = &self.pending_export {
            ui.label(format!("実行中: {}", pending.summary));
        }
    }

    fn draw_export_queue_tab(&self, ui: &mut egui::Ui) {
        if let Some(pending) = &self.pending_export {
            ui.label(format!("実行中: {}", pending.summary));
            ui.label(pending.output_path.display().to_string());
            ui.separator();
        }

        if self.export.jobs.is_empty() {
            ui.label("書き出し履歴はまだありません。");
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for job in &self.export.jobs {
                ui.label(format!("{} / {}", job.summary, job.status));
                ui.label(job.output_path.display().to_string());
                if job.software_fallback_used {
                    ui.label("HW path 失敗後に software fallback で完了");
                }
                ui.separator();
            }
        });
    }
}

impl eframe::App for DesktopApp {
    fn on_exit(&mut self) {
        self.save_settings();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.maybe_apply_egui_fonts(&ctx);
        self.advance_playback(&ctx);
        self.handle_global_shortcuts(&ctx);
        self.handle_guide_modifier_tap(&ctx);
        self.maybe_autosave();
        self.poll_pending_export();

        if self.recovery_prompt_open {
            egui::Window::new("復旧")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(&ctx, |ui| {
                    let autosave_path = self.portable_paths.autosave_file("recovery_latest");
                    ui.label("前回の autosave が見つかりました。");
                    ui.label(autosave_path.display().to_string());
                    ui.horizontal(|ui| {
                        if ui.button("復旧する").clicked() {
                            self.recover_latest_autosave();
                        }
                        if ui.button("破棄する").clicked() {
                            let _ = fs::remove_file(&autosave_path);
                            self.recovery_prompt_open = false;
                        }
                    });
                });
        }

        egui::Panel::top("top_bar").show(&ctx, |ui| {
            let undo_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Z);
            let redo_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            );
            ui.horizontal_wrapped(|ui| {
                if ui.button("開く").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PauseInk プロジェクト", &["pauseink"])
                        .pick_file()
                    {
                        self.open_project(path);
                    }
                }
                if ui.button("保存").clicked() {
                    if let Some(path) = self.session.document_path.clone() {
                        self.save_project(path);
                    } else if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PauseInk プロジェクト", &["pauseink"])
                        .save_file()
                    {
                        self.save_project(path);
                    }
                }
                if ui.button("別名保存").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PauseInk プロジェクト", &["pauseink"])
                        .save_file()
                    {
                        self.save_project(path);
                    }
                }
                if ui.button("メディア読込").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.import_media(path);
                    }
                }
                if ui
                    .add(
                        egui::Button::new("元に戻す")
                            .shortcut_text(ctx.format_shortcut(&undo_shortcut)),
                    )
                    .clicked()
                {
                    self.perform_undo();
                }
                if ui
                    .add(
                        egui::Button::new("やり直す")
                            .shortcut_text(ctx.format_shortcut(&redo_shortcut)),
                    )
                    .clicked()
                {
                    self.perform_redo();
                }
                if ui.button("全消去").clicked() {
                    match self
                        .session
                        .insert_clear_event(pauseink_domain::ClearKind::Instant)
                    {
                        Ok(clear_id) => {
                            self.push_log(format!("clear event を挿入: {}", clear_id.0))
                        }
                        Err(error) => self.push_log(format!("clear event 挿入失敗: {error:#}")),
                    }
                }

                ui.separator();
                if ui.button("設定").clicked() {
                    self.preferences_open = true;
                }
                if ui.button("キャッシュ").clicked() {
                    self.cache_manager_open = true;
                }
                if ui.button("診断").clicked() {
                    self.runtime_diagnostics_open = true;
                }
                ui.separator();
                ui.label(format!("状態: {}", self.session.transport_summary()));
                ui.label(format!(
                    "未保存変更: {}",
                    if self.session.dirty {
                        "あり"
                    } else {
                        "なし"
                    }
                ));
            });
        });

        egui::Panel::top("transport_bar").show(&ctx, |ui| {
            ui.horizontal(|ui| {
                let playing = self
                    .session
                    .playback
                    .as_ref()
                    .map(|playback| playback.is_playing)
                    .unwrap_or(false);
                if ui
                    .button(if playing {
                        "再生中: 一時停止"
                    } else {
                        "停止中: 再生"
                    })
                    .clicked()
                {
                    if playing {
                        self.session.pause();
                    } else {
                        self.session.play();
                    }
                }

                if let Some(duration) = self
                    .session
                    .playback
                    .as_ref()
                    .and_then(|playback| playback.media.duration())
                {
                    let mut current_ms = self.session.current_time().ticks as f64;
                    let response = ui.add_sized(
                        [ui.available_width().max(240.0), 0.0],
                        egui::Slider::new(&mut current_ms, 0.0..=duration.ticks as f64)
                            .text("シーク")
                            .show_value(false),
                    );
                    ui.label(format!(
                        "{:.2} / {:.2} 秒",
                        current_ms / 1000.0,
                        duration.ticks as f64 / 1000.0
                    ));
                    if response.changed() {
                        self.session.seek(pauseink_domain::MediaTime::from_millis(
                            current_ms.round() as i64,
                        ));
                    }
                } else {
                    ui.label("メディアを読み込むとシークバーがここに表示されます。");
                }
            });
        });

        self.draw_preferences_window(&ctx);
        self.draw_cache_manager_window(&ctx);
        self.draw_runtime_diagnostics_window(&ctx);

        egui::Panel::left("left_panel")
            .default_width(250.0)
            .resizable(true)
            .show(&ctx, |ui| {
                ui.heading("メディア");
                ui.label(&self.runtime_status);
                if self.runtime.is_none() {
                    ui.label("Windows で runtime が見つからない場合は `診断` に配置場所の案内があります。");
                }
                ui.horizontal(|ui| {
                    if ui.button("診断を開く").clicked() {
                        self.runtime_diagnostics_open = true;
                    }
                    if ui.button("機能情報更新").clicked() {
                        self.refresh_runtime_capabilities();
                    }
                });
                if let Some(path) = self.session.media_source_hint() {
                    ui.label(format!("ソース: {}", path.display()));
                }
                if let Some(imported) = &self.session.imported_media {
                    ui.label(format!(
                        "{}x{} / {:.2}fps",
                        imported.probe.width.unwrap_or_default(),
                        imported.probe.height.unwrap_or_default(),
                        imported.probe.frame_rate.unwrap_or_default()
                    ));
                }

                ui.separator();
                ui.heading("テンプレート");
                ui.text_edit_singleline(&mut self.template.text);
                let mut selected_template_font = self.template.font_family.clone();
                egui::ComboBox::from_label("テンプレート font")
                    .selected_text(&selected_template_font)
                    .show_ui(ui, |ui| {
                        for family in
                            template_font_choices(&self.local_font_families, &self.template.font_family)
                        {
                            ui.selectable_value(
                                &mut selected_template_font,
                                family.clone(),
                                family,
                            );
                        }
                    });
                if selected_template_font != self.template.font_family {
                    self.template.font_family = selected_template_font;
                    self.font_config_dirty = true;
                }
                ui.add(
                    egui::Slider::new(&mut self.template.settings.font_size, 24.0..=180.0)
                        .text("フォントサイズ"),
                );
                ui.add(
                    egui::Slider::new(&mut self.template.settings.tracking, 0.0..=48.0)
                        .text("字間"),
                );
                ui.add(
                    egui::Slider::new(&mut self.template.settings.slope_degrees, -20.0..=20.0)
                        .text("傾き"),
                );
                ui.horizontal(|ui| {
                    if ui.button("テンプレート配置").clicked() {
                        self.template.placement_armed = true;
                        self.template.placed_slots = None;
                        self.template.slot_object_ids.clear();
                    }
                    if ui.button("次スロット").clicked() {
                        if let Some(slots) = &self.template.placed_slots {
                            if self.template.current_slot_index + 1 < slots.len() {
                                self.template.current_slot_index += 1;
                            }
                        }
                    }
                    if ui.button("テンプレート解除").clicked() {
                        self.template.placement_armed = false;
                        self.template.placed_slots = None;
                        self.template.slot_object_ids.clear();
                        self.template.current_slot_index = 0;
                    }
                });

                ui.separator();
                ui.heading("フォント");
                if ui.button("ローカル一覧更新").clicked() {
                    self.rebuild_local_font_families();
                }
                ui.label(format!(
                    "読み込み済み候補: {} 件",
                    self.local_font_families.len()
                ));
                for family in self.local_font_families.iter().take(8) {
                    ui.label(format!("・{family}"));
                }
                ui.separator();
                ui.label("Google Fonts 設定:");
                for family in &self.settings.google_fonts.families {
                    let cached = google_font_cache_file(
                        &self.portable_paths.google_fonts_cache_dir(),
                        family,
                    );
                    ui.label(format!(
                        "・{} ({})",
                        family,
                        if cached.exists() {
                            "cache あり"
                        } else {
                            "cache なし"
                        }
                    ));
                }
                if ui.button("Google Fonts 設定を開く").clicked() {
                    self.preferences_open = true;
                }
            });

        egui::Panel::right("inspector")
            .default_width(260.0)
            .resizable(true)
            .show(&ctx, |ui| {
                ui.heading("インスペクター");
                let mut title = self.session.project_title();
                ui.label(format!("タイトル: {}", title));
                if ui.text_edit_singleline(&mut title).changed() {
                    self.session.set_project_title(title);
                }
                ui.separator();
                if !self.style_presets.is_empty() {
                    ui.label("style preset");
                    egui::ComboBox::from_label("組み込み preset")
                        .selected_text(
                            self.style_presets
                                .iter()
                                .find(|preset| preset.id == self.selected_style_preset_id)
                                .map(|preset| preset.display_name.clone())
                                .unwrap_or_else(|| "未選択".to_owned()),
                        )
                        .show_ui(ui, |ui| {
                            for preset in &self.style_presets {
                                ui.selectable_value(
                                    &mut self.selected_style_preset_id,
                                    preset.id.clone(),
                                    &preset.display_name,
                                );
                            }
                        });
                    if ui.button("preset を適用").clicked() {
                        self.apply_selected_style_preset();
                    }
                    ui.separator();
                }
                ui.label("基本スタイル");

                let mut color = Color32::from_rgba_premultiplied(
                    self.session.active_style.color.r,
                    self.session.active_style.color.g,
                    self.session.active_style.color.b,
                    self.session.active_style.color.a,
                );
                if ui.color_edit_button_srgba(&mut color).changed() {
                    self.session.active_style.color =
                        pauseink_domain::RgbaColor::new(color.r(), color.g(), color.b(), color.a());
                }
                ui.add(
                    egui::Slider::new(&mut self.session.active_style.thickness, 1.0..=32.0)
                        .text("太さ"),
                );
                ui.add(
                    egui::Slider::new(&mut self.session.active_style.opacity, 0.05..=1.0)
                        .text("不透明度"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.session.active_style.stabilization_strength,
                        0.0..=1.0,
                    )
                    .text("手ブレ補正"),
                );
                ui.separator();
                ui.label("ガイド");
                if ui
                    .add(
                        egui::Slider::new(&mut self.settings.guide_slope_degrees, -20.0..=20.0)
                            .text("ガイド傾き"),
                    )
                    .changed()
                {
                    self.refresh_guide_geometry();
                }
                if ui.button("ガイド解除").clicked() {
                    self.guide_state = None;
                    self.guide_geometry = None;
                }
                ui.separator();
                self.draw_export_panel(ui);
            });

        egui::Panel::bottom("bottom_tabs")
            .default_height(180.0)
            .resizable(true)
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (BottomTab::Outline, "オブジェクト一覧"),
                        (BottomTab::PageEvents, "ページイベント"),
                        (BottomTab::ExportQueue, "書き出しキュー"),
                        (BottomTab::Logs, "ログ"),
                    ] {
                        if ui.selectable_label(self.bottom_tab == tab, label).clicked() {
                            self.bottom_tab = tab;
                        }
                    }
                });
                ui.separator();

                match self.bottom_tab {
                    BottomTab::Outline => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for object in &self.session.project.glyph_objects {
                                ui.label(format!(
                                    "{} / stroke:{} / page:{} / z:{}",
                                    object.id.0,
                                    object.stroke_ids.len(),
                                    object.page_index(&self.session.project.clear_events),
                                    object.ordering.z_index
                                ));
                            }
                            if self.session.project.glyph_objects.is_empty() {
                                ui.label("オブジェクトはまだありません。");
                            }
                        });
                    }
                    BottomTab::PageEvents => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for clear in &self.session.project.clear_events {
                                ui.label(format!(
                                    "{} / t={} / {:?}",
                                    clear.id.0, clear.time.ticks, clear.kind
                                ));
                            }
                            if self.session.project.clear_events.is_empty() {
                                ui.label("clear event はまだありません。");
                            }
                        });
                    }
                    BottomTab::ExportQueue => {
                        self.draw_export_queue_tab(ui);
                    }
                    BottomTab::Logs => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for message in &self.logs {
                                ui.label(message);
                            }
                        });
                    }
                }
            });

        egui::CentralPanel::default().show(&ctx, |ui| {
            let canvas_size = ui.available_size();
            let (response, painter) = ui.allocate_painter(canvas_size, Sense::click_and_drag());
            let (frame_width, frame_height) = self.frame_dimensions();
            let frame_rect = fit_frame_to_canvas(
                frame_width,
                frame_height,
                CanvasSize {
                    width: response.rect.width(),
                    height: response.rect.height(),
                },
            )
            .map(|rect| {
                Rect::from_min_size(
                    Pos2::new(response.rect.left() + rect.x, response.rect.top() + rect.y),
                    Vec2::new(rect.width, rect.height),
                )
            })
            .unwrap_or(response.rect);

            painter.rect_filled(response.rect, 0.0, Color32::from_rgb(18, 22, 28));
            painter.rect_stroke(
                frame_rect,
                0.0,
                EguiStroke::new(1.0, Color32::from_gray(80)),
                egui::StrokeKind::Middle,
            );

            self.refresh_preview_texture(
                &ctx,
                frame_rect.width().round().max(1.0) as u32,
                frame_rect.height().round().max(1.0) as u32,
            );
            self.refresh_overlay_texture(
                &ctx,
                frame_rect.width().round().max(1.0) as u32,
                frame_rect.height().round().max(1.0) as u32,
                frame_width,
                frame_height,
            );

            if self.settings.gpu_preview_enabled {
                if let Some(texture) = &self.preview_texture {
                    painter.image(
                        texture.id(),
                        frame_rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
            } else {
                painter.text(
                    frame_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "プレビュー GPU を無効化中",
                    egui::FontId::proportional(16.0),
                    Color32::from_rgb(180, 180, 180),
                );
            }
            if let Some(texture) = &self.overlay_texture {
                painter.image(
                    texture.id(),
                    frame_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            self.draw_template_preview(&ctx, &painter, frame_rect, &response);
            self.draw_guide_overlay(&painter, frame_rect, frame_width, frame_height);

            if let Some(slots) = &self.template.placed_slots {
                if let Some(slot) = slots.get(self.template.current_slot_index) {
                    painter.text(
                        frame_rect.left_top() + egui::vec2(12.0, 12.0),
                        egui::Align2::LEFT_TOP,
                        format!(
                            "スロット {}/{}: {}",
                            self.template.current_slot_index + 1,
                            slots.len(),
                            slot.grapheme
                        ),
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(255, 232, 120),
                    );
                }
            }

            self.handle_canvas_input(&response, frame_rect, frame_width, frame_height, &ctx);

            if self.canvas_drag_active {
                if let Some(pointer_position) = response.interact_pointer_pos() {
                    if frame_rect.contains(pointer_position) {
                        painter.circle_filled(
                            pointer_position,
                            3.0,
                            Color32::from_rgb(255, 255, 255),
                        );
                    }
                }
            }
        });
    }
}

fn initialize_export_selection(export: &mut ExportState) {
    let Some(catalog) = export.catalog.as_ref() else {
        return;
    };

    let family_ids = catalog
        .families_for_tier(RuntimeTier::Mainline)
        .into_iter()
        .map(|family| family.id.clone())
        .collect::<Vec<_>>();
    if family_ids.is_empty() {
        return;
    }
    if !family_ids.iter().any(|id| id == &export.family_id) {
        export.family_id = preferred_family_id(&family_ids);
    }

    let profile_ids = catalog
        .profiles_for_family(&export.family_id)
        .into_iter()
        .map(|profile| profile.id.clone())
        .collect::<Vec<_>>();
    if profile_ids.is_empty() {
        export.profile_id.clear();
    } else if !profile_ids.iter().any(|id| id == &export.profile_id) {
        export.profile_id = preferred_profile_id(&profile_ids);
    }
}

fn preferred_profile_id(profile_ids: &[String]) -> String {
    ["medium", "high", "low", "custom"]
        .iter()
        .find_map(|preferred| {
            profile_ids
                .iter()
                .find(|candidate| candidate.as_str() == *preferred)
                .cloned()
        })
        .unwrap_or_else(|| profile_ids[0].clone())
}

fn preferred_family_id(family_ids: &[String]) -> String {
    [
        "webm_vp9_opus",
        "webm_av1_opus",
        "mov_prores_4444_pcm",
        "png_sequence_rgba",
    ]
    .iter()
    .find_map(|preferred| {
        family_ids
            .iter()
            .find(|candidate| candidate.as_str() == *preferred)
            .cloned()
    })
    .unwrap_or_else(|| family_ids[0].clone())
}

fn template_font_choices(local_font_families: &[String], selected_family: &str) -> Vec<String> {
    let mut choices = vec![SYSTEM_DEFAULT_FONT_FAMILY_LABEL.to_owned()];
    for family in local_font_families {
        if !choices.iter().any(|candidate| candidate == family) {
            choices.push(family.clone());
        }
    }
    if !selected_family.trim().is_empty()
        && !choices
            .iter()
            .any(|candidate| candidate.as_str() == selected_family)
    {
        choices.push(selected_family.to_owned());
    }
    choices
}

fn ffmpeg_runtime_windows_help(runtime_root: &Path) -> Vec<String> {
    let sidecar_dir = sidecar_runtime_dir(runtime_root, "windows-x86_64");
    vec![
        format!(
            "1. 推奨配置: `{}` に `ffmpeg.exe` / `ffprobe.exe` / `manifest.json` を置きます。",
            sidecar_dir.display()
        ),
        "2. sidecar を置けない場合は、`ffmpeg.exe` と `ffprobe.exe` の両方を `PATH` に通してください。"
            .to_owned(),
        "3. 配置後に `機能情報更新` を押すか、アプリを再起動すると再検出します。".to_owned(),
    ]
}

fn default_export_filename(title: &str, extension: &str) -> String {
    let mut stem = title
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>()
        .trim()
        .to_owned();
    if stem.is_empty() {
        stem = "pauseink_export".to_owned();
    }
    format!("{stem}.{extension}")
}

fn format_bytes(size: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = size as f64;
    let mut unit_index = 0usize;
    while value >= 1024.0 && unit_index + 1 < UNITS.len() {
        value /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{} {}", size, UNITS[unit_index])
    } else {
        format!("{value:.1} {}", UNITS[unit_index])
    }
}

fn draw_optional_u32_field(ui: &mut egui::Ui, label: &str, value: &mut Option<u32>, enabled: bool) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut raw = value.unwrap_or(0);
        let response = ui.add_enabled(
            enabled,
            egui::DragValue::new(&mut raw)
                .range(0..=1_000_000)
                .speed(1.0),
        );
        if enabled && response.changed() {
            *value = if raw == 0 { None } else { Some(raw) };
        }
    });
}

fn preview_frame_to_color_image(frame: &PreviewFrame) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &frame.rgba_pixels,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_pointer_roundtrips_through_frame_space_mapping() {
        let frame_rect = Rect::from_min_size(Pos2::new(50.0, 75.0), Vec2::new(640.0, 360.0));
        let pointer = Pos2::new(210.0, 165.0);

        let frame_point =
            pointer_position_to_frame_point(pointer, frame_rect, 1920, 1080).expect("in frame");
        let roundtrip = frame_point_to_screen_position(frame_point, frame_rect, 1920, 1080)
            .expect("frame point should map back");

        assert!((roundtrip.x - pointer.x).abs() < 0.01);
        assert!((roundtrip.y - pointer.y).abs() < 0.01);
    }

    #[test]
    fn preview_pointer_mapping_rejects_letterbox_area() {
        let frame_rect = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(400.0, 300.0));
        let pointer = Pos2::new(80.0, 140.0);

        assert!(pointer_position_to_frame_point(pointer, frame_rect, 1280, 720).is_none());
    }

    #[test]
    fn pointer_and_frame_coordinate_helpers_roundtrip_with_offset_frame_rect() {
        let frame_rect = Rect::from_min_size(Pos2::new(120.0, 48.0), Vec2::new(400.0, 225.0));
        let pointer = Pos2::new(320.0, 160.5);

        let frame_point =
            pointer_position_to_frame_point(pointer, frame_rect, 1920, 1080).expect("frame point");
        let roundtrip = frame_point_to_screen_position(frame_point, frame_rect, 1920, 1080)
            .expect("screen point");

        assert!((frame_point.x - 960.0).abs() < 0.01);
        assert!((frame_point.y - 540.0).abs() < 0.01);
        assert!((roundtrip.x - pointer.x).abs() < 0.01);
        assert!((roundtrip.y - pointer.y).abs() < 0.01);
    }

    #[test]
    fn template_font_choices_keep_system_default_first_and_preserve_selection() {
        let choices = template_font_choices(
            &[
                "BIZ UDPGothic".to_owned(),
                "Noto Sans JP".to_owned(),
                "BIZ UDPGothic".to_owned(),
            ],
            "M PLUS Rounded 1c",
        );

        assert_eq!(choices[0], SYSTEM_DEFAULT_FONT_FAMILY_LABEL);
        assert!(choices.iter().any(|family| family == "M PLUS Rounded 1c"));
        assert_eq!(
            choices
                .iter()
                .filter(|family| family.as_str() == "BIZ UDPGothic")
                .count(),
            1
        );
    }

    #[test]
    fn guide_overlay_state_can_advance_vertical_guides_without_moving_horizontal_origin() {
        let mut state = GuideOverlayState::from_reference_bounds(
            pauseink_domain::Point2 { x: 100.0, y: 200.0 },
            pauseink_domain::Point2 { x: 160.0, y: 280.0 },
        );
        let original_origin = state.horizontal_origin;

        state.advance_to_next_from_bounds(Some((
            pauseink_domain::Point2 { x: 180.0, y: 210.0 },
            pauseink_domain::Point2 { x: 250.0, y: 282.0 },
        )));

        let geometry = state.build_geometry(12.0);
        let first_vertical = geometry
            .vertical_lines
            .iter()
            .find(|line| line.kind == GuideLineKind::Main)
            .expect("main vertical");
        let first_horizontal = geometry.horizontal_lines.first().expect("horizontal line");

        assert_eq!(state.horizontal_origin, original_origin);
        assert!((first_vertical.start.x - 250.0).abs() < 0.01);
        assert!((first_horizontal.start.y - 200.0).abs() < 0.01);
    }

    #[test]
    fn windows_runtime_help_mentions_sidecar_layout_and_path_fallback() {
        let lines = ffmpeg_runtime_windows_help(Path::new("/tmp/pauseink_data/runtime"));

        assert!(lines.iter().any(|line| line.contains("windows-x86_64")));
        assert!(lines.iter().any(|line| line.contains("ffmpeg.exe")));
        assert!(lines.iter().any(|line| line.contains("PATH")));
    }
}
