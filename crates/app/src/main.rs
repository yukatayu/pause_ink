use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke as EguiStroke, Vec2};
use pauseink_app::AppSession;
use pauseink_export::{
    execute_export_with_settings_with_progress, plan_export, ConcreteExportSettings,
    ExportOutputRequest, ExportProgressUpdate, ExportRequest,
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
    load_base_style_presets_overlay, save_base_style_preset_to_path, BaseStylePreset,
    ExportCatalog, OutputKind, RuntimeTier, StylePresetSource,
};
use pauseink_renderer::{render_overlay_rgba, RenderRequest};
use pauseink_template_layout::{
    build_guide_geometry, template_grapheme_scale, GuideGeometry, GuideLineKind, GuidePlacement,
    Point, TemplateSettings, UnderlayMode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use unicode_segmentation::UnicodeSegmentation;

const SYSTEM_DEFAULT_FONT_FAMILY_LABEL: &str = "システム既定";
const DEFAULT_BOTTOM_PANEL_CONTENT_WIDTH: f32 = 1400.0;
const PROJECT_EDITOR_UI_SETTINGS_KEY: &str = "pauseink_editor_ui";
const PROJECT_BASE_STYLE_PRESET_KEY: &str = "base_style";
const PROJECT_ENTRANCE_PRESET_KEY: &str = "entrance";

fn main() -> Result<()> {
    let executable_dir = std::env::current_exe()?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().expect("current dir should resolve"));
    let portable_paths = PortablePaths::from_root(portable_root_from_env(&executable_dir));
    portable_paths.ensure_exists()?;

    let settings = load_settings_or_default(&portable_paths)?;

    let (runtime, runtime_error) =
        match discover_runtime(&portable_paths.runtime_dir, &default_platform_id(), true) {
            Ok(runtime) => (Some(runtime), None),
            Err(error) => (None, Some(error.to_string())),
        };
    let options = eframe::NativeOptions::default();
    let portable_paths_for_app = portable_paths.clone();
    let settings_for_app = settings.clone();
    let runtime_for_app = runtime.clone();
    let runtime_error_for_app = runtime_error.clone();

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
                runtime_error_for_app.clone(),
            )))
        }),
    )?;
    Ok(())
}

