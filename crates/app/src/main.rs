use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke as EguiStroke, Vec2};
use pauseink_app::AppSession;
use pauseink_fonts::{discover_local_font_families, google_font_cache_file};
use pauseink_media::{
    canvas_point_to_frame, default_platform_id, discover_runtime, fit_frame_to_canvas,
    frame_point_to_canvas, CanvasSize, FfprobeMediaProvider, MediaProvider, PreviewFrame,
};
use pauseink_portable_fs::{
    load_settings_from_str, save_settings_to_string, portable_root_from_env, PortablePaths,
    Settings,
};
use pauseink_renderer::{render_overlay_rgba, RenderRequest};
use pauseink_template_layout::{
    build_guide_geometry, create_template_slots, GuideGeometry, GuideLineKind, GuidePlacement,
    Point, TemplateSettings, UnderlayMode,
};

fn main() -> Result<()> {
    let executable_dir = std::env::current_exe()?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().expect("current dir should resolve"));
    let portable_paths = PortablePaths::from_root(portable_root_from_env(&executable_dir));
    portable_paths.ensure_exists()?;

    let settings = if portable_paths.settings_file().exists() {
        load_settings_from_str(&fs::read_to_string(portable_paths.settings_file())?)?
    } else {
        Settings::default()
    };

    let runtime = discover_runtime(&portable_paths.runtime_dir, &default_platform_id(), true).ok();
    let options = eframe::NativeOptions::default();
    let app = DesktopApp::new(portable_paths, settings, runtime);

    eframe::run_native("PauseInk", options, Box::new(|_cc| Ok(Box::new(app))))?;
    Ok(())
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
            font_family: "システム既定".to_owned(),
            placement_armed: false,
            placed_slots: None,
            current_slot_index: 0,
            slot_object_ids: Vec::new(),
        }
    }
}

struct DesktopApp {
    session: AppSession,
    portable_paths: PortablePaths,
    settings: Settings,
    provider: Option<FfprobeMediaProvider>,
    runtime_status: String,
    logs: Vec<String>,
    local_font_families: Vec<String>,
    template: TemplatePreviewState,
    guide_geometry: Option<GuideGeometry>,
    bottom_tab: BottomTab,
    preview_texture: Option<egui::TextureHandle>,
    preview_key: Option<(PathBuf, i64, u32, u32)>,
    overlay_texture: Option<egui::TextureHandle>,
    overlay_key: Option<(i64, usize, usize, u32, u32)>,
    canvas_drag_active: bool,
    guide_capture_armed: bool,
    recovery_prompt_open: bool,
    last_update_at: Instant,
    last_autosave_at: Instant,
}

