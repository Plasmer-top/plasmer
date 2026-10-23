use eframe::egui::{
  self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Layout, RichText, Sense, Stroke,
  StrokeKind, TextFormat, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::discord_rpc::DiscordRpc;
use crate::input::{
  action_name, action_to_trigger, any_capture_key_down, capture_pressed_token, parse_action,
};
use crate::settings::{self, ActionKeyMode, ActionPressMode, SavedSettings};
use crate::threads::{SharedState, spawn_listener_thread, spawn_spam_thread};
use crate::updater::{self, UpdateState};
use crate::win32::{PressMode, SpamAction, qpc, qpc_freq, using_send_input_fallback};

const BG: Color32 = Color32::from_rgb(12, 14, 18);
const SURFACE: Color32 = Color32::from_rgb(19, 22, 28);
const SURFACE_2: Color32 = Color32::from_rgb(27, 31, 39);
const SURFACE_HOVER: Color32 = Color32::from_rgb(36, 41, 51);
const BORDER: Color32 = Color32::from_rgb(34, 39, 49);
const BORDER_STRONG: Color32 = Color32::from_rgb(54, 61, 76);
const TEXT: Color32 = Color32::from_rgb(233, 237, 244);
const TEXT_MUTED: Color32 = Color32::from_rgb(146, 154, 171);
const TEXT_DIM: Color32 = Color32::from_rgb(98, 106, 122);
const ACCENT: Color32 = Color32::from_rgb(24, 132, 242);
const ACCENT_HOVER: Color32 = Color32::from_rgb(58, 154, 250);
const ACCENT_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(24, 132, 242, 36);
const GREEN: Color32 = Color32::from_rgb(52, 211, 153);
const AMBER: Color32 = Color32::from_rgb(245, 182, 66);
const AMBER_SOFT: Color32 = Color32::from_rgba_unmultiplied_const(245, 182, 66, 20);
const AMBER_LINE: Color32 = Color32::from_rgba_unmultiplied_const(245, 182, 66, 80);
const RED: Color32 = Color32::from_rgb(240, 98, 98);

const CAPTION: f32 = 13.0;
const SEMIBOLD: &str = "semibold";

const WEBSITE_URL: &str = "https://plasmer.top";

const CAPTURE_HINT: &str = "Press any key or mouse button · click the box to cancel";
const BURST_HELP: &str = "One-shot burst hotkey.\n\n\
  Tap the burst key once: Plasmer fires exactly the set number of clicks at the \
  burst speed — same high-precision timing as normal clicking — then stops on its own.\n\n\
  While a burst is running the key is locked out; it re-arms after the last click. \
  The normal trigger keeps working independently.";

fn format_elapsed(secs: f64) -> String {
  let total_centis = (secs.max(0.0) * 100.0).round() as u64;
  let hours = total_centis / 360_000;
  let minutes = (total_centis / 6_000) % 60;
  let seconds = (total_centis / 100) % 60;
  let centis = total_centis % 100;

  if hours > 0 {
    format!("{hours}:{minutes:02}:{seconds:02}")
  } else {
    format!("{minutes:02}:{seconds:02}.{centis:02}")
  }
}

fn key_label(token: &str) -> String {
  parse_action(token)
    .map(action_name)
    .unwrap_or_else(|| "—".into())
}

fn semibold(size: f32) -> FontId {
  FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

fn text(s: impl Into<String>, size: f32, color: Color32) -> RichText {
  RichText::new(s).size(size).color(color)
}

fn strong(s: impl Into<String>, size: f32, color: Color32) -> RichText {
  RichText::new(s).font(semibold(size)).color(color)
}

fn setup_fonts(ctx: &egui::Context) {
  let mut fonts = egui::FontDefinitions::default();
  let dir =
    std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into()))
      .join("Fonts");

  if let Ok(data) = std::fs::read(dir.join("segoeui.ttf")) {
    fonts.font_data.insert(
      "segoe-ui".into(),
      Arc::new(egui::FontData::from_owned(data)),
    );
    fonts
      .families
      .get_mut(&FontFamily::Proportional)
      .unwrap()
      .insert(0, "segoe-ui".into());
  }

  let mut semibold = fonts.families[&FontFamily::Proportional].clone();
  if let Ok(data) = std::fs::read(dir.join("seguisb.ttf")) {
    fonts.font_data.insert(
      "segoe-ui-semibold".into(),
      Arc::new(egui::FontData::from_owned(data)),
    );
    semibold.insert(0, "segoe-ui-semibold".into());
  }
  fonts
    .families
    .insert(FontFamily::Name(SEMIBOLD.into()), semibold);

  ctx.set_fonts(fonts);
}