fn summarize_runtime_status(runtime: Option<&MediaRuntime>) -> String {
    runtime
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
        .unwrap_or_else(|| "ランタイム: 未検出".to_owned())
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
    placed_origin: Option<Point>,
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
            placed_origin: None,
            placed_slots: None,
            current_slot_index: 0,
            slot_object_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectEditorUiState {
    template: StoredTemplateUiState,
    guide_slope_degrees: f32,
}

impl ProjectEditorUiState {
    fn capture(app: &DesktopApp) -> Self {
        Self {
            template: StoredTemplateUiState::capture(&app.template),
            guide_slope_degrees: app.settings.guide_slope_degrees,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredTemplateUiState {
    text: String,
    font_family: String,
    current_slot_index: usize,
    font_size: f32,
    tracking: f32,
    line_height: f32,
    kana_scale: f32,
    latin_scale: f32,
    punctuation_scale: f32,
    slope_degrees: f32,
    underlay_mode: String,
}

impl StoredTemplateUiState {
    fn capture(template: &TemplatePreviewState) -> Self {
        Self {
            text: template.text.clone(),
            font_family: template.font_family.clone(),
            current_slot_index: template.current_slot_index,
            font_size: template.settings.font_size,
            tracking: template.settings.tracking,
            line_height: template.settings.line_height,
            kana_scale: template.settings.kana_scale,
            latin_scale: template.settings.latin_scale,
            punctuation_scale: template.settings.punctuation_scale,
            slope_degrees: template.settings.slope_degrees,
            underlay_mode: underlay_mode_key(template.settings.underlay_mode).to_owned(),
        }
    }

    fn apply_to(self, template: &mut TemplatePreviewState) {
        template.text = self.text;
        template.font_family = self.font_family;
        template.current_slot_index = self.current_slot_index;
        template.settings.font_size = self.font_size;
        template.settings.tracking = self.tracking;
        template.settings.line_height = self.line_height;
        template.settings.kana_scale = self.kana_scale;
        template.settings.latin_scale = self.latin_scale;
        template.settings.punctuation_scale = self.punctuation_scale;
        template.settings.slope_degrees = self.slope_degrees;
        template.settings.underlay_mode = parse_underlay_mode(&self.underlay_mode);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedBaseStylePresetState {
    preset_id: Option<String>,
    source: Option<StylePresetSource>,
    display_name: Option<String>,
    resolved_snapshot: pauseink_domain::StyleSnapshot,
}

impl SavedBaseStylePresetState {
    fn capture(app: &DesktopApp) -> Self {
        let selected = app
            .style_presets
            .iter()
            .find(|preset| preset.id == app.selected_style_preset_id);
        Self {
            preset_id: if app.selected_style_preset_id.is_empty() {
                None
            } else {
                Some(app.selected_style_preset_id.clone())
            },
            source: selected.map(|preset| preset.source),
            display_name: selected.map(|preset| preset.display_name.clone()),
            resolved_snapshot: app.session.active_style.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedEntrancePresetState {
    preset_id: Option<String>,
    resolved_snapshot: StoredEntranceBehavior,
}

impl SavedEntrancePresetState {
    fn capture(app: &DesktopApp) -> Self {
        Self {
            preset_id: if app.selected_style_preset_id.is_empty() {
                None
            } else {
                Some(app.selected_style_preset_id.clone())
            },
            resolved_snapshot: StoredEntranceBehavior::from_domain(&app.session.active_entrance),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEntranceBehavior {
    kind: StoredEntranceKind,
    duration_mode: StoredEntranceDurationMode,
    duration_ms: i64,
    speed_scalar: f32,
}

impl StoredEntranceBehavior {
    fn from_domain(entrance: &pauseink_domain::EntranceBehavior) -> Self {
        Self {
            kind: match entrance.kind {
                pauseink_domain::EntranceKind::PathTrace => StoredEntranceKind::PathTrace,
                pauseink_domain::EntranceKind::Instant => StoredEntranceKind::Instant,
                pauseink_domain::EntranceKind::Wipe => StoredEntranceKind::Wipe,
                pauseink_domain::EntranceKind::Dissolve => StoredEntranceKind::Dissolve,
            },
            duration_mode: match entrance.duration_mode {
                pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength => {
                    StoredEntranceDurationMode::LengthProportional
                }
                pauseink_domain::EntranceDurationMode::FixedTotalDuration => {
                    StoredEntranceDurationMode::FixedTotalDuration
                }
            },
            duration_ms: media_duration_to_millis(entrance.duration),
            speed_scalar: entrance.speed_scalar,
        }
    }

    fn into_domain(self) -> pauseink_domain::EntranceBehavior {
        let mut entrance = pauseink_domain::EntranceBehavior::default();
        entrance.kind = match self.kind {
            StoredEntranceKind::PathTrace => pauseink_domain::EntranceKind::PathTrace,
            StoredEntranceKind::Instant => pauseink_domain::EntranceKind::Instant,
            StoredEntranceKind::Wipe => pauseink_domain::EntranceKind::Wipe,
            StoredEntranceKind::Dissolve => pauseink_domain::EntranceKind::Dissolve,
        };
        entrance.duration_mode = match self.duration_mode {
            StoredEntranceDurationMode::LengthProportional => {
                pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength
            }
            StoredEntranceDurationMode::FixedTotalDuration => {
                pauseink_domain::EntranceDurationMode::FixedTotalDuration
            }
        };
        entrance.duration = pauseink_domain::MediaDuration::from_millis(self.duration_ms.max(0));
        entrance.speed_scalar = self.speed_scalar;
        entrance
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredEntranceKind {
    PathTrace,
    Instant,
    Wipe,
    Dissolve,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StoredEntranceDurationMode {
    LengthProportional,
    FixedTotalDuration,
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
            next_cell_origin_x: max.x,
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
        if let Some((_, max)) = bounds {
            self.next_cell_origin_x = max.x;
        } else {
            self.next_cell_origin_x += self.cell_width;
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GuideCaptureState {
    reference_object_id: Option<pauseink_domain::GlyphObjectId>,
    in_progress: bool,
    finalize_pending: bool,
}

impl GuideCaptureState {
    fn start(&mut self) {
        self.in_progress = true;
        self.finalize_pending = false;
    }

    fn current_target_object_id(&self) -> Option<pauseink_domain::GlyphObjectId> {
        self.reference_object_id.clone()
    }

    fn record_committed_object(&mut self, object_id: pauseink_domain::GlyphObjectId) {
        self.in_progress = true;
        self.reference_object_id = Some(object_id);
    }

    fn note_modifier_release(
        &mut self,
        while_dragging: bool,
    ) -> Option<pauseink_domain::GlyphObjectId> {
        if !self.in_progress {
            return None;
        }

        if while_dragging {
            self.finalize_pending = true;
            None
        } else {
            self.take_finalized_object_id()
        }
    }

    fn take_if_pending_after_commit(&mut self) -> Option<pauseink_domain::GlyphObjectId> {
        if self.finalize_pending {
            self.take_finalized_object_id()
        } else {
            None
        }
    }

    fn cancel(&mut self) {
        *self = Self::default();
    }

    fn take_finalized_object_id(&mut self) -> Option<pauseink_domain::GlyphObjectId> {
        let object_id = self.reference_object_id.take();
        self.in_progress = false;
        self.finalize_pending = false;
        object_id
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
    receiver: Receiver<ExportThreadMessage>,
    summary: String,
    output_path: PathBuf,
    progress_fraction: f32,
    progress_label: String,
}

enum ExportThreadMessage {
    Progress(ExportProgressUpdate),
    Finished(Result<pauseink_export::ExportExecutionResult, String>),
}

fn export_progress_hint(progress_label: &str) -> &'static str {
    if progress_label.contains("一時ファイルを整理中") {
        "書き出し自体は終わっており、作業用の一時ファイルを削除しています。"
    } else if progress_label.contains("最終処理中") {
        "ffmpeg がコンテナの最終化を行っています。完了までもう少し待ってください。"
    } else if progress_label.contains("フレーム生成中")
        || progress_label.contains("フレームを準備中")
    {
        "各フレーム用の透明オーバーレイ画像を生成しています。動画が長いほど時間がかかります。"
    } else if progress_label.contains("書き出し中") {
        "ffmpeg が動画をエンコードしています。出力形式や解像度によって時間が変わります。"
    } else if progress_label.contains("PNG 連番") {
        "生成済みフレームを連番 PNG として配置しています。"
    } else {
        "書き出し処理を進めています。"
    }
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
    last_runtime_error: Option<String>,
    logs: Vec<String>,
    bottom_panel_content_width: f32,
    local_font_families: Vec<String>,
    style_presets: Vec<BaseStylePreset>,
    selected_style_preset_id: String,
    preset_editor_id: String,
    preset_editor_name: String,
    template: TemplatePreviewState,
    export: ExportState,
    guide_state: Option<GuideOverlayState>,
    guide_capture_state: GuideCaptureState,
    guide_geometry: Option<GuideGeometry>,
    last_committed_object_bounds: Option<(pauseink_domain::Point2, pauseink_domain::Point2)>,
    bottom_tab: BottomTab,
    preview_texture: Option<egui::TextureHandle>,
    preview_key: Option<(PathBuf, i64, u32, u32)>,
    overlay_texture: Option<egui::TextureHandle>,
    overlay_key: Option<(
        i64,
        usize,
        usize,
        u32,
        u32,
        u32,
        u32,
        Option<pauseink_domain::MediaTime>,
    )>,
    preview_visible_batch_anchor: Option<pauseink_domain::MediaTime>,
    canvas_drag_active: bool,
    guide_capture_armed: bool,
    guide_modifier_was_down: bool,
    guide_modifier_used_for_stroke: bool,
    guide_modifier_tap_suppressed: bool,
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
        runtime_error: Option<String>,
    ) -> Self {
        let recovery_prompt_open = portable_paths.autosave_file("recovery_latest").exists();
        let provider = runtime.clone().map(FfprobeMediaProvider::new);
        let runtime_capabilities = provider
            .as_ref()
            .and_then(|provider| provider.capabilities().ok());
        let runtime_status = summarize_runtime_status(runtime.as_ref());
        let mut font_dirs = vec![portable_paths.google_fonts_cache_dir()];
        font_dirs.extend(settings.local_font_dirs.clone());
        let local_font_families = discover_local_font_families(&font_dirs);
        let export_catalog = ExportCatalog::load_builtin_from_dir(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/export_profiles"),
        )
        .ok();
        let style_presets = load_style_presets(&portable_paths).unwrap_or_default();
        let mut session = AppSession::with_history_limit(settings.history_depth);
        session.active_style.stabilization_strength =
            (settings.stroke_stabilization_default as f32 / 100.0).clamp(0.0, 1.0);
        let mut export = ExportState {
            catalog: export_catalog,
            ..ExportState::default()
        };
        initialize_export_selection(&mut export);

        let selected_style_preset_id = style_presets
            .first()
            .map(|preset| preset.id.clone())
            .unwrap_or_default();
        let (preset_editor_id, preset_editor_name) =
            preset_editor_fields_from_selection(&style_presets, &selected_style_preset_id);

        let mut app = Self {
            session,
            portable_paths,
            settings,
            runtime,
            provider,
            runtime_capabilities,
            runtime_status,
            last_runtime_error: runtime_error,
            logs: Vec::new(),
            bottom_panel_content_width: DEFAULT_BOTTOM_PANEL_CONTENT_WIDTH,
            local_font_families,
            selected_style_preset_id,
            preset_editor_id,
            preset_editor_name,
            style_presets,
            template: TemplatePreviewState::default(),
            export,
            guide_state: None,
            guide_capture_state: GuideCaptureState::default(),
            guide_geometry: None,
            last_committed_object_bounds: None,
            bottom_tab: BottomTab::Outline,
            preview_texture: None,
            preview_key: None,
            overlay_texture: None,
            overlay_key: None,
            preview_visible_batch_anchor: None,
            canvas_drag_active: false,
            guide_capture_armed: false,
            guide_modifier_was_down: false,
            guide_modifier_used_for_stroke: false,
            guide_modifier_tap_suppressed: false,
            recovery_prompt_open,
            preferences_open: false,
            cache_manager_open: false,
            runtime_diagnostics_open: false,
            font_config_dirty: true,
            google_font_input: String::new(),
            pending_export: None,
            last_update_at: Instant::now(),
            last_autosave_at: Instant::now(),
        };
        app.restore_app_ui_state_from_settings();
        app
    }

    fn push_log(&mut self, message: impl Into<String>) {
        self.logs.push(message.into());
        if self.logs.len() > 200 {
            let overflow = self.logs.len() - 200;
            self.logs.drain(0..overflow);
        }
    }

    fn mark_project_ui_dirty(&mut self) {
        self.session.dirty = true;
    }

    fn current_preview_force_visible_batch(&self) -> Option<pauseink_domain::MediaTime> {
        let is_playing = self
            .session
            .playback
            .as_ref()
            .is_some_and(|playback| playback.is_playing);
        if is_playing {
            None
        } else {
            self.preview_visible_batch_anchor
        }
    }

    fn set_preview_force_visible_batch(&mut self, anchor: pauseink_domain::MediaTime) {
        if self.preview_visible_batch_anchor == Some(anchor) {
            return;
        }
        self.preview_visible_batch_anchor = Some(anchor);
        self.overlay_key = None;
    }

    fn finalize_preview_force_visible_batch(&mut self) {
        if self.preview_visible_batch_anchor.take().is_some() {
            self.overlay_key = None;
        }
    }

    fn cancel_active_canvas_stroke(&mut self) {
        if !self.canvas_drag_active
            && self.session.current_stroke_preview().is_none()
            && !self.guide_capture_state.in_progress
        {
            return;
        }

        self.session.cancel_stroke();
        self.canvas_drag_active = false;
        self.guide_capture_armed = false;
        self.guide_capture_state.cancel();
        self.guide_modifier_used_for_stroke = false;
    }

    fn play_transport(&mut self) {
        self.finalize_preview_force_visible_batch();
        self.cancel_active_canvas_stroke();
        if self.session.play() {
            self.preview_key = None;
            self.overlay_key = None;
        }
    }

    fn pause_transport(&mut self) {
        if self.session.pause() {
            self.preview_key = None;
            self.overlay_key = None;
        }
    }

    fn seek_transport(&mut self, time: pauseink_domain::MediaTime) {
        self.finalize_preview_force_visible_batch();
        self.cancel_active_canvas_stroke();
        if self.session.seek(time) {
            self.preview_key = None;
            self.overlay_key = None;
        }
    }

    fn selected_style_preset(&self) -> Option<&BaseStylePreset> {
        self.style_presets
            .iter()
            .find(|preset| preset.id == self.selected_style_preset_id)
    }

    fn selected_style_preset_label(&self) -> String {
        self.selected_style_preset()
            .map(|preset| preset.display_name.clone())
            .or_else(|| {
                (!self.selected_style_preset_id.is_empty())
                    .then(|| format!("保存済み: {}", self.selected_style_preset_id))
            })
            .unwrap_or_else(|| "未選択".to_owned())
    }

    fn sync_preset_editor_fields_from_selection(&mut self) {
        let (preset_id, preset_name) = preset_editor_fields_from_selection(
            &self.style_presets,
            &self.selected_style_preset_id,
        );
        self.preset_editor_id = preset_id;
        self.preset_editor_name = preset_name;
    }

    fn reload_style_presets(&mut self) {
        let previous_selection = self.selected_style_preset_id.clone();
        match load_style_presets(&self.portable_paths) {
            Ok(style_presets) => {
                self.style_presets = style_presets;
                if self.selected_style_preset_id.is_empty() {
                    self.selected_style_preset_id = self
                        .style_presets
                        .first()
                        .map(|preset| preset.id.clone())
                        .unwrap_or_default();
                } else if !self
                    .style_presets
                    .iter()
                    .any(|preset| preset.id == self.selected_style_preset_id)
                {
                    self.selected_style_preset_id = self
                        .style_presets
                        .iter()
                        .find(|preset| preset.id == previous_selection)
                        .or_else(|| self.style_presets.first())
                        .map(|preset| preset.id.clone())
                        .unwrap_or_default();
                }
                self.sync_preset_editor_fields_from_selection();
            }
            Err(error) => self.push_log(format!("preset 再読込失敗: {error}")),
        }
    }

    fn save_user_style_preset(&mut self, overwrite_selected: bool) {
        let selected_user_preset = self
            .selected_style_preset()
            .filter(|preset| preset.source == StylePresetSource::User)
            .cloned();
        let raw_id = if overwrite_selected {
            selected_user_preset
                .as_ref()
                .map(|preset| preset.id.clone())
                .unwrap_or_else(|| self.preset_editor_id.clone())
        } else {
            self.preset_editor_id.clone()
        };
        let preset_id = sanitize_style_preset_id(&raw_id);
        if preset_id.is_empty() {
            self.push_log("preset 保存失敗: preset ID が空です。");
            return;
        }

        let display_name = if overwrite_selected {
            if self.preset_editor_name.trim().is_empty() {
                selected_user_preset
                    .as_ref()
                    .map(|preset| preset.display_name.clone())
                    .unwrap_or_else(|| preset_id.clone())
            } else {
                self.preset_editor_name.trim().to_owned()
            }
        } else if self.preset_editor_name.trim().is_empty() {
            preset_id.clone()
        } else {
            self.preset_editor_name.trim().to_owned()
        };

        let preset = BaseStylePreset {
            id: preset_id.clone(),
            display_name: display_name.clone(),
            thickness: Some(self.session.active_style.thickness),
            color_rgba: Some([
                self.session.active_style.color.r,
                self.session.active_style.color.g,
                self.session.active_style.color.b,
                255,
            ]),
            opacity: Some(self.session.active_style.opacity),
            outline: Some(self.session.active_style.outline.clone()),
            drop_shadow: Some(self.session.active_style.drop_shadow.clone()),
            glow: Some(self.session.active_style.glow.clone()),
            blend_mode: Some(self.session.active_style.blend_mode),
            stabilization_strength: Some(self.session.active_style.stabilization_strength),
            entrance: Some(self.session.active_entrance.clone()),
            source: StylePresetSource::User,
            file_path: None,
        };
        let preset_path = if overwrite_selected {
            selected_user_preset
                .as_ref()
                .and_then(|preset| preset.file_path.clone())
                .unwrap_or_else(|| {
                    self.portable_paths
                        .user_style_presets_dir()
                        .join(format!("{preset_id}.json5"))
                })
        } else {
            self.portable_paths
                .user_style_presets_dir()
                .join(format!("{preset_id}.json5"))
        };

        if !overwrite_selected && preset_path.exists() {
            self.push_log(format!(
                "user preset 保存失敗: `{preset_id}` は既に存在します。上書き保存を使ってください。"
            ));
            return;
        }

        match save_base_style_preset_to_path(&preset_path, &preset) {
            Ok(()) => {
                self.selected_style_preset_id = preset_id;
                self.preset_editor_name = display_name.clone();
                self.reload_style_presets();
                self.mark_project_ui_dirty();
                self.push_log(format!("user preset 保存: {display_name}"));
            }
            Err(error) => self.push_log(format!("user preset 保存失敗: {error}")),
        }
    }

    fn delete_selected_user_style_preset(&mut self) {
        let Some(preset) = self.selected_style_preset().cloned() else {
            self.push_log("preset 削除失敗: 選択中の preset がありません。");
            return;
        };
        if preset.source != StylePresetSource::User {
            self.push_log("preset 削除失敗: built-in preset は削除できません。");
            return;
        }

        let path = preset.file_path.clone().unwrap_or_else(|| {
            self.portable_paths
                .user_style_presets_dir()
                .join(format!("{}.json5", sanitize_style_preset_id(&preset.id)))
        });
        match fs::remove_file(&path) {
            Ok(()) => {
                self.reload_style_presets();
                if self
                    .style_presets
                    .iter()
                    .any(|candidate| candidate.id == preset.id)
                {
                    self.selected_style_preset_id = preset.id.clone();
                } else {
                    self.selected_style_preset_id = self
                        .style_presets
                        .first()
                        .map(|candidate| candidate.id.clone())
                        .unwrap_or_default();
                }
                self.sync_preset_editor_fields_from_selection();
                self.mark_project_ui_dirty();
                self.push_log(format!("user preset 削除: {}", preset.display_name));
            }
            Err(error) => self.push_log(format!("user preset 削除失敗: {error}")),
        }
    }

    fn persist_project_ui_state_into_document(&mut self) {
        let editor_state = serde_json::to_value(ProjectEditorUiState::capture(self))
            .expect("project editor ui state should serialize");
        let base_style_state = serde_json::to_value(SavedBaseStylePresetState::capture(self))
            .expect("base style preset state should serialize");
        let entrance_state = serde_json::to_value(SavedEntrancePresetState::capture(self))
            .expect("entrance preset state should serialize");
        let settings = ensure_object_value(&mut self.session.document.project.settings);
        settings.insert(PROJECT_EDITOR_UI_SETTINGS_KEY.to_owned(), editor_state);

        let presets = ensure_object_value(&mut self.session.document.project.presets);
        presets.insert(PROJECT_BASE_STYLE_PRESET_KEY.to_owned(), base_style_state);
        presets.insert(PROJECT_ENTRANCE_PRESET_KEY.to_owned(), entrance_state);
    }

    fn persist_app_ui_state_into_settings(&mut self) {
        self.settings.editor_ui_state = Some(
            serde_json::to_value(ProjectEditorUiState::capture(self))
                .expect("settings editor ui state should serialize"),
        );
        self.settings.base_style_state = Some(
            serde_json::to_value(SavedBaseStylePresetState::capture(self))
                .expect("settings base style state should serialize"),
        );
        self.settings.entrance_state = Some(
            serde_json::to_value(SavedEntrancePresetState::capture(self))
                .expect("settings entrance state should serialize"),
        );
    }

    fn restore_project_ui_state_from_document(&mut self) {
        if let Some(editor_state) = self
            .session
            .document
            .project
            .settings
            .get(PROJECT_EDITOR_UI_SETTINGS_KEY)
            .cloned()
            .and_then(|value| serde_json::from_value::<ProjectEditorUiState>(value).ok())
        {
            editor_state.template.apply_to(&mut self.template);
            self.settings.guide_slope_degrees = editor_state.guide_slope_degrees;
        }

        if let Some(base_style_state) = self
            .session
            .document
            .project
            .presets
            .get(PROJECT_BASE_STYLE_PRESET_KEY)
            .cloned()
            .and_then(|value| serde_json::from_value::<SavedBaseStylePresetState>(value).ok())
        {
            self.session.active_style = base_style_state.resolved_snapshot;
            if let Some(preset_id) = base_style_state.preset_id {
                self.selected_style_preset_id = preset_id;
            }
        }

        if let Some(entrance_state) = self
            .session
            .document
            .project
            .presets
            .get(PROJECT_ENTRANCE_PRESET_KEY)
            .cloned()
            .and_then(|value| serde_json::from_value::<SavedEntrancePresetState>(value).ok())
        {
            self.session.active_entrance = entrance_state.resolved_snapshot.into_domain();
            if let Some(preset_id) = entrance_state.preset_id {
                self.selected_style_preset_id = preset_id;
            }
        }

        self.font_config_dirty = true;
        self.reset_template_slots();
        self.refresh_guide_geometry();
        self.sync_preset_editor_fields_from_selection();
    }

    fn restore_app_ui_state_from_settings(&mut self) {
        if let Some(editor_state) = self
            .settings
            .editor_ui_state
            .clone()
            .and_then(|value| serde_json::from_value::<ProjectEditorUiState>(value).ok())
        {
            editor_state.template.apply_to(&mut self.template);
            self.settings.guide_slope_degrees = editor_state.guide_slope_degrees;
        }

        if let Some(base_style_state) = self
            .settings
            .base_style_state
            .clone()
            .and_then(|value| serde_json::from_value::<SavedBaseStylePresetState>(value).ok())
        {
            self.session.active_style = base_style_state.resolved_snapshot;
            if let Some(preset_id) = base_style_state.preset_id {
                self.selected_style_preset_id = preset_id;
            }
        }

        if let Some(entrance_state) = self
            .settings
            .entrance_state
            .clone()
            .and_then(|value| serde_json::from_value::<SavedEntrancePresetState>(value).ok())
        {
            self.session.active_entrance = entrance_state.resolved_snapshot.into_domain();
            if let Some(preset_id) = entrance_state.preset_id {
                self.selected_style_preset_id = preset_id;
            }
        }

        self.font_config_dirty = true;
        self.reset_template_slots();
        self.refresh_guide_geometry();
        self.sync_preset_editor_fields_from_selection();
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

    fn finalize_guide_capture_with_object(
        &mut self,
        object_id: Option<pauseink_domain::GlyphObjectId>,
    ) {
        self.guide_capture_state.cancel();
        self.guide_capture_armed = false;
        if let Some(object_id) = object_id {
            self.capture_guide_from_object(&object_id);
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

    fn clear_guide_state(&mut self) {
        self.guide_state = None;
        self.guide_geometry = None;
        self.last_committed_object_bounds = None;
        self.guide_capture_state.cancel();
        self.guide_capture_armed = false;
        self.guide_modifier_was_down = false;
        self.guide_modifier_used_for_stroke = false;
        self.guide_modifier_tap_suppressed = false;
        self.push_log("ガイドを解除しました。");
    }

    fn move_template_slot(&mut self, delta: isize) {
        let slot_len = self.template.placed_slots.as_ref().map_or(0, Vec::len);
        let next_index =
            step_template_slot_index(self.template.current_slot_index, slot_len, delta);
        if next_index != self.template.current_slot_index {
            self.template.current_slot_index = next_index;
            self.mark_project_ui_dirty();
        }
    }

    fn reset_template_slots(&mut self) {
        self.template.placed_origin = None;
        self.template.placed_slots = None;
        self.template.slot_object_ids.clear();
        self.template.current_slot_index = 0;
    }

    fn refresh_placed_template_slots(&mut self, ctx: &egui::Context) {
        let Some(origin) = self.template.placed_origin else {
            return;
        };

        let slots = self.template_slots_at_origin(ctx, origin);
        self.template.slot_object_ids.resize(slots.len(), None);
        self.template.current_slot_index = if slots.is_empty() {
            0
        } else {
            self.template
                .current_slot_index
                .min(slots.len().saturating_sub(1))
        };
        self.template.placed_slots = Some(slots);
    }

    fn current_style_target_object_id(&self) -> Option<pauseink_domain::GlyphObjectId> {
        if self.template.placed_slots.is_some() {
            return self
                .template
                .slot_object_ids
                .get(self.template.current_slot_index)
                .cloned()
                .flatten();
        }

        if self.guide_capture_state.in_progress {
            return self.guide_capture_state.current_target_object_id();
        }

        self.session.selected_object_id.clone()
    }

    fn sync_active_style_to_current_object(&mut self) {
        let Some(object_id) = self.current_style_target_object_id() else {
            return;
        };

        if self
            .session
            .overwrite_glyph_object_style(&object_id, self.session.active_style.clone())
        {
            self.overlay_key = None;
        }
    }

    fn sync_active_entrance_to_current_object(&mut self) {
        let Some(object_id) = self.current_style_target_object_id() else {
            return;
        };

        if self
            .session
            .overwrite_glyph_object_entrance(&object_id, self.session.active_entrance.clone())
        {
            self.overlay_key = None;
        }
    }

    fn handle_guide_modifier_tap(&mut self, ctx: &egui::Context) {
        let modifier_active = self.guide_modifier_active(ctx);
        if modifier_active && !self.guide_modifier_was_down && !self.guide_modifier_tap_suppressed {
            self.guide_modifier_used_for_stroke = false;
        }
        if !modifier_active && self.guide_modifier_was_down {
            if self.guide_capture_state.in_progress {
                let object_id = self
                    .guide_capture_state
                    .note_modifier_release(self.canvas_drag_active);
                if !self.canvas_drag_active {
                    self.finalize_guide_capture_with_object(object_id);
                    self.guide_modifier_used_for_stroke = false;
                }
            } else if self.guide_modifier_tap_suppressed {
                self.guide_modifier_tap_suppressed = false;
                self.guide_modifier_used_for_stroke = false;
            } else if !self.guide_modifier_used_for_stroke
                && !ctx.egui_wants_keyboard_input()
                && !self.template.placement_armed
            {
                self.advance_guide_to_next_character();
            }
        }
        self.guide_modifier_was_down = modifier_active;
    }

    fn perform_undo(&mut self) {
        if let Err(error) = self.session.undo() {
            self.push_log(format!("undo 失敗: {error:#}"));
        } else {
            self.guide_capture_state.cancel();
        }
    }

    fn perform_redo(&mut self) {
        if let Err(error) = self.session.redo() {
            self.push_log(format!("redo 失敗: {error:#}"));
        } else {
            self.guide_capture_state.cancel();
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
            self.guide_modifier_tap_suppressed = true;
            self.guide_modifier_used_for_stroke = true;
            self.perform_undo();
        }
        if redo_shift || redo_y {
            self.guide_modifier_tap_suppressed = true;
            self.guide_modifier_used_for_stroke = true;
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
                pauseink_domain::RgbaColor::new(color[0], color[1], color[2], 255);
        }
        if let Some(opacity) = preset.opacity {
            self.session.active_style.opacity = opacity;
        } else if let Some(color) = preset.color_rgba {
            self.session.active_style.opacity = color[3] as f32 / 255.0;
        }
        if let Some(outline) = preset.outline {
            self.session.active_style.outline = outline;
        }
        if let Some(drop_shadow) = preset.drop_shadow {
            self.session.active_style.drop_shadow = drop_shadow;
        }
        if let Some(glow) = preset.glow {
            self.session.active_style.glow = glow;
        }
        if let Some(blend_mode) = preset.blend_mode {
            self.session.active_style.blend_mode = blend_mode;
        }
        if let Some(stabilization_strength) = preset.stabilization_strength {
            self.session.active_style.stabilization_strength = stabilization_strength;
        }
        if let Some(entrance) = preset.entrance {
            self.session.active_entrance = entrance;
        }
        self.sync_active_style_to_current_object();
        self.sync_active_entrance_to_current_object();
        self.mark_project_ui_dirty();
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

    fn apply_runtime_discovery(
        &mut self,
        runtime: Option<MediaRuntime>,
        runtime_error: Option<String>,
    ) {
        self.runtime_status = summarize_runtime_status(runtime.as_ref());
        self.provider = runtime.clone().map(FfprobeMediaProvider::new);
        self.runtime = runtime;
        self.runtime_capabilities = None;
        self.last_runtime_error = runtime_error;
    }

    fn rediscover_runtime(&mut self) {
        match discover_runtime(
            &self.portable_paths.runtime_dir,
            &default_platform_id(),
            true,
        ) {
            Ok(runtime) => {
                self.apply_runtime_discovery(Some(runtime.clone()), None);
                self.push_log(format!(
                    "runtime 再検出成功: {} / {}",
                    runtime.ffmpeg_path.display(),
                    runtime.ffprobe_path.display()
                ));
                self.refresh_runtime_capabilities();
            }
            Err(error) => {
                let message = error.to_string();
                self.apply_runtime_discovery(None, Some(message.clone()));
                self.push_log(format!("runtime 再検出失敗: {error}"));
            }
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
        let Some(_) = self.pending_export.as_ref() else {
            return;
        };

        loop {
            let message = {
                let pending = self.pending_export.as_ref().expect("checked above");
                pending.receiver.try_recv()
            };

            match message {
                Ok(ExportThreadMessage::Progress(update)) => {
                    if let Some(pending) = self.pending_export.as_mut() {
                        pending.progress_fraction = pending
                            .progress_fraction
                            .max(update.fraction.clamp(0.0, 1.0));
                        pending.progress_label = update.stage_label;
                    }
                }
                Ok(ExportThreadMessage::Finished(result)) => {
                    let pending = self
                        .pending_export
                        .take()
                        .expect("pending export should exist");
                    let summary = pending.summary;
                    let output_path = pending.output_path;
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
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.push_log("export worker が切断されました。");
                    self.pending_export = None;
                    break;
                }
            }
        }
    }

    fn start_export(&mut self) {
        if self.pending_export.is_some() {
            self.push_log("export はすでに実行中です。");
            return;
        }
        self.finalize_preview_force_visible_batch();
        self.cancel_active_canvas_stroke();

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
            let progress_sender = sender.clone();
            let result = execute_export_with_settings_with_progress(
                &runtime,
                &capabilities,
                &snapshot,
                &settings,
                &request,
                move |progress| {
                    let _ = progress_sender.send(ExportThreadMessage::Progress(progress));
                },
            )
            .map_err(|error| error.to_string());
            if result.is_ok() {
                let _ = sender.send(ExportThreadMessage::Progress(ExportProgressUpdate {
                    fraction: 0.995,
                    stage_label: "3/3 一時ファイルを整理中".to_owned(),
                }));
            }
            let _ = fs::remove_dir_all(&working_directory);
            let _ = sender.send(ExportThreadMessage::Finished(result));
        });

        self.push_log(format!("export 開始: {}", output_path.display()));
        self.pending_export = Some(PendingExportJob {
            receiver,
            summary,
            output_path,
            progress_fraction: 0.0,
            progress_label: "開始待ち".to_owned(),
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
        self.finalize_preview_force_visible_batch();
        self.cancel_active_canvas_stroke();

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
        self.finalize_preview_force_visible_batch();
        self.cancel_active_canvas_stroke();
        match AppSession::load_project_from_path(&path) {
            Ok(mut session) => {
                session.set_history_limit(self.settings.history_depth);
                session.active_style.stabilization_strength =
                    (self.settings.stroke_stabilization_default as f32 / 100.0).clamp(0.0, 1.0);
                self.session = session;
                self.clear_guide_state();
                self.restore_app_ui_state_from_settings();
                self.restore_project_ui_state_from_document();
                self.template.placement_armed = false;
                self.preview_key = None;
                self.overlay_key = None;
                self.push_log(format!("プロジェクトを読込: {}", path.display()));
            }
            Err(error) => self.push_log(format!("プロジェクト読込失敗: {error:#}")),
        }
    }

    fn save_project(&mut self, path: PathBuf) {
        self.finalize_preview_force_visible_batch();
        self.persist_project_ui_state_into_document();
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
        let preview_force_visible_batch = self.current_preview_force_visible_batch();
        let key = (
            self.session.current_time().ticks,
            self.session.project.strokes.len(),
            self.session.project.clear_events.len(),
            target_width,
            target_height,
            source_width,
            source_height,
            preview_force_visible_batch,
        );
        if self.overlay_key.as_ref() == Some(&key) {
            return;
        }

        match render_overlay_rgba(&RenderRequest {
            project: &self.session.project,
            time: self.session.current_time(),
            preview_force_visible_batch,
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
        if self
            .session
            .playback
            .as_ref()
            .is_some_and(|playback| playback.is_playing)
        {
            self.cancel_active_canvas_stroke();
            return;
        }

        let pointer_position = response
            .interact_pointer_pos()
            .or_else(|| ctx.input(|input| input.pointer.hover_pos()));
        let primary_press_position = current_frame_primary_press_position(ctx);

        if self.template.placement_armed {
            if response.clicked() {
                if let Some(pointer_position) = pointer_position {
                    let relative = Pos2::new(
                        pointer_position.x - frame_rect.left(),
                        pointer_position.y - frame_rect.top(),
                    );
                    self.template.placed_origin = Some(Point::new(relative.x, relative.y));
                    self.template.current_slot_index = 0;
                    self.template.slot_object_ids.clear();
                    self.refresh_placed_template_slots(ctx);
                    self.template.placement_armed = false;
                    self.push_log("テンプレート配置を確定しました。");
                }
            }
            return;
        }

        let pointer_down = ctx.input(|input| input.pointer.primary_down());
        let press_started_on_canvas =
            primary_press_position.is_some_and(|position| response.rect.contains(position));
        let mut started_stroke_this_frame = false;

        if !self.canvas_drag_active && press_started_on_canvas {
            let batch_anchor = self.session.current_time();
            self.guide_capture_armed =
                self.guide_modifier_active(ctx) && self.template.placed_slots.is_none();
            if self.guide_capture_armed {
                self.guide_capture_state.start();
                self.guide_modifier_used_for_stroke = true;
            }
            if let Some(pointer_position) = primary_press_position.or(pointer_position) {
                if let Some(frame_point) = pointer_position_to_frame_point(
                    pointer_position,
                    frame_rect,
                    frame_width,
                    frame_height,
                ) {
                    self.set_preview_force_visible_batch(batch_anchor);
                    self.session.begin_stroke(frame_point, batch_anchor);
                    self.canvas_drag_active = true;
                    started_stroke_this_frame = true;
                }
            }
        }

        if self.canvas_drag_active {
            if !started_stroke_this_frame {
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
            }

            if !pointer_down {
                let template_target_object = self
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
                let target_object = if self.guide_capture_state.in_progress
                    && self.template.placed_slots.is_none()
                {
                    self.guide_capture_state.current_target_object_id()
                } else {
                    template_target_object
                };

                match self.session.commit_stroke_into_object(target_object) {
                    Ok(Some(object_id)) => {
                        self.last_committed_object_bounds = self.session.object_bounds(&object_id);
                        if self.guide_capture_armed {
                            self.guide_capture_state
                                .record_committed_object(object_id.clone());
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
                if self.guide_capture_state.finalize_pending {
                    let object_id = self.guide_capture_state.take_if_pending_after_commit();
                    self.finalize_guide_capture_with_object(object_id);
                    self.guide_modifier_used_for_stroke = false;
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

        for line in &guide.horizontal_lines {
            let (line_start, line_end) =
                extend_horizontal_guide_line_to_frame_width(line, frame_width);
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
                    x: line_start.x,
                    y: line_start.y,
                },
                frame_rect,
                frame_width,
                frame_height,
            ) else {
                continue;
            };
            let Some(end) = frame_point_to_screen_position(
                pauseink_domain::Point2 {
                    x: line_end.x,
                    y: line_end.y,
                },
                frame_rect,
                frame_width,
                frame_height,
            ) else {
                continue;
            };
            painter.line_segment([start, end], stroke);
        }

        for line in &guide.vertical_lines {
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

    fn draw_live_stroke_preview(
        &self,
        painter: &egui::Painter,
        frame_rect: Rect,
        frame_width: u32,
        frame_height: u32,
    ) {
        let Some(preview) = self.session.current_stroke_preview() else {
            return;
        };

        let color = draft_preview_color(&preview.style);
        let stroke_width = live_preview_stroke_width(
            preview.style.thickness,
            frame_rect,
            frame_width,
            frame_height,
        );
        let stroke = EguiStroke::new(stroke_width, color);
        let points = preview
            .points
            .into_iter()
            .filter_map(|point| {
                frame_point_to_screen_position(point, frame_rect, frame_width, frame_height)
            })
            .collect::<Vec<_>>();

        if points.len() >= 2 {
            for window in points.windows(2) {
                painter.line_segment([window[0], window[1]], stroke);
            }
        } else if let Some(point) = points.first() {
            painter.circle_filled(*point, live_preview_dot_radius(stroke_width), color);
        }
    }

    fn save_settings(&mut self) {
        self.persist_app_ui_state_into_settings();
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

        self.persist_project_ui_state_into_document();
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
                self.restore_app_ui_state_from_settings();
                self.restore_project_ui_state_from_document();
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
                    self.mark_project_ui_dirty();
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
                    self.rediscover_runtime();
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
                if let Some(error) = &self.last_runtime_error {
                    ui.label(format!("最後の検出エラー: {error}"));
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
                ui.heading(ffmpeg_runtime_help_heading(std::env::consts::OS));
                for line in ffmpeg_runtime_help(
                    &self.portable_paths.runtime_dir,
                    std::env::consts::OS,
                    &default_platform_id(),
                ) {
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

        self.draw_pending_export_progress(ui);
    }

    fn draw_export_queue_tab(&self, ui: &mut egui::Ui) {
        if let Some(pending) = &self.pending_export {
            ui.label(format!("実行中: {}", pending.summary));
            ui.label(&pending.progress_label);
            ui.small(export_progress_hint(&pending.progress_label));
            ui.add(
                egui::ProgressBar::new(pending.progress_fraction)
                    .desired_width(ui.available_width().max(160.0))
                    .show_percentage(),
            );
            ui.label(pending.output_path.display().to_string());
            ui.separator();
        }

        if self.export.jobs.is_empty() {
            ui.label("書き出し履歴はまだありません。");
            return;
        }

        for job in &self.export.jobs {
            ui.label(format!("{} / {}", job.summary, job.status));
            ui.label(job.output_path.display().to_string());
            if job.software_fallback_used {
                ui.label("HW path 失敗後に software fallback で完了");
            }
            ui.separator();
        }
    }

    fn draw_bottom_tab_contents(&self, ui: &mut egui::Ui) {
        match self.bottom_tab {
            BottomTab::Outline => {
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
            }
            BottomTab::PageEvents => {
                for clear in &self.session.project.clear_events {
                    ui.label(format!(
                        "{} / t={} / {:?}",
                        clear.id.0, clear.time.ticks, clear.kind
                    ));
                }
                if self.session.project.clear_events.is_empty() {
                    ui.label("clear event はまだありません。");
                }
            }
            BottomTab::ExportQueue => self.draw_export_queue_tab(ui),
            BottomTab::Logs => {
                for message in &self.logs {
                    ui.label(message);
                }
            }
        }
    }

    fn draw_pending_export_progress(&self, ui: &mut egui::Ui) {
        let Some(pending) = &self.pending_export else {
            return;
        };

        ui.label(format!("実行中: {}", pending.summary));
        ui.label(&pending.progress_label);
        ui.small(export_progress_hint(&pending.progress_label));
        ui.add(
            egui::ProgressBar::new(pending.progress_fraction)
                .desired_width(ui.available_width().max(160.0))
                .show_percentage(),
        );
    }

    fn draw_bottom_tab_scroll_region(&self, ui: &mut egui::Ui) {
        let available_size = ui.available_size();
        let content_width =
            clamp_bottom_panel_content_width(self.bottom_panel_content_width).max(available_size.x);

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .max_width(available_size.x)
            .max_height(available_size.y)
            .show(ui, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.set_width(content_width);
                ui.set_min_width(content_width);
                ui.set_min_height(available_size.y);
                self.draw_bottom_tab_contents(ui);
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
                        self.pause_transport();
                    } else {
                        self.play_transport();
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
                        self.seek_transport(pauseink_domain::MediaTime::from_millis(
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
                        self.rediscover_runtime();
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
                let mut template_layout_changed =
                    ui.text_edit_singleline(&mut self.template.text).changed();
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
                    self.maybe_apply_egui_fonts(ui.ctx());
                    template_layout_changed = true;
                }
                template_layout_changed |= ui
                    .add(
                    egui::Slider::new(&mut self.template.settings.font_size, 24.0..=180.0)
                        .text("フォントサイズ"))
                    .changed();
                template_layout_changed |= ui
                    .add(
                    egui::Slider::new(&mut self.template.settings.tracking, 0.0..=48.0)
                        .text("字間"))
                    .changed();
                template_layout_changed |= ui
                    .add(
                    egui::Slider::new(&mut self.template.settings.slope_degrees, -20.0..=20.0)
                        .text("傾き"))
                    .changed();
                if template_layout_changed {
                    self.mark_project_ui_dirty();
                    self.refresh_placed_template_slots(ui.ctx());
                }
                ui.horizontal(|ui| {
                    if ui.button("テンプレート配置").clicked() {
                        self.template.placement_armed = true;
                        self.reset_template_slots();
                    }
                    if ui.button("前スロット").clicked() {
                        self.move_template_slot(-1);
                    }
                    if ui.button("次スロット").clicked() {
                        self.move_template_slot(1);
                    }
                    if ui.button("テンプレート解除").clicked() {
                        self.template.placement_armed = false;
                        self.reset_template_slots();
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
                    let previous_style_preset_id = self.selected_style_preset_id.clone();
                    egui::ComboBox::from_label("組み込み preset")
                        .selected_text(self.selected_style_preset_label())
                        .show_ui(ui, |ui| {
                            for preset in &self.style_presets {
                                ui.selectable_value(
                                    &mut self.selected_style_preset_id,
                                    preset.id.clone(),
                                    &preset.display_name,
                                );
                            }
                        });
                    if self.selected_style_preset_id != previous_style_preset_id {
                        self.sync_preset_editor_fields_from_selection();
                        self.mark_project_ui_dirty();
                    }
                    if ui.button("preset を適用").clicked() {
                        self.apply_selected_style_preset();
                    }
                    ui.horizontal(|ui| {
                        ui.label("user preset ID");
                        ui.text_edit_singleline(&mut self.preset_editor_id);
                    });
                    ui.horizontal(|ui| {
                        ui.label("user preset 名");
                        ui.text_edit_singleline(&mut self.preset_editor_name);
                    });
                    ui.horizontal(|ui| {
                        if ui.button("追加保存").clicked() {
                            self.save_user_style_preset(false);
                        }
                        if ui.button("上書き保存").clicked() {
                            self.save_user_style_preset(true);
                        }
                        let selected_is_user = self
                            .selected_style_preset()
                            .is_some_and(|preset| preset.source == StylePresetSource::User);
                        if ui
                            .add_enabled(selected_is_user, egui::Button::new("削除"))
                            .clicked()
                        {
                            self.delete_selected_user_style_preset();
                        }
                    });
                    ui.separator();
                }
                ui.label("基本スタイル");

                let mut color = [
                    self.session.active_style.color.r,
                    self.session.active_style.color.g,
                    self.session.active_style.color.b,
                ];
                if ui.color_edit_button_srgb(&mut color).changed() {
                    self.session.active_style.color = pauseink_domain::RgbaColor::new(
                        color[0],
                        color[1],
                        color[2],
                        self.session.active_style.color.a,
                    );
                    self.sync_active_style_to_current_object();
                    self.mark_project_ui_dirty();
                }
                if ui
                    .add(
                        egui::Slider::new(&mut self.session.active_style.thickness, 1.0..=32.0)
                            .text("太さ"),
                    )
                    .changed()
                {
                    self.sync_active_style_to_current_object();
                    self.mark_project_ui_dirty();
                }
                if ui
                    .add(
                        egui::Slider::new(&mut self.session.active_style.opacity, 0.05..=1.0)
                            .text("不透明度"),
                    )
                    .changed()
                {
                    self.sync_active_style_to_current_object();
                    self.mark_project_ui_dirty();
                }
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.session.active_style.stabilization_strength,
                            0.0..=1.0,
                        )
                        .text("手ブレ補正"),
                    )
                    .changed()
                {
                    self.sync_active_style_to_current_object();
                    self.mark_project_ui_dirty();
                }
                ui.horizontal(|ui| {
                    ui.label("合成");
                    let mut blend_mode = self.session.active_style.blend_mode;
                    egui::ComboBox::from_id_salt("blend_mode")
                        .selected_text(blend_mode_label(blend_mode))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                pauseink_domain::BlendMode::Normal,
                                pauseink_domain::BlendMode::Multiply,
                                pauseink_domain::BlendMode::Screen,
                                pauseink_domain::BlendMode::Additive,
                            ] {
                                ui.selectable_value(
                                    &mut blend_mode,
                                    candidate,
                                    blend_mode_label(candidate),
                                );
                            }
                        });
                    if blend_mode != self.session.active_style.blend_mode {
                        self.session.active_style.blend_mode = blend_mode;
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                });
                ui.collapsing("アウトライン", |ui| {
                    if ui
                        .checkbox(&mut self.session.active_style.outline.enabled, "有効")
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.session.active_style.outline.width,
                                0.0..=24.0,
                            )
                            .text("幅"),
                        )
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    let mut outline_color = rgba_to_color32(self.session.active_style.outline.color);
                    if ui.color_edit_button_srgba(&mut outline_color).changed() {
                        self.session.active_style.outline.color = color32_to_rgba(outline_color);
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                });
                ui.collapsing("ドロップシャドウ", |ui| {
                    if ui
                        .checkbox(&mut self.session.active_style.drop_shadow.enabled, "有効")
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.session.active_style.drop_shadow.offset_x,
                                -64.0..=64.0,
                            )
                            .text("横オフセット"),
                        )
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.session.active_style.drop_shadow.offset_y,
                                -64.0..=64.0,
                            )
                            .text("縦オフセット"),
                        )
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.session.active_style.drop_shadow.blur_radius,
                                0.0..=48.0,
                            )
                            .text("ぼかし"),
                        )
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    let mut shadow_color =
                        rgba_to_color32(self.session.active_style.drop_shadow.color);
                    if ui.color_edit_button_srgba(&mut shadow_color).changed() {
                        self.session.active_style.drop_shadow.color = color32_to_rgba(shadow_color);
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                });
                ui.collapsing("グロー", |ui| {
                    if ui
                        .checkbox(&mut self.session.active_style.glow.enabled, "有効")
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    if ui
                        .add(
                            egui::Slider::new(
                                &mut self.session.active_style.glow.blur_radius,
                                0.0..=48.0,
                            )
                            .text("ぼかし"),
                        )
                        .changed()
                    {
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                    let mut glow_color = rgba_to_color32(self.session.active_style.glow.color);
                    if ui.color_edit_button_srgba(&mut glow_color).changed() {
                        self.session.active_style.glow.color = color32_to_rgba(glow_color);
                        self.sync_active_style_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                });
                ui.separator();
                ui.label("出現");
                ui.horizontal(|ui| {
                    ui.label("方式");
                    let mut entrance_kind = self.session.active_entrance.kind;
                    egui::ComboBox::from_id_salt("entrance_kind")
                        .selected_text(entrance_kind_label(entrance_kind))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                pauseink_domain::EntranceKind::Instant,
                                pauseink_domain::EntranceKind::PathTrace,
                                pauseink_domain::EntranceKind::Wipe,
                                pauseink_domain::EntranceKind::Dissolve,
                            ] {
                                ui.selectable_value(
                                    &mut entrance_kind,
                                    candidate,
                                    entrance_kind_label(candidate),
                                );
                            }
                        });
                    if entrance_kind != self.session.active_entrance.kind {
                        self.session.active_entrance.kind = entrance_kind;
                        self.sync_active_entrance_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("時間モード");
                    let mut duration_mode = self.session.active_entrance.duration_mode;
                    egui::ComboBox::from_id_salt("entrance_duration_mode")
                        .selected_text(entrance_duration_mode_label(duration_mode))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                pauseink_domain::EntranceDurationMode::FixedTotalDuration,
                                pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength,
                            ] {
                                ui.selectable_value(
                                    &mut duration_mode,
                                    candidate,
                                    entrance_duration_mode_label(candidate),
                                );
                            }
                        });
                    if duration_mode != self.session.active_entrance.duration_mode {
                        self.session.active_entrance.duration_mode = duration_mode;
                        self.sync_active_entrance_to_current_object();
                        self.mark_project_ui_dirty();
                    }
                });
                let mut entrance_duration_ms =
                    media_duration_to_millis(self.session.active_entrance.duration) as f32;
                if ui
                    .add(
                        egui::Slider::new(&mut entrance_duration_ms, 50.0..=5_000.0)
                            .logarithmic(true)
                            .text(match self.session.active_entrance.duration_mode {
                                pauseink_domain::EntranceDurationMode::FixedTotalDuration => {
                                    "出現時間 ms"
                                }
                                pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength => {
                                    "基準時間 ms"
                                }
                            }),
                    )
                    .changed()
                {
                    self.session.active_entrance.duration =
                        pauseink_domain::MediaDuration::from_millis(
                            entrance_duration_ms.round() as i64,
                        );
                    self.sync_active_entrance_to_current_object();
                    self.mark_project_ui_dirty();
                }
                if ui
                    .add(
                        egui::Slider::new(
                            &mut self.session.active_entrance.speed_scalar,
                            0.1..=8.0,
                        )
                        .logarithmic(true)
                        .text("出現速度"),
                    )
                    .changed()
                {
                    self.sync_active_entrance_to_current_object();
                    self.mark_project_ui_dirty();
                }
                ui.small(match self.session.active_entrance.duration_mode {
                    pauseink_domain::EntranceDurationMode::FixedTotalDuration => {
                        "固定時間モードでは、値が短いほど速く出現します。出現速度はこの時間に追加で倍率を掛けます。"
                    }
                    pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength => {
                        "長さ比例モードでは、基準時間を 600px 相当の長さへ当てて、出現速度倍率で最終時間を調整します。"
                    }
                });
                ui.separator();
                ui.label("ガイド");
                if ui
                    .add(
                        egui::Slider::new(&mut self.settings.guide_slope_degrees, -20.0..=20.0)
                            .text("ガイド傾き"),
                    )
                    .changed()
                {
                    self.mark_project_ui_dirty();
                    self.refresh_guide_geometry();
                }
                if ui.button("ガイド解除").clicked() {
                    self.clear_guide_state();
                }
                ui.separator();
                self.draw_export_panel(ui);
            });

        egui::TopBottomPanel::bottom("bottom_tabs")
            .default_height(180.0)
            .min_height(120.0)
            .max_height((ctx.content_rect().height() * 0.45).max(120.0))
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
                    ui.separator();
                    ui.label("内容幅");
                    ui.add(
                        egui::DragValue::new(&mut self.bottom_panel_content_width)
                            .range(320.0..=8_192.0)
                            .speed(16.0),
                    );
                });
                ui.separator();

                self.draw_bottom_tab_scroll_region(ui);
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

            self.handle_canvas_input(&response, frame_rect, frame_width, frame_height, &ctx);

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
            self.draw_template_preview(&ctx, &painter, frame_rect, &response);
            if let Some(texture) = &self.overlay_texture {
                painter.image(
                    texture.id(),
                    frame_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }

            self.draw_live_stroke_preview(&painter, frame_rect, frame_width, frame_height);
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

fn ffmpeg_runtime_help_heading(os: &str) -> &'static str {
    match os {
        "windows" => "Windows でランタイムが見つからないとき",
        "macos" => "macOS でランタイムが見つからないとき",
        _ => "Linux でランタイムが見つからないとき",
    }
}

fn ffmpeg_runtime_help(runtime_root: &Path, os: &str, platform_id: &str) -> Vec<String> {
    let sidecar_dir = sidecar_runtime_dir(runtime_root, platform_id);
    match os {
        "windows" => vec![
            format!(
                "1. 推奨配置: `{}` に `ffmpeg.exe` / `ffprobe.exe` / `manifest.json` を置きます。",
                sidecar_dir.display()
            ),
            "2. `winget install --id=Gyan.FFmpeg -e` 後に未検出のままなら、`%LOCALAPPDATA%\\Microsoft\\WinGet\\Links` または `%LOCALAPPDATA%\\Microsoft\\WinGet\\Packages\\...\\bin` に `ffmpeg.exe` / `ffprobe.exe` があるか確認します。".to_owned(),
            "3. sidecar を置けない場合は、`ffmpeg.exe` と `ffprobe.exe` の両方を `PATH` へ通してください。Scoop の場合は `~/scoop/shims` も探索対象です。".to_owned(),
            "4. 配置後は `機能情報更新` か `診断を再取得` を押すと、その場で再検出します。".to_owned(),
        ],
        "macos" => vec![
            format!(
                "1. 推奨配置: `{}` に `ffmpeg` / `ffprobe` / `manifest.json` を置きます。",
                sidecar_dir.display()
            ),
            "2. host runtime を使う場合、Homebrew は `/opt/homebrew/bin` または `/usr/local/bin`、MacPorts は `/opt/local/bin` を確認します。".to_owned(),
            "3. 配置後は `機能情報更新` か `診断を再取得` を押すと、その場で再検出します。".to_owned(),
        ],
        _ => vec![
            format!(
                "1. 推奨配置: `{}` に `ffmpeg` / `ffprobe` / `manifest.json` を置きます。",
                sidecar_dir.display()
            ),
            "2. host runtime を使う場合、`/usr/bin`、`/usr/local/bin`、`/snap/bin`、`~/.local/bin`、Linuxbrew 系を順に確認します。".to_owned(),
            "3. 配置後は `機能情報更新` か `診断を再取得` を押すと、その場で再検出します。".to_owned(),
        ],
    }
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

fn repository_style_preset_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../presets/style_presets")
}

fn load_style_presets(
    portable_paths: &PortablePaths,
) -> std::result::Result<Vec<BaseStylePreset>, String> {
    load_base_style_presets_overlay(
        &repository_style_preset_dir(),
        Some(&portable_paths.user_style_presets_dir()),
    )
    .map_err(|error| error.to_string())
}

fn preset_editor_fields_from_selection(
    style_presets: &[BaseStylePreset],
    selected_style_preset_id: &str,
) -> (String, String) {
    style_presets
        .iter()
        .find(|preset| preset.id == selected_style_preset_id)
        .map(|preset| (preset.id.clone(), preset.display_name.clone()))
        .unwrap_or_else(|| {
            (
                selected_style_preset_id.to_owned(),
                if selected_style_preset_id.is_empty() {
                    String::new()
                } else {
                    selected_style_preset_id.to_owned()
                },
            )
        })
}

fn sanitize_style_preset_id(raw: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_was_separator = false;

    for ch in raw.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if matches!(ch, '_' | '-' | ' ' | '　') {
            Some('_')
        } else {
            None
        };

        let Some(mapped) = mapped else {
            continue;
        };
        if mapped == '_' {
            if previous_was_separator || sanitized.is_empty() {
                continue;
            }
            previous_was_separator = true;
            sanitized.push(mapped);
        } else {
            previous_was_separator = false;
            sanitized.push(mapped);
        }
    }

    sanitized.trim_matches('_').to_owned()
}

fn clamp_bottom_panel_content_width(width: f32) -> f32 {
    width.clamp(320.0, 8_192.0)
}

fn ensure_object_value(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .expect("value was converted into an object above")
}

fn rgba_to_color32(color: pauseink_domain::RgbaColor) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

fn color32_to_rgba(color: Color32) -> pauseink_domain::RgbaColor {
    pauseink_domain::RgbaColor::new(color.r(), color.g(), color.b(), color.a())
}

fn media_duration_to_millis(duration: pauseink_domain::MediaDuration) -> i64 {
    ((duration.ticks as f64 * duration.time_base.numerator as f64 * 1000.0)
        / duration.time_base.denominator as f64)
        .round() as i64
}

fn blend_mode_label(mode: pauseink_domain::BlendMode) -> &'static str {
    match mode {
        pauseink_domain::BlendMode::Normal => "通常",
        pauseink_domain::BlendMode::Multiply => "乗算",
        pauseink_domain::BlendMode::Screen => "スクリーン",
        pauseink_domain::BlendMode::Additive => "加算",
    }
}

fn entrance_kind_label(kind: pauseink_domain::EntranceKind) -> &'static str {
    match kind {
        pauseink_domain::EntranceKind::Instant => "即時",
        pauseink_domain::EntranceKind::PathTrace => "なぞり書き",
        pauseink_domain::EntranceKind::Wipe => "ワイプ",
        pauseink_domain::EntranceKind::Dissolve => "ディゾルブ",
    }
}

fn entrance_duration_mode_label(mode: pauseink_domain::EntranceDurationMode) -> &'static str {
    match mode {
        pauseink_domain::EntranceDurationMode::FixedTotalDuration => "固定時間",
        pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength => "長さ比例",
    }
}

fn underlay_mode_key(mode: UnderlayMode) -> &'static str {
    match mode {
        UnderlayMode::Outline => "outline",
        UnderlayMode::FaintFill => "faint_fill",
        UnderlayMode::SlotBoxOnly => "slot_box_only",
        UnderlayMode::OutlineAndSlotBox => "outline_and_slot_box",
    }
}

fn parse_underlay_mode(raw: &str) -> UnderlayMode {
    match raw {
        "faint_fill" => UnderlayMode::FaintFill,
        "slot_box_only" => UnderlayMode::SlotBoxOnly,
        "outline_and_slot_box" => UnderlayMode::OutlineAndSlotBox,
        _ => UnderlayMode::Outline,
    }
}

fn current_frame_primary_press_position(ctx: &egui::Context) -> Option<Pos2> {
    ctx.input(|input| {
        input.events.iter().rev().find_map(|event| match event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => Some(*pos),
            _ => None,
        })
    })
}

fn extend_horizontal_guide_line_to_frame_width(
    line: &pauseink_template_layout::GuideLine,
    frame_width: u32,
) -> (
    pauseink_template_layout::Point,
    pauseink_template_layout::Point,
) {
    let frame_right = frame_width as f32;
    let dx = line.end.x - line.start.x;
    if dx.abs() <= f32::EPSILON {
        return (line.start, line.end);
    }

    let slope = (line.end.y - line.start.y) / dx;
    let start =
        pauseink_template_layout::Point::new(0.0, line.start.y + (0.0 - line.start.x) * slope);
    let end = pauseink_template_layout::Point::new(
        frame_right,
        line.start.y + (frame_right - line.start.x) * slope,
    );
    (start, end)
}

fn live_preview_stroke_width(
    thickness: f32,
    frame_rect: Rect,
    frame_width: u32,
    frame_height: u32,
) -> f32 {
    let scale_x = frame_rect.width() / frame_width.max(1) as f32;
    let scale_y = frame_rect.height() / frame_height.max(1) as f32;
    (thickness * scale_x.min(scale_y)).max(1.0)
}

fn live_preview_dot_radius(stroke_width: f32) -> f32 {
    (stroke_width * 0.5).max(1.0)
}

fn draft_preview_color(style: &pauseink_domain::StyleSnapshot) -> Color32 {
    let alpha = ((style.color.a as f32) * style.opacity.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(style.color.r, style.color.g, style.color.b, alpha)
}

fn step_template_slot_index(current: usize, slot_len: usize, delta: isize) -> usize {
    if slot_len == 0 {
        return 0;
    }

    if delta.is_negative() {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current
            .saturating_add(delta as usize)
            .min(slot_len.saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Event, Modifiers, PointerButton, Pos2, RawInput};
    use pauseink_media::{ImportedMedia, MediaProbe, MediaSupport, PlaybackState};
    use pauseink_portable_fs::{load_settings_from_file, PortablePaths, Settings};
    use tempfile::tempdir;

    fn initialized_test_context() -> egui::Context {
        let ctx = egui::Context::default();
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0))),
                ..Default::default()
            },
            |_ctx| {},
        );
        ctx
    }

    fn sample_imported_media() -> ImportedMedia {
        ImportedMedia {
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
        }
    }

    fn run_canvas_input_frame(
        app: &mut DesktopApp,
        ctx: &egui::Context,
        events: Vec<Event>,
    ) -> Rect {
        let mut frame_rect = Rect::NOTHING;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let (response, _painter) =
                        ui.allocate_painter(Vec2::new(640.0, 360.0), Sense::click_and_drag());
                    frame_rect = response.rect;
                    app.handle_canvas_input(&response, response.rect, 1280, 720, ctx);
                });
            },
        );
        frame_rect
    }

    fn run_shortcut_frame(
        app: &mut DesktopApp,
        ctx: &egui::Context,
        modifiers: Modifiers,
        events: Vec<Event>,
    ) {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0))),
                modifiers,
                events,
                ..Default::default()
            },
            |ctx| {
                app.handle_global_shortcuts(ctx);
                app.handle_guide_modifier_tap(ctx);
            },
        );
    }

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
    fn save_and_reopen_project_restores_style_template_and_guide_state() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let path = temp_dir.path().join("stateful-project.pauseink");

        let mut app = DesktopApp::new(portable_paths.clone(), Settings::default(), None, None);
        app.selected_style_preset_id = "marker_highlight".to_owned();
        app.session.active_style.color = pauseink_domain::RgbaColor::new(240, 32, 64, 255);
        app.session.active_style.thickness = 13.0;
        app.session.active_style.opacity = 0.42;
        app.session.active_style.outline.enabled = true;
        app.session.active_style.outline.width = 4.0;
        app.session.active_style.blend_mode = pauseink_domain::BlendMode::Additive;
        app.session.active_style.stabilization_strength = 0.77;
        app.session.active_entrance.kind = pauseink_domain::EntranceKind::PathTrace;
        app.session.active_entrance.duration_mode =
            pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength;
        app.session.active_entrance.duration = pauseink_domain::MediaDuration::from_millis(900);
        app.session.active_entrance.speed_scalar = 2.2;
        app.template.text = "保存対象".to_owned();
        app.template.font_family = "BIZ UDPGothic".to_owned();
        app.template.settings.font_size = 128.0;
        app.template.settings.tracking = 12.0;
        app.template.settings.slope_degrees = 9.5;
        app.template.settings.underlay_mode = UnderlayMode::FaintFill;
        app.settings.guide_slope_degrees = -6.0;

        app.save_project(path.clone());

        let mut reopened = DesktopApp::new(portable_paths, Settings::default(), None, None);
        reopened.open_project(path);

        assert_eq!(reopened.selected_style_preset_id, "marker_highlight");
        assert_eq!(reopened.session.active_style.thickness, 13.0);
        assert!((reopened.session.active_style.opacity - 0.42).abs() < 0.001);
        assert!((reopened.session.active_style.stabilization_strength - 0.77).abs() < 0.001);
        assert!(reopened.session.active_style.outline.enabled);
        assert_eq!(
            reopened.session.active_style.blend_mode,
            pauseink_domain::BlendMode::Additive
        );
        assert_eq!(
            reopened.session.active_style.color,
            pauseink_domain::RgbaColor::new(240, 32, 64, 255)
        );
        assert_eq!(
            reopened.session.active_entrance.kind,
            pauseink_domain::EntranceKind::PathTrace
        );
        assert_eq!(
            reopened.session.active_entrance.duration_mode,
            pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength
        );
        assert_eq!(
            media_duration_to_millis(reopened.session.active_entrance.duration),
            900
        );
        assert!((reopened.session.active_entrance.speed_scalar - 2.2).abs() < 0.001);
        assert_eq!(reopened.template.text, "保存対象");
        assert_eq!(reopened.template.font_family, "BIZ UDPGothic");
        assert!((reopened.template.settings.font_size - 128.0).abs() < 0.01);
        assert!((reopened.template.settings.tracking - 12.0).abs() < 0.01);
        assert!((reopened.template.settings.slope_degrees - 9.5).abs() < 0.01);
        assert_eq!(
            reopened.template.settings.underlay_mode,
            UnderlayMode::FaintFill
        );
        assert!((reopened.settings.guide_slope_degrees - -6.0).abs() < 0.01);
    }

    #[test]
    fn save_and_relaunch_restores_style_template_and_effect_state_from_settings_file() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");

        let mut app = DesktopApp::new(portable_paths.clone(), Settings::default(), None, None);
        app.selected_style_preset_id = "marker_highlight".to_owned();
        app.session.active_style.color = pauseink_domain::RgbaColor::new(240, 32, 64, 255);
        app.session.active_style.thickness = 13.0;
        app.session.active_style.opacity = 0.42;
        app.session.active_style.outline.enabled = true;
        app.session.active_style.outline.width = 4.0;
        app.session.active_style.drop_shadow.enabled = true;
        app.session.active_style.drop_shadow.offset_x = 5.0;
        app.session.active_style.glow.enabled = true;
        app.session.active_style.glow.blur_radius = 12.0;
        app.session.active_style.blend_mode = pauseink_domain::BlendMode::Additive;
        app.session.active_style.stabilization_strength = 0.77;
        app.session.active_entrance.kind = pauseink_domain::EntranceKind::PathTrace;
        app.session.active_entrance.duration_mode =
            pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength;
        app.session.active_entrance.duration = pauseink_domain::MediaDuration::from_millis(900);
        app.session.active_entrance.speed_scalar = 2.2;
        app.template.text = "次回起動復元".to_owned();
        app.template.font_family = "BIZ UDPGothic".to_owned();
        app.template.settings.font_size = 128.0;
        app.template.settings.tracking = 12.0;
        app.template.settings.slope_degrees = 9.5;
        app.template.settings.underlay_mode = UnderlayMode::FaintFill;
        app.settings.guide_slope_degrees = -6.0;

        app.save_settings();

        let reopened_settings =
            pauseink_portable_fs::load_settings_or_default(&portable_paths).expect("settings load");
        let reopened = DesktopApp::new(portable_paths, reopened_settings, None, None);

        assert_eq!(reopened.selected_style_preset_id, "marker_highlight");
        assert_eq!(reopened.session.active_style.thickness, 13.0);
        assert!((reopened.session.active_style.opacity - 0.42).abs() < 0.001);
        assert!((reopened.session.active_style.stabilization_strength - 0.77).abs() < 0.001);
        assert!(reopened.session.active_style.outline.enabled);
        assert!(reopened.session.active_style.drop_shadow.enabled);
        assert!(reopened.session.active_style.glow.enabled);
        assert_eq!(
            reopened.session.active_style.blend_mode,
            pauseink_domain::BlendMode::Additive
        );
        assert_eq!(
            reopened.session.active_style.color,
            pauseink_domain::RgbaColor::new(240, 32, 64, 255)
        );
        assert_eq!(
            reopened.session.active_entrance.kind,
            pauseink_domain::EntranceKind::PathTrace
        );
        assert_eq!(
            reopened.session.active_entrance.duration_mode,
            pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength
        );
        assert_eq!(
            media_duration_to_millis(reopened.session.active_entrance.duration),
            900
        );
        assert!((reopened.session.active_entrance.speed_scalar - 2.2).abs() < 0.001);
        assert_eq!(reopened.template.text, "次回起動復元");
        assert_eq!(reopened.template.font_family, "BIZ UDPGothic");
        assert!((reopened.template.settings.font_size - 128.0).abs() < 0.01);
        assert!((reopened.template.settings.tracking - 12.0).abs() < 0.01);
        assert!((reopened.template.settings.slope_degrees - 9.5).abs() < 0.01);
        assert_eq!(
            reopened.template.settings.underlay_mode,
            UnderlayMode::FaintFill
        );
        assert!((reopened.settings.guide_slope_degrees - -6.0).abs() < 0.01);
    }

    #[test]
    fn desktop_app_loads_user_style_presets_from_portable_root_and_overrides_builtin_ids() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let user_preset_dir = portable_paths.config_dir.join("style_presets");
        std::fs::create_dir_all(&user_preset_dir).expect("user preset dir");
        std::fs::write(
            user_preset_dir.join("marker_highlight.json5"),
            r#"
            {
              id: "marker_highlight",
              display_name: "ユーザー上書きマーカー",
              base_style: {
                thickness: 22.0,
                opacity: 0.35,
                color_rgba: [0.2, 0.8, 1.0, 0.35],
              },
            }
            "#,
        )
        .expect("user preset file");

        let app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let preset = app
            .style_presets
            .iter()
            .find(|preset| preset.id == "marker_highlight")
            .expect("marker_highlight should exist");

        assert_eq!(preset.display_name, "ユーザー上書きマーカー");
        assert_eq!(preset.thickness, Some(22.0));
        assert_eq!(preset.opacity, Some(0.35));
    }

    #[test]
    fn save_settings_and_restart_restore_workspace_style_and_template_state() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");

        let mut app = DesktopApp::new(portable_paths.clone(), Settings::default(), None, None);
        app.selected_style_preset_id = "marker_highlight".to_owned();
        app.session.active_style.color = pauseink_domain::RgbaColor::new(250, 180, 32, 255);
        app.session.active_style.thickness = 15.0;
        app.session.active_style.opacity = 0.38;
        app.session.active_style.outline.enabled = true;
        app.session.active_style.outline.width = 5.0;
        app.session.active_style.drop_shadow.enabled = true;
        app.session.active_style.drop_shadow.offset_x = 6.0;
        app.session.active_style.drop_shadow.offset_y = 4.0;
        app.session.active_style.drop_shadow.blur_radius = 7.0;
        app.session.active_style.glow.enabled = true;
        app.session.active_style.glow.blur_radius = 9.0;
        app.session.active_entrance.kind = pauseink_domain::EntranceKind::PathTrace;
        app.session.active_entrance.speed_scalar = 1.8;
        app.template.font_family = "BIZ UDPGothic".to_owned();
        app.template.text = "再起動復元".to_owned();
        app.template.settings.font_size = 132.0;
        app.template.settings.tracking = 10.0;
        app.template.settings.slope_degrees = 8.0;
        app.settings.guide_slope_degrees = -4.5;

        app.save_settings();

        let loaded = load_settings_from_file(&portable_paths).expect("settings should load");
        let reopened = DesktopApp::new(portable_paths, loaded, None, None);

        assert_eq!(reopened.selected_style_preset_id, "marker_highlight");
        assert_eq!(reopened.session.active_style.thickness, 15.0);
        assert!((reopened.session.active_style.opacity - 0.38).abs() < 0.001);
        assert!(reopened.session.active_style.outline.enabled);
        assert!((reopened.session.active_style.outline.width - 5.0).abs() < 0.001);
        assert!(reopened.session.active_style.drop_shadow.enabled);
        assert!((reopened.session.active_style.drop_shadow.offset_x - 6.0).abs() < 0.001);
        assert!(reopened.session.active_style.glow.enabled);
        assert_eq!(
            reopened.session.active_entrance.kind,
            pauseink_domain::EntranceKind::PathTrace
        );
        assert!((reopened.session.active_entrance.speed_scalar - 1.8).abs() < 0.001);
        assert_eq!(reopened.template.font_family, "BIZ UDPGothic");
        assert_eq!(reopened.template.text, "再起動復元");
        assert!((reopened.template.settings.font_size - 132.0).abs() < 0.001);
        assert!((reopened.template.settings.slope_degrees - 8.0).abs() < 0.001);
        assert!((reopened.settings.guide_slope_degrees - -4.5).abs() < 0.001);
    }

    #[test]
    fn user_style_preset_save_overwrite_and_delete_roundtrip_updates_catalog() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths.clone(), Settings::default(), None, None);
        let preset_path = portable_paths
            .user_style_presets_dir()
            .join("custom_soft_marker.json5");

        app.preset_editor_id = "custom_soft_marker".to_owned();
        app.preset_editor_name = "Custom Soft Marker".to_owned();
        app.session.active_style.thickness = 9.0;
        app.session.active_style.opacity = 0.4;
        app.save_user_style_preset(false);

        assert!(preset_path.exists());
        let saved = app
            .style_presets
            .iter()
            .find(|preset| preset.id == "custom_soft_marker")
            .expect("saved preset should appear in catalog");
        assert_eq!(saved.source, StylePresetSource::User);
        assert_eq!(saved.thickness, Some(9.0));
        assert_eq!(saved.opacity, Some(0.4));

        app.selected_style_preset_id = "custom_soft_marker".to_owned();
        app.session.active_style.thickness = 14.0;
        app.session.active_style.opacity = 0.2;
        app.save_user_style_preset(true);

        let overwritten = load_base_style_presets_overlay(
            &repository_style_preset_dir(),
            Some(&portable_paths.user_style_presets_dir()),
        )
        .expect("overlay preset load");
        let overwritten = overwritten
            .iter()
            .find(|preset| preset.id == "custom_soft_marker")
            .expect("overwritten preset should remain");
        assert_eq!(overwritten.thickness, Some(14.0));
        assert_eq!(overwritten.opacity, Some(0.2));

        app.delete_selected_user_style_preset();
        assert!(!preset_path.exists());
        assert!(app
            .style_presets
            .iter()
            .all(|preset| preset.id != "custom_soft_marker"));
    }

    #[test]
    fn style_preset_application_updates_effect_fields_and_persists_entrance_state() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let project_path = temp_dir.path().join("effect-entrance.pauseink");
        std::fs::write(
            portable_paths
                .user_style_presets_dir()
                .join("effect_trace.json5"),
            r#"
            {
              id: "effect_trace",
              display_name: "Effect Trace",
              base_style: {
                thickness: 10.0,
                color_rgba: [0.2, 0.9, 1.0, 0.5],
                opacity: 0.5,
                blend_mode: "additive",
                outline: {
                  enabled: true,
                  width: 3.0,
                  color_rgba: [0.0, 0.0, 0.0, 1.0],
                },
                drop_shadow: {
                  enabled: true,
                  offset_x: 4.0,
                  offset_y: 6.0,
                  blur_radius: 8.0,
                  color_rgba: [0.1, 0.1, 0.1, 0.8],
                },
                glow: {
                  enabled: true,
                  blur_radius: 12.0,
                  color_rgba: [0.9, 1.0, 1.0, 0.7],
                },
              },
              entrance: {
                kind: "path_trace",
                duration_mode: "length_proportional",
                speed_scalar: 2.5,
              },
            }
            "#,
        )
        .expect("user preset file");

        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        app.selected_style_preset_id = "effect_trace".to_owned();

        app.apply_selected_style_preset();
        app.save_project(project_path.clone());

        assert!(app.session.active_style.outline.enabled);
        assert!(app.session.active_style.drop_shadow.enabled);
        assert!(app.session.active_style.glow.enabled);
        assert_eq!(
            app.session.active_style.blend_mode,
            pauseink_domain::BlendMode::Additive
        );
        assert_eq!(
            app.session.active_entrance.kind,
            pauseink_domain::EntranceKind::PathTrace
        );
        assert_eq!(
            app.session.active_entrance.duration_mode,
            pauseink_domain::EntranceDurationMode::ProportionalToStrokeLength
        );
        assert!((app.session.active_entrance.speed_scalar - 2.5).abs() < 0.001);

        let serialized = std::fs::read_to_string(project_path).expect("saved project");
        assert!(
            serialized.contains("\"entrance\""),
            "project へ active entrance resolved snapshot を保存したい"
        );
        assert!(
            serialized.contains("\"path_trace\""),
            "preset 適用後の entrance kind を保存したい"
        );
    }

    #[test]
    fn desktop_app_can_add_overwrite_and_delete_user_style_presets() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let preset_path = portable_paths
            .user_style_presets_dir()
            .join("custom_marker.json5");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);

        app.preset_editor_id = "custom_marker".to_owned();
        app.preset_editor_name = "Custom Marker".to_owned();
        app.session.active_style.thickness = 11.0;
        app.session.active_style.opacity = 0.28;
        app.session.active_style.stabilization_strength = 0.66;
        app.save_user_style_preset(false);

        assert!(preset_path.exists(), "追加保存で user preset file が必要");
        let saved = app
            .style_presets
            .iter()
            .find(|preset| preset.id == "custom_marker")
            .expect("saved preset");
        assert_eq!(saved.source, StylePresetSource::User);
        assert_eq!(saved.thickness, Some(11.0));
        assert_eq!(saved.opacity, Some(0.28));

        app.selected_style_preset_id = "custom_marker".to_owned();
        app.session.active_style.thickness = 19.0;
        app.session.active_style.opacity = 0.51;
        app.save_user_style_preset(true);

        let overwritten = app
            .style_presets
            .iter()
            .find(|preset| preset.id == "custom_marker")
            .expect("overwritten preset");
        assert_eq!(overwritten.thickness, Some(19.0));
        assert_eq!(overwritten.opacity, Some(0.51));

        app.delete_selected_user_style_preset();
        assert!(!preset_path.exists(), "削除で file も消したい");
        assert!(
            !app.style_presets
                .iter()
                .any(|preset| preset.id == "custom_marker"
                    && preset.source == StylePresetSource::User)
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
    fn guide_overlay_state_keeps_vertical_width_constant_and_anchors_to_previous_right_edge() {
        let mut state = GuideOverlayState::from_reference_bounds(
            pauseink_domain::Point2 { x: 100.0, y: 200.0 },
            pauseink_domain::Point2 { x: 130.0, y: 280.0 },
        );
        let original_width = state.cell_width;

        assert!((original_width - 40.0).abs() < 0.01);
        assert!((state.next_cell_origin_x - 130.0).abs() < 0.01);

        state.advance_to_next_from_bounds(Some((
            pauseink_domain::Point2 { x: 180.0, y: 210.0 },
            pauseink_domain::Point2 { x: 245.0, y: 282.0 },
        )));

        let geometry = state.build_geometry(0.0);
        let main_verticals = geometry
            .vertical_lines
            .iter()
            .filter(|line| line.kind == GuideLineKind::Main)
            .map(|line| line.start.x)
            .collect::<Vec<_>>();

        assert!((state.cell_width - original_width).abs() < 0.01);
        assert!((state.next_cell_origin_x - 245.0).abs() < 0.01);
        assert!((main_verticals[0] - 245.0).abs() < 0.01);
        assert!((main_verticals[2] - (245.0 + original_width)).abs() < 0.01);
    }

    #[test]
    fn guide_overlay_state_keeps_vertical_set_width_constant_when_advancing_from_bounds() {
        let mut state = GuideOverlayState::from_reference_bounds(
            pauseink_domain::Point2 { x: 100.0, y: 200.0 },
            pauseink_domain::Point2 { x: 160.0, y: 280.0 },
        );
        let original_width = state.cell_width;

        state.advance_to_next_from_bounds(Some((
            pauseink_domain::Point2 { x: 180.0, y: 210.0 },
            pauseink_domain::Point2 { x: 250.0, y: 282.0 },
        )));

        let geometry = state.build_geometry(0.0);
        let main_verticals = geometry
            .vertical_lines
            .iter()
            .filter(|line| line.kind == GuideLineKind::Main)
            .collect::<Vec<_>>();

        assert!((state.cell_width - original_width).abs() < 0.01);
        assert!((main_verticals[0].start.x - 250.0).abs() < 0.01);
        assert!((main_verticals[2].start.x - 310.0).abs() < 0.01);
    }

    #[test]
    fn guide_modifier_tap_does_not_advance_after_ctrl_z_shortcut() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        app.guide_state = Some(GuideOverlayState::from_reference_bounds(
            pauseink_domain::Point2 { x: 100.0, y: 200.0 },
            pauseink_domain::Point2 { x: 160.0, y: 280.0 },
        ));
        app.refresh_guide_geometry();
        let original_x = app
            .guide_geometry
            .as_ref()
            .expect("guide geometry")
            .vertical_lines
            .iter()
            .find(|line| line.kind == GuideLineKind::Main)
            .expect("main vertical")
            .start
            .x;

        let ctx = initialized_test_context();
        let mut command_modifiers = Modifiers::default();
        command_modifiers.ctrl = true;
        command_modifiers.command = true;
        run_shortcut_frame(
            &mut app,
            &ctx,
            command_modifiers,
            vec![Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed: true,
                repeat: false,
                modifiers: command_modifiers,
            }],
        );
        run_shortcut_frame(&mut app, &ctx, Modifiers::default(), Vec::new());

        let advanced_x = app
            .guide_geometry
            .as_ref()
            .expect("guide geometry")
            .vertical_lines
            .iter()
            .find(|line| line.kind == GuideLineKind::Main)
            .expect("main vertical")
            .start
            .x;

        assert!((advanced_x - original_x).abs() < 0.01);
    }

    #[test]
    fn windows_runtime_help_mentions_winget_and_sidecar_layout() {
        let lines = ffmpeg_runtime_help(
            Path::new("/tmp/pauseink_data/runtime"),
            "windows",
            "windows-x86_64",
        );

        assert!(lines.iter().any(|line| line.contains("windows-x86_64")));
        assert!(lines.iter().any(|line| line.contains("WinGet")));
        assert!(lines.iter().any(|line| line.contains("ffmpeg.exe")));
        assert!(lines.iter().any(|line| line.contains("PATH")));
    }

    #[test]
    fn apply_runtime_discovery_updates_status_and_provider() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(
            portable_paths,
            Settings::default(),
            None,
            Some("not found".to_owned()),
        );
        let runtime = MediaRuntime {
            ffmpeg_path: PathBuf::from("/tmp/custom-ffmpeg"),
            ffprobe_path: PathBuf::from("/tmp/custom-ffprobe"),
            origin: pauseink_media::RuntimeOrigin::SystemHost,
            manifest_path: None,
            build_summary: Some("custom runtime".to_owned()),
            license_summary: None,
        };

        app.apply_runtime_discovery(Some(runtime.clone()), None);

        assert!(app.runtime.is_some());
        assert!(app.provider.is_some());
        assert!(app.runtime_status.contains("custom runtime"));
        assert!(app.last_runtime_error.is_none());

        app.apply_runtime_discovery(None, Some("still missing".to_owned()));
        assert!(app.runtime.is_none());
        assert!(app.provider.is_none());
        assert_eq!(app.runtime_status, "ランタイム: 未検出");
        assert_eq!(app.last_runtime_error.as_deref(), Some("still missing"));
    }

    #[test]
    fn placed_template_slots_reflow_when_font_size_changes_after_placement() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let ctx = initialized_test_context();
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let origin = Point::new(24.0, 36.0);
        app.template.text = "AB".to_owned();
        app.template.placed_origin = Some(origin);
        app.refresh_placed_template_slots(&ctx);

        let before = app
            .template
            .placed_slots
            .clone()
            .expect("slots should exist after placement");

        app.template.settings.font_size = 160.0;
        app.refresh_placed_template_slots(&ctx);

        let after = app
            .template
            .placed_slots
            .clone()
            .expect("slots should still exist");

        assert!(after[0].height > before[0].height);
        assert!(after[1].origin.x > before[1].origin.x);
    }

    #[test]
    fn stroke_starts_on_pointer_press_before_drag_threshold() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let ctx = initialized_test_context();
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let press = Pos2::new(120.0, 120.0);

        let _press_snapshot = run_canvas_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(press),
                Event::PointerButton {
                    pos: press,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );

        assert!(
            app.canvas_drag_active,
            "pointer press frame で stroke draft を開始したい"
        );
        let preview = app
            .session
            .current_stroke_preview()
            .expect("drag threshold 未満でも最初の点は見えている必要がある");
        assert_eq!(
            preview.points.len(),
            1,
            "press frame は同一点の二重 sample ではなく 1 点 preview にしたい"
        );
    }

    #[test]
    fn playback_running_disables_canvas_stroke_capture() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let ctx = initialized_test_context();
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        app.session.playback = Some(PlaybackState::new(sample_imported_media()));
        assert!(app.session.play());

        run_canvas_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(Pos2::new(120.0, 120.0)),
                Event::PointerButton {
                    pos: Pos2::new(120.0, 120.0),
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );

        assert!(!app.canvas_drag_active, "再生中は stroke を開始しない");
        assert!(
            app.session.current_stroke_preview().is_none(),
            "再生中に preview draft を作ってはいけない"
        );
    }

    #[test]
    fn preview_force_visible_batch_tracks_written_batch_not_current_transport_time() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        app.session.playback = Some(PlaybackState::new(sample_imported_media()));

        app.set_preview_force_visible_batch(pauseink_domain::MediaTime::from_millis(1_000));
        app.session
            .seek(pauseink_domain::MediaTime::from_millis(1_120));

        assert_eq!(
            app.current_preview_force_visible_batch(),
            Some(pauseink_domain::MediaTime::from_millis(1_000)),
            "paused preview は current transport time ではなく、いま fully visible 扱いにした batch anchor を保持すべき"
        );

        app.play_transport();
        assert_eq!(
            app.current_preview_force_visible_batch(),
            None,
            "play 開始時には preview-only batch override を timeline へ戻すべき"
        );
    }

    #[test]
    fn committed_stroke_keeps_press_origin_as_first_raw_sample() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let ctx = initialized_test_context();
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let press = Pos2::new(120.0, 120.0);
        let drag = Pos2::new(152.0, 132.0);
        let release = Pos2::new(220.0, 160.0);

        let press_snapshot = run_canvas_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(press),
                Event::PointerButton {
                    pos: press,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
        let expected_press_origin =
            pointer_position_to_frame_point(press, press_snapshot, 1280, 720)
                .expect("press should be inside frame rect");
        run_canvas_input_frame(&mut app, &ctx, vec![Event::PointerMoved(drag)]);
        assert!(
            app.session
                .current_stroke_preview()
                .is_some_and(|preview| preview.points.len() >= 2),
            "drag frame では 2 点目以降の sample が draft に入る必要がある"
        );
        run_canvas_input_frame(&mut app, &ctx, vec![Event::PointerMoved(release)]);
        run_canvas_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(release),
                Event::PointerButton {
                    pos: release,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: Modifiers::NONE,
                },
            ],
        );

        let stroke = app
            .session
            .project
            .strokes
            .first()
            .expect("stroke should be committed");
        let first = stroke.raw_samples.first().expect("first sample");

        assert!(
            (first.position.x - expected_press_origin.x).abs() < 0.01,
            "最初の x sample は press origin を保つべき"
        );
        assert!(
            (first.position.y - expected_press_origin.y).abs() < 0.01,
            "最初の y sample は press origin を保つべき"
        );
    }

    #[test]
    fn canvas_input_is_ignored_while_playback_is_running() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let ctx = initialized_test_context();
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        app.session.playback = Some(pauseink_media::PlaybackState {
            media: pauseink_media::ImportedMedia {
                source_path: PathBuf::from("/tmp/example.mp4"),
                probe: pauseink_media::MediaProbe {
                    format_name: Some("mp4".to_owned()),
                    duration_seconds: Some(10.0),
                    duration_raw: Some("10".to_owned()),
                    width: Some(1280),
                    height: Some(720),
                    frame_rate: Some(30.0),
                    avg_frame_rate_raw: Some("30/1".to_owned()),
                    r_frame_rate_raw: Some("30/1".to_owned()),
                    pix_fmt: Some("yuv420p".to_owned()),
                    has_alpha: false,
                    has_audio: true,
                    video_codec: Some("h264".to_owned()),
                    audio_codec: Some("aac".to_owned()),
                    support: pauseink_media::MediaSupport::Supported,
                },
            },
            current_time: pauseink_domain::MediaTime::from_millis(250),
            is_playing: true,
        });

        let press = Pos2::new(120.0, 120.0);
        run_canvas_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(press),
                Event::PointerButton {
                    pos: press,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
        );

        assert!(!app.canvas_drag_active);
        assert!(app.session.current_stroke_preview().is_none());
        assert!(app.session.project.strokes.is_empty());
    }

    #[test]
    fn same_frame_move_keeps_pointer_button_press_as_first_preview_point() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let ctx = initialized_test_context();
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let press = Pos2::new(120.0, 120.0);
        let moved = Pos2::new(152.0, 132.0);

        let frame_rect = run_canvas_input_frame(
            &mut app,
            &ctx,
            vec![
                Event::PointerMoved(press),
                Event::PointerButton {
                    pos: press,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
                Event::PointerMoved(moved),
            ],
        );

        let preview = app
            .session
            .current_stroke_preview()
            .expect("press frame の preview が必要");
        let expected_press_origin = pointer_position_to_frame_point(press, frame_rect, 1280, 720)
            .expect("press should map into frame");

        assert!(
            (preview.points[0].x - expected_press_origin.x).abs() < 0.01,
            "同一 frame 内に move が来ても最初の x は PointerButton の press 座標を使うべき"
        );
        assert!(
            (preview.points[0].y - expected_press_origin.y).abs() < 0.01,
            "同一 frame 内に move が来ても最初の y は PointerButton の press 座標を使うべき"
        );
    }

    #[test]
    fn horizontal_guide_line_extends_to_frame_edges() {
        let line = pauseink_template_layout::GuideLine {
            start: pauseink_template_layout::Point::new(120.0, 200.0),
            end: pauseink_template_layout::Point::new(360.0, 176.0),
            kind: GuideLineKind::Main,
        };

        let (start, end) = extend_horizontal_guide_line_to_frame_width(&line, 1280);

        assert!((start.x - 0.0).abs() < 0.01);
        assert!((end.x - 1280.0).abs() < 0.01);
        assert!(start.y > end.y, "傾きは保ったまま frame 両端へ伸ばしたい");
    }

    #[test]
    fn live_preview_width_matches_downscaled_overlay_scale() {
        let frame_rect = Rect::from_min_size(Pos2::new(8.0, 8.0), Vec2::new(640.0, 360.0));
        let width = live_preview_stroke_width(8.0, frame_rect, 1280, 720);

        assert!((width - 4.0).abs() < 0.01);
        assert!((live_preview_dot_radius(width) - 2.0).abs() < 0.01);
    }

    fn central_height_with_scrollable_bottom_tab(item_count: usize) -> f32 {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        app.bottom_tab = BottomTab::Logs;
        app.logs = (0..item_count)
            .map(|index| format!("object-{index:03} / stroke:1 / page:0 / z:0"))
            .collect();
        let ctx = initialized_test_context();
        let mut central_height = 0.0;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 600.0))),
                ..Default::default()
            },
            |ctx| {
                egui::TopBottomPanel::bottom("bottom-tab-test")
                    .default_height(180.0)
                    .min_height(120.0)
                    .max_height(270.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("ログ");
                        });
                        ui.separator();
                        app.draw_bottom_tab_scroll_region(ui);
                    });
                egui::CentralPanel::default().show(ctx, |ui| {
                    central_height = ui.max_rect().height();
                });
            },
        );
        central_height
    }

    #[test]
    fn scrollable_bottom_tab_keeps_central_panel_height_stable_when_rows_increase() {
        let small = central_height_with_scrollable_bottom_tab(1);
        let large = central_height_with_scrollable_bottom_tab(400);

        assert!((small - large).abs() < 1.0);
    }

    #[test]
    fn macos_runtime_help_mentions_homebrew() {
        let lines = ffmpeg_runtime_help(
            Path::new("/tmp/pauseink_data/runtime"),
            "macos",
            "macos-aarch64",
        );

        assert!(lines.iter().any(|line| line.contains("/opt/homebrew/bin")));
        assert!(lines.iter().any(|line| line.contains("macos-aarch64")));
    }

    #[test]
    fn linux_runtime_help_mentions_common_system_paths() {
        let lines = ffmpeg_runtime_help(
            Path::new("/tmp/pauseink_data/runtime"),
            "linux",
            "linux-x86_64",
        );

        assert!(lines.iter().any(|line| line.contains("/usr/bin")));
        assert!(lines.iter().any(|line| line.contains("Linuxbrew")));
    }

    #[test]
    fn guide_capture_state_keeps_same_object_until_modifier_release() {
        let mut state = GuideCaptureState::default();
        let object_id = pauseink_domain::GlyphObjectId::new("object-1");

        state.start();
        state.record_committed_object(object_id.clone());

        assert_eq!(state.current_target_object_id(), Some(object_id.clone()));
        assert_eq!(state.note_modifier_release(false), Some(object_id));
        assert_eq!(state.current_target_object_id(), None);
        assert!(!state.in_progress);
    }

    #[test]
    fn guide_capture_state_defers_finalize_when_modifier_is_released_mid_drag() {
        let mut state = GuideCaptureState::default();
        let object_id = pauseink_domain::GlyphObjectId::new("object-2");

        state.start();
        state.record_committed_object(object_id.clone());

        assert_eq!(state.note_modifier_release(true), None);
        assert!(state.finalize_pending);
        assert_eq!(state.current_target_object_id(), Some(object_id.clone()));
        assert_eq!(state.take_if_pending_after_commit(), Some(object_id));
        assert!(!state.in_progress);
    }

    #[test]
    fn template_slot_stepper_supports_previous_and_next_without_overflow() {
        assert_eq!(step_template_slot_index(0, 0, -1), 0);
        assert_eq!(step_template_slot_index(0, 3, -1), 0);
        assert_eq!(step_template_slot_index(1, 3, -1), 0);
        assert_eq!(step_template_slot_index(1, 3, 1), 2);
        assert_eq!(step_template_slot_index(2, 3, 1), 2);
    }

    #[test]
    fn bottom_panel_content_width_is_clamped_to_safe_range() {
        assert_eq!(clamp_bottom_panel_content_width(120.0), 320.0);
        assert_eq!(clamp_bottom_panel_content_width(960.0), 960.0);
        assert_eq!(clamp_bottom_panel_content_width(20_000.0), 8_192.0);
    }

    #[test]
    fn pending_export_progress_updates_and_completion_clears_worker_state() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let (sender, receiver) = std::sync::mpsc::channel();

        app.pending_export = Some(PendingExportJob {
            receiver,
            summary: "テスト export".to_owned(),
            output_path: PathBuf::from("/tmp/out.webm"),
            progress_fraction: 0.0,
            progress_label: "開始待ち".to_owned(),
        });

        sender
            .send(ExportThreadMessage::Progress(ExportProgressUpdate {
                fraction: 0.45,
                stage_label: "フレーム生成中".to_owned(),
            }))
            .expect("progress message should send");
        app.poll_pending_export();

        let pending = app
            .pending_export
            .as_ref()
            .expect("pending export should remain");
        assert!((pending.progress_fraction - 0.45).abs() < 0.001);
        assert_eq!(pending.progress_label, "フレーム生成中");

        sender
            .send(ExportThreadMessage::Finished(Err("失敗".to_owned())))
            .expect("final message should send");
        app.poll_pending_export();

        assert!(app.pending_export.is_none());
        assert_eq!(app.export.jobs.len(), 1);
        assert!(app.export.jobs[0].status.contains("失敗"));
    }

    #[test]
    fn pending_export_progress_does_not_move_backwards_on_retry() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);
        let (sender, receiver) = std::sync::mpsc::channel();

        app.pending_export = Some(PendingExportJob {
            receiver,
            summary: "テスト export".to_owned(),
            output_path: PathBuf::from("/tmp/out.webm"),
            progress_fraction: 0.96,
            progress_label: "合成動画を書き出し中 (57%)".to_owned(),
        });

        sender
            .send(ExportThreadMessage::Progress(ExportProgressUpdate {
                fraction: 0.93,
                stage_label: "合成動画を書き出し中 (14%)".to_owned(),
            }))
            .expect("progress message should send");
        app.poll_pending_export();

        let pending = app
            .pending_export
            .as_ref()
            .expect("pending export should remain");
        assert!((pending.progress_fraction - 0.96).abs() < 0.001);
        assert_eq!(pending.progress_label, "合成動画を書き出し中 (14%)");
    }

    #[test]
    fn export_progress_hint_explains_finalizing_stages() {
        assert!(
            export_progress_hint("2/3 合成動画を書き出し中 (最終処理中)")
                .contains("コンテナの最終化")
        );
        assert!(export_progress_hint("3/3 一時ファイルを整理中").contains("一時ファイル"));
    }

    #[test]
    fn clearing_guide_state_drops_overlay_and_stale_capture_context() {
        let temp_dir = tempdir().expect("temp dir");
        let portable_paths = PortablePaths::from_root(temp_dir.path().join("pauseink_data"));
        portable_paths.ensure_exists().expect("portable dirs");
        let mut app = DesktopApp::new(portable_paths, Settings::default(), None, None);

        app.guide_state = Some(GuideOverlayState::from_reference_bounds(
            pauseink_domain::Point2 { x: 100.0, y: 200.0 },
            pauseink_domain::Point2 { x: 160.0, y: 280.0 },
        ));
        app.guide_geometry = app
            .guide_state
            .map(|guide_state| guide_state.build_geometry(0.0));
        app.last_committed_object_bounds = Some((
            pauseink_domain::Point2 { x: 100.0, y: 200.0 },
            pauseink_domain::Point2 { x: 160.0, y: 280.0 },
        ));
        app.guide_capture_armed = true;
        app.guide_modifier_was_down = true;
        app.guide_modifier_used_for_stroke = true;
        app.guide_capture_state.start();
        app.guide_capture_state
            .record_committed_object(pauseink_domain::GlyphObjectId::new("object-1"));

        app.clear_guide_state();

        assert!(app.guide_state.is_none());
        assert!(app.guide_geometry.is_none());
        assert!(app.last_committed_object_bounds.is_none());
        assert!(!app.guide_capture_armed);
        assert!(!app.guide_modifier_was_down);
        assert!(!app.guide_modifier_used_for_stroke);
        assert!(app.guide_capture_state.current_target_object_id().is_none());
        assert!(!app.guide_capture_state.in_progress);
    }
}
