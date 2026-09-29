use eframe::egui::{self};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::discord_rpc::DiscordRpc;
use crate::input::{
  action_name, action_to_trigger, any_capture_key_down, capture_pressed_token,
  parse_action,
};
use crate::settings::{self, ActionKeyMode, ActionPressMode, SavedSettings};
use crate::threads::{SharedState, spawn_listener_thread, spawn_spam_thread};
use crate::updater::{self, UpdateState};
use crate::win32::{PressMode, using_send_input_fallback};

fn save_settings(
  trigger: &str,
  actions: &str,
  cps: u32,
  action_key_mode: ActionKeyMode,
  action_press_mode: ActionPressMode,
  burst_enabled: bool,
  burst_trigger: &str,
  burst_clicks: u32,
  burst_cps: u32,
) {
  let settings = SavedSettings {
    trigger: trigger.into(),
    actions: actions.into(),
    cps,
    action_key_mode,
    action_press_mode,
    burst_enabled,
    burst_trigger: burst_trigger.into(),
    burst_clicks,
    burst_cps,
  };
  settings::save_settings(&settings);
}

const BG_DARK: egui::Color32 = egui::Color32::from_rgb(14, 14, 18);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(24, 24, 32);
const BG_CARD_HOVER: egui::Color32 = egui::Color32::from_rgb(30, 30, 40);
const BG_INPUT: egui::Color32 = egui::Color32::from_rgb(18, 18, 24);
const BORDER_SUBTLE: egui::Color32 = egui::Color32::from_rgb(45, 45, 60);
const TEXT_PRIMARY: egui::Color32 = egui::Color32::from_rgb(235, 235, 245);
const TEXT_SECONDARY: egui::Color32 = egui::Color32::from_rgb(140, 140, 165);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(95, 95, 115);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(60, 120, 255);
const ACCENT_ACTIVE: egui::Color32 = egui::Color32::from_rgb(40, 100, 220);
const RED_SOFT: egui::Color32 = egui::Color32::from_rgb(255, 90, 90);
const RED_BTN_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 160, 160);
const STOP_BTN: egui::Color32 = egui::Color32::from_rgb(60, 30, 30);
const SELECTION_BG: egui::Color32 = egui::Color32::from_rgba_premultiplied(40, 80, 200, 60);
const UPDATE_BG: egui::Color32 = egui::Color32::from_rgb(45, 36, 10);
const UPDATE_BORDER: egui::Color32 = egui::Color32::from_rgb(150, 118, 35);
const UPDATE_TEXT: egui::Color32 = egui::Color32::from_rgb(255, 222, 130);
const WARN_AMBER: egui::Color32 = egui::Color32::from_rgb(255, 184, 70);

const WEBSITE_URL: &str = "https://plasmer.top";
const DISCORD_URL: &str = "https://plasmer.top/api/discord";

fn format_elapsed(secs: f64) -> String {
  let total_centis = (secs.max(0.0) * 100.0).round() as u64;
  let hours = total_centis / 360_000;
  let minutes = (total_centis / 6_000) % 60;
  let seconds = (total_centis / 100) % 60;
  let centis = total_centis % 100;

  if hours > 0 {
    format!("{:02}:{:02}:{:02}.{:02}", hours, minutes, seconds, centis)
  } else {
    format!("{:02}:{:02}.{:02}", minutes, seconds, centis)
  }
}

fn card_frame() -> egui::Frame {
  egui::Frame::new()
    .fill(BG_CARD)
    .corner_radius(egui::CornerRadius::same(10))
    .stroke(egui::Stroke::new(1.0, BORDER_SUBTLE))
    .inner_margin(egui::Margin::same(16))
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: &str, value_color: egui::Color32) {
  ui.horizontal(|ui| {
    ui.label(egui::RichText::new(label).size(14.0).color(TEXT_SECONDARY));
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
      ui.label(
        egui::RichText::new(value)
          .size(14.0)
          .color(value_color)
          .strong(),
      );
    });
  });
}

fn accent_button(ui: &mut egui::Ui, text: &str, size: f32) -> bool {
  let btn = egui::Button::new(egui::RichText::new(text).size(size).color(TEXT_PRIMARY))
    .fill(ACCENT)
    .corner_radius(egui::CornerRadius::same(8))
    .min_size(egui::vec2(ui.available_width().min(260.0), 42.0));

  let resp = ui.add(btn);

  if resp.hovered() {
    ui.ctx().output_mut(|o| {
      o.cursor_icon = egui::CursorIcon::PointingHand;
    });
  }

  resp.clicked()
}