fn setup_style(ctx: &egui::Context) {
  ctx.set_theme(egui::Theme::Dark);

  let mut style = egui::Style::default();
  style
    .text_styles
    .insert(egui::TextStyle::Body, FontId::proportional(14.0));
  style.override_text_style = Some(egui::TextStyle::Body);

  let v = &mut style.visuals;
  v.override_text_color = Some(TEXT);
  v.panel_fill = BG;
  v.window_fill = SURFACE;
  v.window_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
  v.extreme_bg_color = SURFACE_2;
  v.hyperlink_color = TEXT_MUTED;
  v.selection.bg_fill = ACCENT_SOFT;
  v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
  for (w, fill, stroke) in [
    (&mut v.widgets.inactive, SURFACE_2, BORDER),
    (&mut v.widgets.hovered, SURFACE_HOVER, BORDER_STRONG),
    (&mut v.widgets.active, SURFACE_HOVER, ACCENT),
  ] {
    w.bg_fill = fill;
    w.weak_bg_fill = fill;
    w.bg_stroke = Stroke::new(1.0_f32, stroke);
    w.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.corner_radius = CornerRadius::same(8);
    w.expansion = 0.0;
  }

  style.spacing.item_spacing = vec2(8.0, 6.0);
  style.spacing.button_padding = vec2(12.0, 6.0);
  style.spacing.tooltip_width = 280.0;
  style.interaction.tooltip_delay = 0.0;

  ctx.set_style_of(egui::Theme::Dark, style);
}

fn card(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui)) {
  egui::Frame::new()
    .fill(SURFACE)
    .stroke(Stroke::new(1.0_f32, BORDER))
    .corner_radius(CornerRadius::same(12))
    .inner_margin(egui::Margin::same(14))
    .show(ui, |ui| {
      ui.set_width(ui.available_width());
      add_contents(ui);
    });
}

fn card_header(ui: &mut Ui, title: &str, subtitle: &str, right: impl FnOnce(&mut Ui)) {
  ui.allocate_ui_with_layout(
    vec2(ui.available_width(), 38.0),
    Layout::left_to_right(Align::Center),
    |ui| {
      ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.label(strong(title, 16.0, TEXT));
        ui.label(text(subtitle, 13.0, TEXT_MUTED));
      });
      ui.with_layout(Layout::right_to_left(Align::Center), right);
    },
  );
}

fn wide_button(
  ui: &mut Ui,
  height: f32,
  look: (Color32, Color32, Color32),
  label: &str,
  enabled: bool,
) -> egui::Response {
  let sense = if enabled {
    Sense::CLICK
  } else {
    Sense::hover()
  };
  let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), sense);

  let (mut fill, stroke, color) = look;
  if response.is_pointer_button_down_on() {
    fill = fill.lerp_to_gamma(Color32::BLACK, 0.12);
  } else if response.hovered() {
    fill = fill.lerp_to_gamma(Color32::WHITE, 0.08);
  }

  let painter = ui.painter();
  painter.rect(
    rect,
    10.0,
    fill,
    Stroke::new(1.5_f32, stroke),
    StrokeKind::Inside,
  );
  painter.text(
    rect.center(),
    Align2::CENTER_CENTER,
    label,
    semibold(16.0),
    color,
  );
  response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
  response
}

fn key_chip(
  ui: &mut Ui,
  label: &str,
  name: Option<&str>,
  capturing: bool,
  flagged: bool,
) -> egui::Response {
  let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::CLICK);
  let hovered = response.hovered();

  let (fill, stroke) = if capturing {
    let pulse = 0.55 + 0.45 * (ui.input(|i| i.time) * 4.0).sin().abs() as f32;
    (
      ACCENT_SOFT,
      Stroke::new(1.5_f32, ACCENT.gamma_multiply(pulse)),
    )
  } else if flagged {
    (SURFACE_2, Stroke::new(1.0_f32, RED))
  } else if hovered {
    (SURFACE_HOVER, Stroke::new(1.0_f32, BORDER_STRONG))
  } else {
    (SURFACE_2, Stroke::new(1.0_f32, BORDER))
  };

  let painter = ui.painter();
  painter.rect(rect, 8.0, fill, stroke, StrokeKind::Inside);

  let (shown, font, color) = match (capturing, name) {
    (true, _) => ("Press a key…", FontId::proportional(14.0), ACCENT_HOVER),
    (false, Some(name)) => (name, semibold(15.0), TEXT),
    (false, None) => ("Not set", FontId::proportional(14.0), TEXT_DIM),
  };
  painter.text(rect.center(), Align2::CENTER_CENTER, shown, font, color);

  let enabled = ui.is_enabled();
  response.widget_info(|| {
    let mut info = WidgetInfo::labeled(WidgetType::Button, enabled, label);
    info.current_text_value = Some(shown.to_owned());
    info
  });
  response
}