impl DesktopApp {
    fn new(
        portable_paths: PortablePaths,
        settings: Settings,
        runtime: Option<pauseink_media::MediaRuntime>,
    ) -> Self {
        let recovery_prompt_open = portable_paths.autosave_file("recovery_latest").exists();
        let provider = runtime.clone().map(FfprobeMediaProvider::new);
        let runtime_status = runtime
            .map(|runtime| {
                format!(
                    "runtime: {} ({:?})",
                    runtime
                        .build_summary
                        .unwrap_or_else(|| runtime.ffmpeg_path.display().to_string()),
                    runtime.origin
                )
            })
            .unwrap_or_else(|| "runtime: 未検出".to_owned());
        let local_font_families = discover_local_font_families(&settings.local_font_dirs);

        Self {
            session: AppSession::default(),
            portable_paths,
            settings,
            provider,
            runtime_status,
            logs: Vec::new(),
            local_font_families,
            template: TemplatePreviewState::default(),
            guide_geometry: None,
            bottom_tab: BottomTab::Outline,
            preview_texture: None,
            preview_key: None,
            overlay_texture: None,
            overlay_key: None,
            canvas_drag_active: false,
            guide_capture_armed: false,
            recovery_prompt_open,
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

    fn import_media(&mut self, path: PathBuf) {
        let Some(provider) = self.provider.as_ref() else {
            self.push_log("FFmpeg runtime が見つからないためメディアを読込できません。");
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
            Ok(session) => {
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
    ) {
        let key = (
            self.session.current_time().ticks,
            self.session.project.strokes.len(),
            self.session.project.clear_events.len(),
            target_width,
            target_height,
        );
        if self.overlay_key.as_ref() == Some(&key) {
            return;
        }

        match render_overlay_rgba(&RenderRequest {
            project: &self.session.project,
            time: self.session.current_time(),
            width: target_width.max(1),
            height: target_height.max(1),
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

        if self.template.placement_armed && response.clicked() {
            if let Some(pointer_position) = pointer_position {
                let relative = Pos2::new(
                    pointer_position.x - frame_rect.left(),
                    pointer_position.y - frame_rect.top(),
                );
                let slots = create_template_slots(
                    &self.template.text,
                    Point::new(relative.x, relative.y),
                    &self.template.settings,
                );
                self.template.current_slot_index = 0;
                self.template.slot_object_ids = vec![None; slots.len()];
                self.template.placed_slots = Some(slots);
                self.template.placement_armed = false;
                self.push_log("テンプレート配置を確定しました。");
            }
            return;
        }

        if response.drag_started() {
            self.guide_capture_armed = ctx.input(|input| {
                if cfg!(target_os = "macos") {
                    input.modifiers.alt
                } else {
                    input.modifiers.ctrl
                }
            });
            if let Some(pointer_position) = pointer_position {
                let local = pauseink_domain::Point2 {
                    x: pointer_position.x - response.rect.left(),
                    y: pointer_position.y - response.rect.top(),
                };
                if let Some(frame_point) = canvas_point_to_frame(
                    local,
                    pauseink_media::CanvasRect {
                        x: frame_rect.left() - response.rect.left(),
                        y: frame_rect.top() - response.rect.top(),
                        width: frame_rect.width(),
                        height: frame_rect.height(),
                    },
                    frame_width,
                    frame_height,
                ) {
                    self.session.begin_stroke(frame_point, self.session.current_time());
                    self.canvas_drag_active = true;
                }
            }
        }

        if self.canvas_drag_active {
            if let Some(pointer_position) = pointer_position {
                let local = pauseink_domain::Point2 {
                    x: pointer_position.x - response.rect.left(),
                    y: pointer_position.y - response.rect.top(),
                };
                if let Some(frame_point) = canvas_point_to_frame(
                    local,
                    pauseink_media::CanvasRect {
                        x: frame_rect.left() - response.rect.left(),
                        y: frame_rect.top() - response.rect.top(),
                        width: frame_rect.width(),
                        height: frame_rect.height(),
                    },
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
                        if self.guide_capture_armed {
                            if let Some((min, max)) = self.session.object_bounds(&object_id) {
                                self.guide_geometry = Some(build_guide_geometry(
                                    Point::new(min.x, min.y),
                                    GuidePlacement {
                                        cell_width: (max.x - min.x).max(40.0),
                                        cell_height: (max.y - min.y).max(48.0),
                                        slope_degrees: self.settings.guide_slope_degrees,
                                    },
                                ));
                                self.push_log("ガイド基準を更新しました。");
                            }
                        }

                        if self.template.placed_slots.is_some() {
                            if let Some(slot_object) =
                                self.template.slot_object_ids.get_mut(self.template.current_slot_index)
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

    fn draw_template_preview(&self, painter: &egui::Painter, frame_rect: Rect, response: &egui::Response) {
        let hovered_origin = response.interact_pointer_pos().map(|position| {
            Point::new(position.x - frame_rect.left(), position.y - frame_rect.top())
        });

        let slots = if let Some(slots) = &self.template.placed_slots {
            Some(slots.clone())
        } else if self.template.placement_armed {
            hovered_origin.map(|origin| {
                create_template_slots(&self.template.text, origin, &self.template.settings)
            })
        } else {
            None
        };

        if let Some(slots) = slots {
            for (index, slot) in slots.iter().enumerate() {
                let rect = Rect::from_min_size(
                    Pos2::new(frame_rect.left() + slot.origin.x, frame_rect.top() + slot.origin.y),
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
                    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle);
                }
                if matches!(
                    self.template.settings.underlay_mode,
                    UnderlayMode::Outline | UnderlayMode::OutlineAndSlotBox | UnderlayMode::FaintFill
                ) {
                    painter.text(
                        rect.left_top(),
                        egui::Align2::LEFT_TOP,
                        &slot.grapheme,
                        egui::FontId::proportional((self.template.settings.font_size * 0.35).max(14.0)),
                        Color32::from_rgba_unmultiplied(220, 220, 240, 180),
                    );
                }
                if matches!(self.template.settings.underlay_mode, UnderlayMode::FaintFill) {
                    painter.rect_filled(
                        rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(180, 200, 255, 32),
                    );
                }
            }
        }
    }

    fn draw_guide_overlay(&self, painter: &egui::Painter, frame_rect: Rect) {
        let Some(guide) = &self.guide_geometry else {
            return;
        };

        for line in guide
            .horizontal_lines
            .iter()
            .chain(guide.vertical_lines.iter())
        {
            let stroke = match line.kind {
                GuideLineKind::Main => EguiStroke::new(1.5, Color32::from_rgba_unmultiplied(120, 200, 255, 180)),
                GuideLineKind::Helper => EguiStroke::new(1.0, Color32::from_rgba_unmultiplied(120, 200, 255, 80)),
            };
            painter.line_segment(
                [
                    Pos2::new(frame_rect.left() + line.start.x, frame_rect.top() + line.start.y),
                    Pos2::new(frame_rect.left() + line.end.x, frame_rect.top() + line.end.y),
                ],
                stroke,
            );
        }
    }

    fn save_settings(&mut self) {
        match save_settings_to_string(&self.settings) {
            Ok(serialized) => {
                if let Err(error) = fs::write(self.portable_paths.settings_file(), serialized) {
                    self.push_log(format!("settings 保存失敗: {error}"));
                }
            }
            Err(error) => self.push_log(format!("settings 直列化失敗: {error}")),
        }
    }

    fn maybe_autosave(&mut self) {
        if !self.session.dirty || self.last_autosave_at.elapsed() < Duration::from_secs(10) {
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
            Some(session) => {
                self.session = session;
                self.preview_key = None;
                self.overlay_key = None;
                self.recovery_prompt_open = false;
                self.push_log(format!("autosave から復旧: {}", autosave_path.display()));
            }
            None => self.push_log("autosave 復旧に失敗しました。"),
        }
    }
}

impl eframe::App for DesktopApp {
    fn on_exit(&mut self) {
        self.save_settings();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.advance_playback(&ctx);
        self.maybe_autosave();

        if self.recovery_prompt_open {
            egui::Window::new("Recovery")
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
            ui.horizontal_wrapped(|ui| {
                if ui.button("開く").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PauseInk", &["pauseink"])
                        .pick_file()
                    {
                        self.open_project(path);
                    }
                }
                if ui.button("保存").clicked() {
                    if let Some(path) = self.session.document_path.clone() {
                        self.save_project(path);
                    } else if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PauseInk", &["pauseink"])
                        .save_file()
                    {
                        self.save_project(path);
                    }
                }
                if ui.button("別名保存").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PauseInk", &["pauseink"])
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
                if ui.button("Undo").clicked() {
                    if let Err(error) = self.session.undo() {
                        self.push_log(format!("undo 失敗: {error:#}"));
                    }
                }
                if ui.button("Redo").clicked() {
                    if let Err(error) = self.session.redo() {
                        self.push_log(format!("redo 失敗: {error:#}"));
                    }
                }
                if ui
                    .button(if self
                        .session
                        .playback
                        .as_ref()
                        .map(|playback| playback.is_playing)
                        .unwrap_or(false)
                    {
                        "一時停止"
                    } else {
                        "再生"
                    })
                    .clicked()
                {
                    if self
                        .session
                        .playback
                        .as_ref()
                        .map(|playback| playback.is_playing)
                        .unwrap_or(false)
                    {
                        self.session.pause();
                    } else {
                        self.session.play();
                    }
                }
                if ui.button("Clear").clicked() {
                    match self
                        .session
                        .insert_clear_event(pauseink_domain::ClearKind::Instant)
                    {
                        Ok(clear_id) => self.push_log(format!("clear event を挿入: {}", clear_id.0)),
                        Err(error) => self.push_log(format!("clear event 挿入失敗: {error:#}")),
                    }
                }

                if let Some(duration) = self
                    .session
                    .playback
                    .as_ref()
                    .and_then(|playback| playback.media.duration())
                {
                    let mut current_ms = self.session.current_time().ticks as f64;
                    let response = ui.add(
                        egui::Slider::new(&mut current_ms, 0.0..=duration.ticks as f64)
                            .text("現在位置 ms")
                            .show_value(true),
                    );
                    if response.changed() {
                        self.session
                            .seek(pauseink_domain::MediaTime::from_millis(current_ms.round() as i64));
                    }
                }

                ui.separator();
                ui.label(format!("状態: {}", self.session.transport_summary()));
                ui.label(format!("dirty: {}", if self.session.dirty { "あり" } else { "なし" }));
            });
        });

        egui::Panel::left("left_panel")
            .default_width(250.0)
            .show(&ctx, |ui| {
                ui.heading("メディア");
                ui.label(&self.runtime_status);
                if let Some(path) = self.session.media_source_hint() {
                    ui.label(format!("source: {}", path.display()));
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
                ui.text_edit_singleline(&mut self.template.font_family);
                ui.add(
                    egui::Slider::new(&mut self.template.settings.font_size, 24.0..=180.0)
                        .text("font size"),
                );
                ui.add(
                    egui::Slider::new(&mut self.template.settings.tracking, 0.0..=48.0)
                        .text("tracking"),
                );
                ui.add(
                    egui::Slider::new(&mut self.template.settings.slope_degrees, -20.0..=20.0)
                        .text("slope"),
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
                    self.local_font_families =
                        discover_local_font_families(&self.settings.local_font_dirs);
                }
                ui.label(format!("ローカル候補: {} 件", self.local_font_families.len()));
                for family in self.local_font_families.iter().take(8) {
                    ui.label(format!("・{family}"));
                }
                ui.separator();
                ui.label("Google Fonts 設定:");
                for family in &self.settings.google_fonts.families {
                    let cached = google_font_cache_file(&self.portable_paths.google_fonts_cache_dir(), family);
                    ui.label(format!(
                        "・{} ({})",
                        family,
                        if cached.exists() { "cache あり" } else { "cache なし" }
                    ));
                }
            });

        egui::Panel::right("inspector")
            .default_width(260.0)
            .show(&ctx, |ui| {
                ui.heading("Inspector");
                let mut title = self.session.project_title();
                ui.label(format!("タイトル: {}", title));
                if ui.text_edit_singleline(&mut title).changed() {
                    self.session.set_project_title(title);
                }
                ui.separator();
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
                        .text("thickness"),
                );
                ui.add(
                    egui::Slider::new(&mut self.session.active_style.opacity, 0.05..=1.0)
                        .text("opacity"),
                );
                ui.add(
                    egui::Slider::new(
                        &mut self.session.active_style.stabilization_strength,
                        0.0..=1.0,
                    )
                    .text("stabilization"),
                );
                ui.separator();
                ui.label("ガイド");
                ui.add(
                    egui::Slider::new(&mut self.settings.guide_slope_degrees, -20.0..=20.0)
                        .text("guide slope"),
                );
                if ui.button("ガイド解除").clicked() {
                    self.guide_geometry = None;
                }
                ui.separator();
                ui.label("export");
                ui.label("v1.0 engine は次フェーズで接続します。");
                ui.label("ここでは profile/family UI の足場のみ準備中です。");
            });

        egui::Panel::bottom("bottom_tabs")
            .default_height(180.0)
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in [
                        (BottomTab::Outline, "Object Outline"),
                        (BottomTab::PageEvents, "Page Events"),
                        (BottomTab::ExportQueue, "Export Queue"),
                        (BottomTab::Logs, "Logs"),
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
                        ui.label("export queue は次フェーズで接続します。");
                        ui.label("現状は settings 計算と capability 判定の backend 済みです。");
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
            );

            if let Some(texture) = &self.preview_texture {
                painter.image(
                    texture.id(),
                    frame_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
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

            self.draw_template_preview(&painter, frame_rect, &response);
            self.draw_guide_overlay(&painter, frame_rect);

            if let Some(slots) = &self.template.placed_slots {
                if let Some(slot) = slots.get(self.template.current_slot_index) {
                    painter.text(
                        frame_rect.left_top() + egui::vec2(12.0, 12.0),
                        egui::Align2::LEFT_TOP,
                        format!("slot {}/{}: {}", self.template.current_slot_index + 1, slots.len(), slot.grapheme),
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(255, 232, 120),
                    );
                }
            }

            self.handle_canvas_input(&response, frame_rect, frame_width, frame_height, &ctx);

            if self.canvas_drag_active {
                if let Some(pointer_position) = response.interact_pointer_pos() {
                    let local = pauseink_domain::Point2 {
                        x: pointer_position.x - frame_rect.left(),
                        y: pointer_position.y - frame_rect.top(),
                    };
                    if let Some(frame_point) = frame_point_to_canvas(
                        local,
                        pauseink_media::CanvasRect {
                            x: 0.0,
                            y: 0.0,
                            width: frame_rect.width(),
                            height: frame_rect.height(),
                        },
                        frame_width,
                        frame_height,
                    ) {
                        painter.circle_filled(
                            Pos2::new(frame_rect.left() + frame_point.x, frame_rect.top() + frame_point.y),
                            3.0,
                            Color32::from_rgb(255, 255, 255),
                        );
                    }
                }
            }
        });
    }
}

fn preview_frame_to_color_image(frame: &PreviewFrame) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [frame.width as usize, frame.height as usize],
        &frame.rgba_pixels,
    )
}