fn danger_button(ui: &mut egui::Ui, text: &str) -> bool {
  let btn = egui::Button::new(
    egui::RichText::new(text)
      .size(14.0)
      .color(RED_BTN_TEXT),
  )
  .fill(STOP_BTN)
  .corner_radius(egui::CornerRadius::same(8))
  .min_size(egui::vec2(ui.available_width().min(200.0), 36.0));

  let resp = ui.add(btn);

  if resp.hovered() {
    ui.ctx().output_mut(|o| {
      o.cursor_icon = egui::CursorIcon::PointingHand;
    });
  }

  resp.clicked()
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

pub(crate) struct App {
  state: Arc<SharedState>,
  rpc: DiscordRpc,
  update_state: Arc<UpdateState>,
  app_version: u32,
  trigger_input: String,
  action_input: String,
  cps: u32,
  action_key_mode: ActionKeyMode,
  action_press_mode: ActionPressMode,
  burst_enabled: bool,
  burst_trigger_input: String,
  burst_clicks: u32,
  burst_cps: u32,
  configured: bool,
  trigger_label: String,
  action_label: String,
  burst_label: String,
  error_msg: String,
  capturing: Option<CaptureTarget>,
  capture_armed: bool,
  capture_button_hovered: bool,
  send_input_fallback: bool,
}

impl App {
  pub(crate) fn new(state: Arc<SharedState>, app_version: u32) -> Self {
    spawn_listener_thread(state.clone());

    let s = settings::load_settings();
    let (trigger_input, action_input, cps, action_key_mode, action_press_mode) = (
      s.trigger,
      s.actions,
      s.cps.clamp(1, 1000),
      s.action_key_mode,
      s.action_press_mode,
    );
    let (burst_enabled, burst_trigger_input, burst_clicks, burst_cps) = (
      s.burst_enabled,
      s.burst_trigger,
      s.burst_clicks.clamp(1, 1000),
      s.burst_cps.clamp(1, 1000),
    );

    Self {
      state,
      rpc: DiscordRpc::new(app_version, cps),
      update_state: updater::start_update_check(&app_version.to_string()),
      app_version,
      trigger_input,
      action_input,
      cps,
      action_key_mode,
      action_press_mode,
      burst_enabled,
      burst_trigger_input,
      burst_clicks,
      burst_cps,
      configured: false,
      trigger_label: String::new(),
      action_label: String::new(),
      burst_label: String::new(),
      error_msg: String::new(),
      capturing: None,
      capture_armed: false,
      capture_button_hovered: false,
      send_input_fallback: using_send_input_fallback(),
    }
  }
}

impl eframe::App for App {
  fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
    self.rpc.update();

    let mut style = (*ctx.style()).clone();
    style.override_text_style = Some(egui::TextStyle::Body);
    style
      .text_styles
      .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
    style
      .text_styles
      .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
    style
      .text_styles
      .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));

    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(TEXT_PRIMARY);
    style.visuals.panel_fill = BG_DARK;
    style.visuals.window_fill = BG_DARK;

    style.visuals.widgets.noninteractive.bg_fill = BG_CARD;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT_SECONDARY);

    style.visuals.widgets.inactive.bg_fill = BG_INPUT;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, BORDER_SUBTLE);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);

    style.visuals.widgets.hovered.bg_fill = BG_CARD_HOVER;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

    style.visuals.widgets.active.bg_fill = ACCENT_ACTIVE;
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);

    style.visuals.selection.bg_fill = SELECTION_BG;
    style.visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.tooltip_width = 280.0;
    style.interaction.tooltip_delay = 0.0;

    ctx.set_style(style);

    egui::CentralPanel::default()
      .frame(
        egui::Frame::new()
          .fill(BG_DARK)
          .inner_margin(egui::Margin::same(20)),
      )
      .show(ctx, |ui| {
        ui.add_space(2.0);
        ui.columns(2, |columns| {
          columns[0].vertical(|ui| {
            ui.label(
              egui::RichText::new("Plasmer")
                .size(28.0)
                .color(egui::Color32::WHITE)
                .strong(),
            );
            ui.label(
              egui::RichText::new(format!("v{}", self.app_version))
                .size(11.0)
                .color(TEXT_DIM),
            );
          });

          columns[1].add_space(8.0);
          columns[1].vertical(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
              ui.hyperlink_to("Discord", DISCORD_URL);
              ui.label(egui::RichText::new("•").color(TEXT_DIM));
              ui.hyperlink_to("Website", WEBSITE_URL);
            });
            if self.send_input_fallback {
              ui.add_space(4.0);
              ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                ui.label(
                  egui::RichText::new("\u{26A0}")
                    .size(22.0)
                    .color(WARN_AMBER),
                )
                .on_hover_text("The optimized SendInput could not be loaded");
              });
            }
          });
        });

        self.show_update_notice(ui);
        ui.add_space(12.0);

        if !self.configured {
          self.show_config(ui);
        } else {
          self.show_running(ui);
        }
      });

    self.poll_capture(ctx);

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
  fn show_update_notice(&self, ui: &mut egui::Ui) {
    let Some(update) = self.update_state.available_update() else {
      return;
    };

    egui::Frame::new()
      .fill(UPDATE_BG)
      .corner_radius(egui::CornerRadius::same(8))
      .stroke(egui::Stroke::new(1.0, UPDATE_BORDER))
      .inner_margin(egui::Margin::same(10))
      .show(ui, |ui| {
        ui.vertical_centered(|ui| {
          ui.label(
            egui::RichText::new(format!("Update available: v{}", update.latest_version))
              .size(13.0)
              .color(UPDATE_TEXT)
              .strong(),
          );

          ui.hyperlink_to("Open update page", &update.download_url);
        });
      });
  }

  fn show_config(&mut self, ui: &mut egui::Ui) {
    self.capture_button_hovered = false;
    egui::ScrollArea::vertical()
      .auto_shrink([false, true])
      .show(ui, |ui| {
        self.show_config_inner(ui);
      });
  }

  fn show_config_inner(&mut self, ui: &mut egui::Ui) {
    card_frame().show(ui, |ui| {
      ui.label(
        egui::RichText::new("Configuration")
          .size(15.0)
          .color(TEXT_PRIMARY)
          .strong(),
      );
      ui.add_space(8.0);

      ui.label(
        egui::RichText::new("Trigger key")
          .size(12.0)
          .color(TEXT_SECONDARY),
      );
      ui.add_space(2.0);
      self.bindable_input_row(ui, CaptureTarget::Trigger);

      ui.add_space(8.0);

      ui.label(
        egui::RichText::new("Action key")
          .size(12.0)
          .color(TEXT_SECONDARY),
      );
      ui.add_space(2.0);
      self.bindable_input_row(ui, CaptureTarget::Actions);

      ui.add_space(8.0);

      ui.label(
        egui::RichText::new("Action key mode")
          .size(12.0)
          .color(TEXT_SECONDARY),
      );
      ui.add_space(2.0);
      ui.horizontal(|ui| {
        ui.selectable_value(&mut self.action_key_mode, ActionKeyMode::Hold, "Hold");
        ui.selectable_value(&mut self.action_key_mode, ActionKeyMode::Toggle, "Toggle");
      });

      ui.add_space(8.0);

      ui.label(
        egui::RichText::new("Action press mode")
          .size(12.0)
          .color(TEXT_SECONDARY),
      );
      ui.add_space(2.0);
      ui.horizontal(|ui| {
        ui.selectable_value(
          &mut self.action_press_mode,
          ActionPressMode::DownUp,
          "Down + Up",
        );
        ui.selectable_value(
          &mut self.action_press_mode,
          ActionPressMode::DownOnly,
          "Down only",
        );
      });

      ui.add_space(10.0);

      ui.label(
        egui::RichText::new("Clicks per second")
          .size(12.0)
          .color(TEXT_SECONDARY),
      );
      ui.add_space(2.0);
      ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = ui.available_width() - 80.0;
        ui.add(egui::Slider::new(&mut self.cps, 1..=1000).show_value(false));
        let (rect, _) = ui.allocate_exact_size(
          egui::vec2(60.0, ui.spacing().interact_size.y),
          egui::Sense::hover(),
        );
        ui.painter().text(
          rect.right_center(),
          egui::Align2::RIGHT_CENTER,
          format!("{} cps", self.cps),
          egui::FontId::proportional(14.0),
          TEXT_PRIMARY,
        );
      });

      ui.add_space(10.0);
      ui.separator();
      ui.add_space(8.0);

      ui.horizontal(|ui| {
        ui.label(
          egui::RichText::new("Burst mode")
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.label(
          egui::RichText::new("\u{2139}")
            .size(13.0)
            .color(ACCENT),
        )
        .on_hover_ui(|ui| {
          ui.set_max_width(240.0);
          ui.label(
            egui::RichText::new(
              "One-shot burst hotkey.\n\n\
               Tap the burst trigger key once: Plasmer fires exactly the set number \
               of clicks at the burst CPS — same high-precision timing as \
               normal spam — then stops on its own.\n\n\
               While a burst is running the key is locked out; it re-arms \
               after the last click. The normal trigger keeps working \
               independently.",
            )
            .size(12.0)
            .color(TEXT_PRIMARY),
          );
        });
      });
      ui.add_space(2.0);
      ui.horizontal(|ui| {
        ui.selectable_value(&mut self.burst_enabled, false, "Off");
        ui.selectable_value(&mut self.burst_enabled, true, "On");
      });

      if self.burst_enabled {
        ui.add_space(8.0);

        ui.label(
          egui::RichText::new("Burst trigger key")
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(2.0);
        self.bindable_input_row(ui, CaptureTarget::Burst);

        ui.add_space(8.0);

        ui.label(
          egui::RichText::new("Clicks per burst")
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(2.0);
        ui.horizontal(|ui| {
          ui.spacing_mut().slider_width = ui.available_width() - 80.0;
          ui.add(egui::Slider::new(&mut self.burst_clicks, 1..=100).show_value(false));
          let (rect, _) = ui.allocate_exact_size(
            egui::vec2(60.0, ui.spacing().interact_size.y),
            egui::Sense::hover(),
          );
          ui.painter().text(
            rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            format!("{} clicks", self.burst_clicks),
            egui::FontId::proportional(14.0),
            TEXT_PRIMARY,
          );
        });

        ui.add_space(8.0);

        ui.label(
          egui::RichText::new("Burst clicks per second")
            .size(12.0)
            .color(TEXT_SECONDARY),
        );
        ui.add_space(2.0);
        ui.horizontal(|ui| {
          ui.spacing_mut().slider_width = ui.available_width() - 80.0;
          ui.add(egui::Slider::new(&mut self.burst_cps, 1..=1000).show_value(false));
          let (rect, _) = ui.allocate_exact_size(
            egui::vec2(60.0, ui.spacing().interact_size.y),
            egui::Sense::hover(),
          );
          ui.painter().text(
            rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            format!("{} cps", self.burst_cps),
            egui::FontId::proportional(14.0),
            TEXT_PRIMARY,
          );
        });
      }
    });

    ui.add_space(10.0);

    if !self.error_msg.is_empty() {
      ui.vertical_centered(|ui| {
        ui.label(
          egui::RichText::new(&self.error_msg)
            .size(13.0)
            .color(RED_SOFT),
        );
      });
      ui.add_space(4.0);
    }

    ui.vertical_centered(|ui| {
      if accent_button(ui, "▶  Start", 16.0) {
        self.try_start();
      }
    });
  }

  fn show_running(&mut self, ui: &mut egui::Ui) {
    let running = self.state.running.load(Ordering::Relaxed);
    let clicks = self.state.click_count.load(Ordering::Relaxed);
    let cps = f64::from_bits(self.state.actual_cps.load(Ordering::Relaxed));
    let elapsed_secs = f64::from_bits(self.state.last_elapsed_secs.load(Ordering::Relaxed));
    let jitter = f64::from_bits(self.state.last_jitter_ms.load(Ordering::Relaxed));
    let consistency = f64::from_bits(self.state.last_consistency.load(Ordering::Relaxed));

    card_frame().show(ui, |ui| {
      stat_row(ui, "Trigger", &self.trigger_label, ACCENT);
      ui.add_space(2.0);
      stat_row(
        ui,
        "Action Key Mode",
        match self.action_key_mode {
          ActionKeyMode::Hold => "Hold",
          ActionKeyMode::Toggle => "Toggle",
        },
        ACCENT,
      );
      ui.add_space(2.0);
      stat_row(
        ui,
        "Action Press Mode",
        match self.action_press_mode {
          ActionPressMode::DownUp => "Down + Up",
          ActionPressMode::DownOnly => "Down only",
        },
        ACCENT,
      );
      ui.add_space(2.0);
      stat_row(ui, "Action", &self.action_label, ACCENT);
      ui.add_space(2.0);
      stat_row(ui, "Target CPS", &format!("{}", self.cps), ACCENT);
      if self.burst_enabled {
        ui.add_space(2.0);
        stat_row(ui, "Burst Trigger Key", &self.burst_label, ACCENT);
        ui.add_space(2.0);
        stat_row(ui, "Burst Clicks", &format!("{}", self.burst_clicks), ACCENT);
        ui.add_space(2.0);
        stat_row(ui, "Burst CPS", &format!("{}", self.burst_cps), ACCENT);
      }
    });

    ui.add_space(6.0);

    card_frame().show(ui, |ui| {
      ui.label(
        egui::RichText::new("Session Stats")
          .size(13.0)
          .color(TEXT_DIM),
      );
      ui.add_space(6.0);

      let dash = "—".to_string();

      let clicks_text = if running {
        dash.clone()
      } else {
        format!("{}", clicks)
      };
      let cps_text = if running {
        dash.clone()
      } else {
        format!("{:.1}", cps)
      };
      let elapsed_text = if running {
        dash.clone()
      } else {
        format_elapsed(elapsed_secs)
      };
      let jitter_text = if running {
        dash.clone()
      } else {
        format!("{:.3} ms", jitter)
      };
      let consistency_text = if running {
        dash
      } else {
        format!("{:.2}%", consistency)
      };

      stat_row(ui, "Clicks", &clicks_text, TEXT_PRIMARY);
      ui.add_space(2.0);
      stat_row(ui, "Actual CPS", &cps_text, TEXT_PRIMARY);
      ui.add_space(2.0);
      stat_row(ui, "Elapsed", &elapsed_text, TEXT_PRIMARY);
      ui.add_space(2.0);
      stat_row(ui, "Jitter", &jitter_text, TEXT_PRIMARY);
      ui.add_space(2.0);
      stat_row(ui, "Consistency", &consistency_text, TEXT_PRIMARY);
    });

    ui.add_space(10.0);

    ui.vertical_centered(|ui| {
      if danger_button(ui, "■  Stop & Reset") {
        self.rpc.set_idling();
        self.state.stop_spam_worker();
        self.state.clear_trigger();
        self.state.reset_session_stats();
        self.configured = false;
        self.trigger_label.clear();
        self.action_label.clear();
        self.burst_label.clear();
      }
    });

    ui.add_space(6.0);

    ui.vertical_centered(|ui| {
      ui.label(
        egui::RichText::new(match self.action_key_mode {
          ActionKeyMode::Hold => "Hold trigger key to spam",
          ActionKeyMode::Toggle => "Press trigger key to toggle spam",
        })
        .size(11.0)
        .color(TEXT_DIM),
      );
      if self.burst_enabled {
        ui.label(
          egui::RichText::new(format!(
            "Tap burst trigger key to fire {} clicks",
            self.burst_clicks
          ))
          .size(11.0)
          .color(TEXT_DIM),
        );
      }
    });
  }

  fn bindable_input_row(&mut self, ui: &mut egui::Ui, target: CaptureTarget) {
    let is_capturing = self.capturing == Some(target);

    let row_height = 28.0;
    ui.allocate_ui_with_layout(
      egui::vec2(ui.available_width(), row_height),
      egui::Layout::right_to_left(egui::Align::Center),
      |ui| {
        let btn_text = if is_capturing { "..." } else { "Capture" };
        let bind_btn = egui::Button::new(
          egui::RichText::new(btn_text)
            .size(13.0)
            .color(TEXT_PRIMARY),
        )
        .fill(if is_capturing { ACCENT_ACTIVE } else { ACCENT })
        .corner_radius(egui::CornerRadius::same(6))
        .min_size(egui::vec2(70.0, 28.0));

        let bind_resp = ui.add(bind_btn);
        if bind_resp.hovered() {
          ui.ctx().output_mut(|o| {
            o.cursor_icon = egui::CursorIcon::PointingHand;
          });
        }
        if bind_resp.contains_pointer() || bind_resp.is_pointer_button_down_on() {
          self.capture_button_hovered = true;
        }
        if bind_resp.clicked() && self.capturing.is_none() {
          self.capturing = Some(target);
          self.capture_armed = false;
        }

        let buf = match target {
          CaptureTarget::Trigger => &self.trigger_input,
          CaptureTarget::Actions => &self.action_input,
          CaptureTarget::Burst => &self.burst_trigger_input,
        };
        let display_name = parse_action(buf).map(action_name);

        let chip_width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(
          egui::vec2(chip_width, row_height),
          egui::Sense::hover(),
        );
        ui.painter().rect(
          rect,
          6.0,
          BG_INPUT,
          egui::Stroke::new(1.0, BORDER_SUBTLE),
          egui::StrokeKind::Inside,
        );
        if !is_capturing {
          if let Some(name) = display_name {
            let text_pos = rect.left_center() + egui::vec2(10.0, 0.0);
            ui.painter().text(
              text_pos,
              egui::Align2::LEFT_CENTER,
              name,
              egui::FontId::proportional(13.0),
              TEXT_PRIMARY,
            );
          }
        }
      },
    );
  }

  fn poll_capture(&mut self, ctx: &egui::Context) {
    let Some(target) = self.capturing else {
      return;
    };

    // Ignore mouse only while hovering the Capture button, so clicking it doesn't bind LMB.
    let allow_mouse = !self.capture_button_hovered;

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
        CaptureTarget::Trigger => self.trigger_input = token,
        CaptureTarget::Actions => self.action_input = token,
        CaptureTarget::Burst => self.burst_trigger_input = token,
      }
      self.capturing = None;
      self.capture_armed = false;
    } else {
      ctx.request_repaint_after(Duration::from_millis(16));
    }
  }

  fn try_start(&mut self) {
    self.error_msg.clear();

    let trigger_action = parse_action(&self.trigger_input);
    let action = parse_action(&self.action_input);

    let (trigger_action, action) = match (trigger_action, action) {
      (None, _) => {
        self.error_msg = "Capture a trigger key first.".into();
        return;
      }
      (_, None) => {
        self.error_msg = "Capture an action key first.".into();
        return;
      }
      (Some(t), Some(a)) => (t, a),
    };

    if action == trigger_action {
      self.error_msg =
        "Trigger key cannot also be the action. Use a different trigger for stable timing.".into();
      return;
    }

    let burst_trigger = if self.burst_enabled {
      let Some(burst_action) = parse_action(&self.burst_trigger_input) else {
        self.error_msg = "Capture a burst trigger key first.".into();
        return;
      };
      if burst_action == action {
        self.error_msg = "Burst trigger key cannot also be the action key.".into();
        return;
      }
      if burst_action == trigger_action {
        self.error_msg = "Burst trigger key cannot be the same as the trigger key.".into();
        return;
      }
      self.burst_label = action_name(burst_action);
      Some(action_to_trigger(burst_action))
    } else {
      self.burst_label.clear();
      None
    };

    let trigger = action_to_trigger(trigger_action);

    self.trigger_label = action_name(trigger_action);
    self.action_label = action_name(action);

    self.state.stop_spam_worker();
    self.state.reset_session_stats();
    self.state.configure_trigger(trigger, self.action_key_mode);
    self.state.configure_burst(
      burst_trigger,
      if self.burst_enabled {
        self.burst_clicks as u64
      } else {
        0
      },
    );

    let press_mode = match self.action_press_mode {
      ActionPressMode::DownUp => PressMode::DownUp,
      ActionPressMode::DownOnly => PressMode::DownOnly,
    };

    spawn_spam_thread(
      self.state.clone(),
      action,
      press_mode,
      self.cps as f64,
      self.burst_cps as f64,
    );

    save_settings(
      &self.trigger_input,
      &self.action_input,
      self.cps,
      self.action_key_mode,
      self.action_press_mode,
      self.burst_enabled,
      &self.burst_trigger_input,
      self.burst_clicks,
      self.burst_cps,
    );

    self.configured = true;
    self.capturing = None;
    self.rpc.set_cps_on_start(self.cps);
  }
}