fn segmented<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, options: &[(T, &str, &str)]) {
  let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::hover());
  ui.painter().rect(
    rect,
    8.0,
    SURFACE_2,
    Stroke::new(1.0_f32, BORDER),
    StrokeKind::Inside,
  );

  let inner = rect.shrink(3.0);
  let width = inner.width() / options.len() as f32;
  for (i, &(value, label, tip)) in options.iter().enumerate() {
    let seg = egui::Rect::from_min_size(
      inner.min + vec2(i as f32 * width, 0.0),
      vec2(width, inner.height()),
    );
    let response = ui
      .interact(seg, ui.id().with(label), Sense::click())
      .on_hover_text(tip);
    if response.clicked() {
      *current = value;
    }

    let selected = *current == value;
    let enabled = ui.is_enabled();
    response
      .widget_info(|| WidgetInfo::selected(WidgetType::RadioButton, enabled, selected, label));
    let fill = if selected {
      ACCENT
    } else if response.hovered() {
      SURFACE_HOVER
    } else {
      Color32::TRANSPARENT
    };
    let (font, color) = if selected {
      (semibold(14.0), Color32::WHITE)
    } else {
      (FontId::proportional(14.0), TEXT_MUTED)
    };
    ui.painter().rect_filled(seg, 6.0, fill);
    ui.painter()
      .text(seg.center(), Align2::CENTER_CENTER, label, font, color);
  }
}

fn toggle_switch(ui: &mut Ui, label: &str, on: &mut bool) {
  let (rect, response) = ui.allocate_exact_size(vec2(40.0, 22.0), Sense::click());
  if response.clicked() {
    *on = !*on;
  }
  let (enabled, checked) = (ui.is_enabled(), *on);
  response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, checked, label));

  let t = ui.ctx().animate_bool_responsive(response.id, *on);
  let track = BORDER_STRONG.lerp_to_gamma(ACCENT, t);
  let x = egui::lerp((rect.left() + 11.0)..=(rect.right() - 11.0), t);
  let radius = if response.hovered() { 8.5 } else { 8.0 };

  ui.painter().rect_filled(rect, 11.0, track);
  ui.painter()
    .circle_filled(pos2(x, rect.center().y), radius, TEXT);
}

fn slider(ui: &mut Ui, label: &str, value: &mut u32, range: RangeInclusive<u32>) {
  let (lo, hi) = (*range.start() as f32, *range.end() as f32);

  let knob = 7.0;
  let (rect, response) =
    ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click_and_drag());
  let rail = egui::Rect::from_center_size(rect.center(), vec2(rect.width() - knob * 2.0, 4.0));

  if let Some(pos) = response.interact_pointer_pos() {
    let t = ((pos.x - rail.left()) / rail.width()).clamp(0.0, 1.0);
    *value = (lo + t * (hi - lo)).round() as u32;
  }

  let (enabled, current) = (ui.is_enabled(), *value);
  response.widget_info(|| WidgetInfo::slider(enabled, current.into(), label));

  let x = rail.left() + rail.width() * ((*value as f32).clamp(lo, hi) - lo) / (hi - lo);
  let active = response.hovered() || response.dragged();
  let painter = ui.painter();
  painter.rect_filled(rail, 2.0, SURFACE_HOVER);
  painter.rect_filled(
    egui::Rect::from_min_max(rail.min, pos2(x, rail.max.y)),
    2.0,
    ACCENT,
  );
  painter.circle(
    pos2(x, rect.center().y),
    if active { knob + 1.0 } else { knob },
    TEXT,
    Stroke::new(2.5_f32, ACCENT),
  );
}

fn value_slider(
  ui: &mut Ui,
  label: &str,
  value: &mut u32,
  slider_range: RangeInclusive<u32>,
  unit: &str,
) {
  ui.allocate_ui_with_layout(
    vec2(ui.available_width(), 26.0),
    Layout::left_to_right(Align::Center),
    |ui| {
      ui.label(text(label, CAPTION, TEXT_MUTED));
      ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add(
          egui::DragValue::new(value)
            .range(1..=1000)
            .speed(0.5)
            .suffix(format!(" {unit}")),
        );
      });
    },
  );
  slider(ui, label, value, slider_range);
}

// No `ui.horizontal`: it misplaces rows in the bottom-up footer.
fn stat_tiles(ui: &mut Ui, tiles: [(&str, String, &str); 5]) {
  let gap = 6.0;
  let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0), Sense::hover());
  let width = (row.width() - gap * 4.0) / 5.0;

  for (i, (label, value, tip)) in tiles.into_iter().enumerate() {
    let rect = egui::Rect::from_min_size(
      row.min + vec2(i as f32 * (width + gap), 0.0),
      vec2(width, row.height()),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, SURFACE_2);
    painter.text(
      rect.center_top() + vec2(0.0, 20.0),
      Align2::CENTER_CENTER,
      &value,
      semibold(16.0),
      TEXT,
    );
    painter.text(
      rect.center_bottom() - vec2(0.0, 14.0),
      Align2::CENTER_CENTER,
      label,
      FontId::proportional(12.0),
      TEXT_DIM,
    );
    let response = ui.interact(rect, ui.id().with(("stat", i)), Sense::hover());
    response
      .widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, format!("{label}: {value}")));
    response.on_hover_text(tip);
  }
}

pub(crate) fn load_icon() -> egui::IconData {
  let bytes = include_bytes!("../assets/icon.png");
  let image = image::load_from_memory(bytes)
    .expect("Failed to load icon")
    .into_rgba8();
  let (width, height) = image.dimensions();

  egui::IconData {
    rgba: image.into_raw(),
    width,
    height,
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptureTarget {
  Trigger,
  Actions,
  Burst,
}

struct Keys {
  trigger: SpamAction,
  action: SpamAction,
  burst: Option<SpamAction>,
}

#[derive(Clone, Copy)]
enum Status {
  Stopped,
  Running,
  Clicking,
  Bursting,
}

impl Status {
  fn label(self) -> &'static str {
    match self {
      Status::Stopped => "Stopped",
      Status::Running => "Running",
      Status::Clicking => "Clicking",
      Status::Bursting => "Bursting",
    }
  }

  fn color(self) -> Color32 {
    match self {
      Status::Stopped => TEXT_MUTED,
      Status::Running => ACCENT_HOVER,
      Status::Clicking => GREEN,
      Status::Bursting => AMBER,
    }
  }
}

type Problem = (&'static str, &'static [CaptureTarget]);

pub(crate) struct App {
  state: Arc<SharedState>,
  rpc: DiscordRpc,
  update_state: Arc<UpdateState>,
  app_version: u32,
  logo: egui::TextureHandle,
  qpc_freq: f64,
  settings: SavedSettings,
  saved: SavedSettings,
  active: bool,
  capturing: Option<CaptureTarget>,
  capture_armed: bool,
  capture_hovered: bool,
  send_input_fallback: bool,
}

impl App {
  pub(crate) fn new(
    cc: &eframe::CreationContext,
    state: Arc<SharedState>,
    app_version: u32,
  ) -> Self {
    spawn_listener_thread(state.clone());
    let saved = settings::load_settings();
    let rpc = DiscordRpc::new(app_version, saved.cps);
    let update_state = updater::start_update_check(&app_version.to_string());
    Self::with_parts(&cc.egui_ctx, state, app_version, saved, rpc, update_state)
  }

  fn with_parts(
    ctx: &egui::Context,
    state: Arc<SharedState>,
    app_version: u32,
    saved: SavedSettings,
    rpc: DiscordRpc,
    update_state: Arc<UpdateState>,
  ) -> Self {
    setup_fonts(ctx);
    setup_style(ctx);

    let mut settings = saved.clone();
    settings.cps = settings.cps.clamp(1, 1000);
    settings.burst_clicks = settings.burst_clicks.clamp(1, 1000);
    settings.burst_cps = settings.burst_cps.clamp(1, 1000);

    let icon = load_icon();
    let logo = ctx.load_texture(
      "logo",
      egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
      ),
      egui::TextureOptions {
        mipmap_mode: Some(egui::TextureFilter::Linear),
        ..egui::TextureOptions::LINEAR
      },
    );

    Self {
      rpc,
      update_state,
      state,
      app_version,
      logo,
      qpc_freq: qpc_freq() as f64,
      settings,
      saved,
      active: false,
      capturing: None,
      capture_armed: false,
      capture_hovered: false,
      send_input_fallback: using_send_input_fallback(),
    }
  }
}

impl eframe::App for App {
  fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
    self.rpc.update();
    self.capture_hovered = false;

    let keys = self.validate();
    let flagged: &[CaptureTarget] = match &keys {
      Err((_, targets)) => targets,
      Ok(_) => &[],
    };
    let mut toggle = false;

    egui::CentralPanel::default()
      .frame(
        egui::Frame::new()
          .fill(BG)
          .inner_margin(egui::Margin::symmetric(20, 16)),
      )
      .show_inside(ui, |ui| {
        // Not a bottom Panel: it sizes itself from the previous frame and glitches when it grows.
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
          toggle = ui.scope(|ui| self.show_footer(ui, &keys)).inner;
          ui.add_space(12.0);

          ui.with_layout(Layout::top_down(Align::Min), |ui| {
            self.show_header(ui);
            ui.add_space(12.0);
            self.show_update_notice(ui);

            egui::ScrollArea::vertical()
              .auto_shrink([false, false])
              .show(ui, |ui| {
                ui.add_enabled_ui(!self.active, |ui| {
                  self.show_keys(ui, flagged);
                  ui.add_space(10.0);
                  self.show_clicking(ui);
                  ui.add_space(10.0);
                  self.show_burst(ui, flagged);
                });
              });
          });
        });
      });

    let ctx = ui.ctx().clone();

    // After drawing, so no frame mixes old and new state.
    if toggle {
      if self.active {
        self.stop();
      } else {
        self.start();
      }
      ctx.request_repaint();
    }

    self.poll_capture(&ctx);
    self.autosave(&ctx);

    ctx.request_repaint_after(Duration::from_millis(50));
  }
}

impl Drop for App {
  fn drop(&mut self) {
    self.state.shutdown_all();
    self.rpc.shutdown();
  }
}

impl App {
  fn show_header(&self, ui: &mut Ui) {
    ui.allocate_ui_with_layout(
      vec2(ui.available_width(), 32.0),
      Layout::left_to_right(Align::Center),
      |ui| {
        let size = self.logo.size_vec2();
        let height = 26.0;
        ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
          self.logo.id(),
          vec2(height * size.x / size.y, height),
        )));
        ui.label(strong("Plasmer", 20.0, TEXT));
        ui.label(text(format!("v{}", self.app_version), 13.0, TEXT_DIM));

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
          ui.spacing_mut().item_spacing.x = 14.0;
          ui.add_enabled(
            self.capturing.is_none(),
            egui::Hyperlink::from_label_and_url(text("Website", 14.0, TEXT_MUTED), WEBSITE_URL),
          );
          if self.send_input_fallback {
            ui.label(text("\u{26A0} Fallback input", 13.0, AMBER))
              .on_hover_text("The optimized SendInput could not be loaded");
          }
        });
      },
    );
  }

  fn show_update_notice(&self, ui: &mut Ui) {
    let Some(update) = self.update_state.available_update() else {
      return;
    };

    egui::Frame::new()
      .fill(AMBER_SOFT)
      .stroke(Stroke::new(1.0_f32, AMBER_LINE))
      .corner_radius(CornerRadius::same(10))
      .inner_margin(egui::Margin::symmetric(14, 10))
      .show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
          ui.label(strong(
            format!("Update available: v{}", update.latest_version),
            13.5,
            AMBER,
          ));
          ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_enabled(
              self.capturing.is_none(),
              egui::Hyperlink::from_label_and_url(
                strong("Download", 13.5, TEXT),
                &update.download_url,
              ),
            );
          });
        });
      });
    ui.add_space(10.0);
  }

  fn status(&self) -> Status {
    if !self.active {
      Status::Stopped
    } else if self.state.burst_active.load(Ordering::Relaxed) {
      Status::Bursting
    } else if self.state.running.load(Ordering::Relaxed) {
      Status::Clicking
    } else {
      Status::Running
    }
  }

  fn status_lines(&self, keys: &Result<Keys, Problem>) -> Vec<(String, Color32)> {
    let s = &self.settings;
    let trigger = key_label(&s.trigger);
    let action = key_label(&s.actions);

    match (self.status(), keys) {
      (Status::Stopped, Err((msg, _))) => vec![(msg.to_string(), AMBER)],
      (Status::Stopped, Ok(_)) => vec![],
      (Status::Bursting, _) => vec![(
        format!("Firing {} clicks at {} CPS", s.burst_clicks, s.burst_cps),
        TEXT_MUTED,
      )],
      (Status::Clicking, _) => vec![(
        match s.action_key_mode {
          ActionKeyMode::Hold => format!("Release {trigger} to stop clicking"),
          ActionKeyMode::Toggle => format!("Press {trigger} again to stop clicking"),
        },
        TEXT_MUTED,
      )],
      (Status::Running, _) => {
        let mut lines = vec![(
          match s.action_key_mode {
            ActionKeyMode::Hold => format!("Hold {trigger} to click {action} at {} CPS", s.cps),
            ActionKeyMode::Toggle => {
              format!(
                "Press {trigger} to start clicking {action} at {} CPS",
                s.cps
              )
            }
          },
          TEXT_MUTED,
        )];
        if s.burst_enabled {
          lines.push((
            format!(
              "Tap {} to fire {} clicks",
              key_label(&s.burst_trigger),
              s.burst_clicks
            ),
            TEXT_MUTED,
          ));
        }
        lines
      }
    }
  }

  fn show_footer(&self, ui: &mut Ui, keys: &Result<Keys, Problem>) -> bool {
    let enabled = self.active || (keys.is_ok() && self.capturing.is_none());
    let (look, label) = match (self.active, enabled) {
      (true, _) => ((SURFACE_2, BORDER_STRONG, TEXT), "Stop"),
      (false, true) => ((ACCENT, Color32::TRANSPARENT, Color32::WHITE), "Start"),
      (false, false) => ((SURFACE_2, BORDER, TEXT_DIM), "Start"),
    };
    let mut response = wide_button(ui, 46.0, look, label, enabled);
    if let (false, Err((msg, _))) = (self.active, keys) {
      response = response.on_hover_text(*msg);
    }
    ui.add_space(6.0);

    ui.scope(|ui| self.show_status_lines(ui, keys));

    if self.active {
      ui.add_space(10.0);
      stat_tiles(ui, self.session_stats());
    }

    response.clicked()
  }

  fn show_status_lines(&self, ui: &mut Ui, keys: &Result<Keys, Problem>) {
    let status = self.status();
    let mut lines = self.status_lines(keys);
    let first = (!lines.is_empty()).then(|| lines.remove(0));
    for (line, color) in lines.into_iter().rev() {
      ui.label(text(line, 14.0, color));
    }
    let mut job = egui::text::LayoutJob::default();
    job.append(
      status.label(),
      0.0,
      TextFormat::simple(semibold(14.0), status.color()),
    );
    if let Some((line, color)) = first {
      job.append(
        &format!("  ·  {line}"),
        0.0,
        TextFormat::simple(FontId::proportional(14.0), color),
      );
    }
    ui.label(job);
  }

  fn session_stats(&self) -> [(&'static str, String, &'static str); 5] {
    let st = &self.state;
    let load = |a: &AtomicU64| f64::from_bits(a.load(Ordering::Relaxed));
    let dash = || "—".to_owned();

    let running = st.running.load(Ordering::Relaxed);
    let clicks = st.click_count.load(Ordering::Relaxed);

    let (cps, secs) = if running {
      let first = st.session_first_fire_qpc.load(Ordering::Relaxed);
      let interval = st.session_interval_qpc.load(Ordering::Relaxed);
      if first > 0 {
        let now = qpc();
        let secs = (now - first) as f64 / self.qpc_freq;
        let span = (now - first + interval) as f64 / self.qpc_freq;
        (Some(clicks as f64 / span), Some(secs))
      } else {
        (None, None)
      }
    } else if clicks > 0 {
      (
        Some(load(&st.actual_cps)),
        Some(load(&st.last_elapsed_secs)),
      )
    } else {
      (None, None)
    };
    let finished = !running && clicks > 1;

    [
      (
        "Clicks",
        if running || clicks > 0 {
          clicks.to_string()
        } else {
          dash()
        },
        "Clicks sent in the current or last session",
      ),
      (
        "CPS",
        cps.map_or_else(dash, |v| format!("{v:.1}")),
        "Measured clicks per second",
      ),
      (
        "Time",
        secs.map_or_else(dash, format_elapsed),
        "Session length",
      ),
      (
        "Jitter",
        if finished {
          format!("{:.3} ms", load(&st.last_jitter_ms))
        } else {
          dash()
        },
        "Average deviation from the mean click interval.\nShown when the session ends.",
      ),
      (
        "Consistency",
        if finished {
          format!("{:.2}%", load(&st.last_consistency))
        } else {
          dash()
        },
        "How even the click intervals were (100% = perfectly even).\nShown when the session ends.",
      ),
    ]
  }

  fn show_keys(&mut self, ui: &mut Ui, flagged: &[CaptureTarget]) {
    let hint = match self.capturing {
      Some(CaptureTarget::Trigger | CaptureTarget::Actions) => CAPTURE_HINT,
      _ => "Click a box to rebind it",
    };

    card(ui, |ui| {
      card_header(ui, "Keys", hint, |_| {});
      ui.add_space(8.0);
      ui.spacing_mut().item_spacing.x = 12.0;
      ui.columns(2, |cols| {
        self.key_field(&mut cols[0], "Trigger", CaptureTarget::Trigger, flagged);
        self.key_field(&mut cols[1], "Action", CaptureTarget::Actions, flagged);
      });
    });
  }

  fn show_clicking(&mut self, ui: &mut Ui) {
    let idle = self.capturing.is_none();
    card(ui, |ui| {
      card_header(
        ui,
        "Clicking",
        "How and how fast the action key is pressed",
        |_| {},
      );
      ui.add_space(8.0);
      ui.spacing_mut().item_spacing.x = 12.0;
      ui.add_enabled_ui(idle, |ui| {
        ui.columns(2, |cols| {
          cols[0].label(text("Trigger mode", CAPTION, TEXT_MUTED));
          segmented(
            &mut cols[0],
            &mut self.settings.action_key_mode,
            &[
              (
                ActionKeyMode::Hold,
                "Hold",
                "Click while the trigger is held down",
              ),
              (
                ActionKeyMode::Toggle,
                "Toggle",
                "Press the trigger to start, press it again to stop",
              ),
            ],
          );
          cols[1].label(text("Press type", CAPTION, TEXT_MUTED));
          segmented(
            &mut cols[1],
            &mut self.settings.action_press_mode,
            &[
              (
                ActionPressMode::DownUp,
                "Down + Up",
                "Press and release the action key on every click",
              ),
              (
                ActionPressMode::DownOnly,
                "Down only",
                "Only press the action key on every click; it's released when clicking stops",
              ),
            ],
          );
        });

        ui.add_space(12.0);
        value_slider(
          ui,
          "Clicks per second",
          &mut self.settings.cps,
          1..=1000,
          "CPS",
        );
      });
    });
  }

  fn show_burst(&mut self, ui: &mut Ui, flagged: &[CaptureTarget]) {
    let hint = if self.capturing == Some(CaptureTarget::Burst) {
      CAPTURE_HINT
    } else {
      "Tap a key to fire an exact number of clicks"
    };

    let idle = self.capturing.is_none();
    card(ui, |ui| {
      card_header(ui, "Burst", hint, |ui| {
        ui.add_enabled_ui(idle, |ui| {
          toggle_switch(ui, "Burst", &mut self.settings.burst_enabled)
        });
        ui.label(text("\u{2139}", 14.0, TEXT_MUTED))
          .on_hover_text(BURST_HELP);
      });

      if !self.settings.burst_enabled {
        return;
      }

      ui.add_space(8.0);
      self.key_field(ui, "Burst key", CaptureTarget::Burst, flagged);
      ui.add_space(12.0);
      ui.add_enabled_ui(idle, |ui| {
        value_slider(
          ui,
          "Clicks per burst",
          &mut self.settings.burst_clicks,
          1..=100,
          "clicks",
        );
        ui.add_space(8.0);
        value_slider(
          ui,
          "Burst speed",
          &mut self.settings.burst_cps,
          1..=1000,
          "CPS",
        );
      });
    });
  }

  fn key_field(
    &mut self,
    ui: &mut Ui,
    label: &str,
    target: CaptureTarget,
    flagged: &[CaptureTarget],
  ) {
    ui.label(text(label, CAPTION, TEXT_MUTED));

    let capturing = self.capturing == Some(target);
    let name = parse_action(self.token(target)).map(action_name);
    let response = ui
      .add_enabled_ui(capturing || self.capturing.is_none(), |ui| {
        key_chip(
          ui,
          label,
          name.as_deref(),
          capturing,
          flagged.contains(&target),
        )
      })
      .inner;

    if capturing && (response.contains_pointer() || response.is_pointer_button_down_on()) {
      self.capture_hovered = true;
    }
    if response.clicked() {
      self.capturing = if capturing { None } else { Some(target) };
      self.capture_armed = false;
    }
  }

  fn token(&self, target: CaptureTarget) -> &str {
    match target {
      CaptureTarget::Trigger => &self.settings.trigger,
      CaptureTarget::Actions => &self.settings.actions,
      CaptureTarget::Burst => &self.settings.burst_trigger,
    }
  }

  fn poll_capture(&mut self, ctx: &egui::Context) {
    let Some(target) = self.capturing else {
      return;
    };

    // Ignore mouse only while hovering a key box, so clicking it doesn't bind LMB.
    let allow_mouse = !self.capture_hovered;

    // Wait for a full release first so the press that started capture isn't captured.
    if !self.capture_armed {
      if !any_capture_key_down(allow_mouse) {
        self.capture_armed = true;
      }
      ctx.request_repaint_after(Duration::from_millis(16));
      return;
    }

    if let Some(token) = capture_pressed_token(allow_mouse) {
      match target {
        CaptureTarget::Trigger => self.settings.trigger = token,
        CaptureTarget::Actions => self.settings.actions = token,
        CaptureTarget::Burst => self.settings.burst_trigger = token,
      }
      self.capturing = None;
      self.capture_armed = false;
    } else {
      ctx.request_repaint_after(Duration::from_millis(16));
    }
  }

  fn autosave(&mut self, ctx: &egui::Context) {
    if self.settings != self.saved && !ctx.egui_is_using_pointer() {
      settings::save_settings(&self.settings);
      self.saved = self.settings.clone();
    }
  }

  fn validate(&self) -> Result<Keys, Problem> {
    use CaptureTarget::*;
    let s = &self.settings;

    let trigger = parse_action(&s.trigger).ok_or(("Set a trigger key", &[Trigger][..]))?;
    let action = parse_action(&s.actions).ok_or(("Set an action key", &[Actions][..]))?;
    if trigger == action {
      return Err((
        "Trigger and action can't be the same key",
        &[Trigger, Actions],
      ));
    }

    let burst = if s.burst_enabled {
      let burst = parse_action(&s.burst_trigger).ok_or(("Set a burst key", &[Burst][..]))?;
      if burst == action {
        return Err(("Burst key can't be the action key", &[Burst, Actions]));
      }
      if burst == trigger {
        return Err(("Burst key can't be the trigger key", &[Burst, Trigger]));
      }
      Some(burst)
    } else {
      None
    };

    Ok(Keys {
      trigger,
      action,
      burst,
    })
  }

  fn start(&mut self) {
    let Ok(keys) = self.validate() else {
      return;
    };
    let s = &self.settings;

    self.state.stop_spam_worker();
    self.state.reset_session_stats();
    self
      .state
      .configure_trigger(action_to_trigger(keys.trigger), s.action_key_mode);
    self.state.configure_burst(
      keys.burst.map(action_to_trigger),
      if keys.burst.is_some() {
        s.burst_clicks as u64
      } else {
        0
      },
    );

    let press_mode = match s.action_press_mode {
      ActionPressMode::DownUp => PressMode::DownUp,
      ActionPressMode::DownOnly => PressMode::DownOnly,
    };

    spawn_spam_thread(
      self.state.clone(),
      keys.action,
      press_mode,
      s.cps as f64,
      s.burst_cps as f64,
    );

    self.rpc.set_cps_on_start(s.cps);
    self.active = true;
  }

  fn stop(&mut self) {
    self.rpc.set_idling();
    self.state.stop_spam_worker();
    self.state.clear_trigger();
    self.state.reset_session_stats();
    self.active = false;
  }
}

#[cfg(test)]
mod tests;
