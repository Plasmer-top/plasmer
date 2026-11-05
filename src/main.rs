#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod discord_rpc;
mod gui;
mod input;
mod settings;
mod threads;
mod updater;
mod win32;

use eframe::egui;
use std::sync::Arc;

use config::{APP_NAME, APP_VERSION};
use gui::{App, load_icon};
use threads::SharedState;
use win32::{CpuLayout, TimerGuard, elevate_process, init_low_fns};

fn main() -> eframe::Result<()> {
  init_low_fns();

  let _timer_guard = unsafe { TimerGuard::install() };

  let cpu = CpuLayout::detect();
  unsafe { elevate_process(cpu.other_mask) };

  let state = Arc::new(SharedState::new());

  let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
      .with_inner_size([470.0, 648.0])
      .with_min_inner_size([470.0, 648.0])
      .with_icon(load_icon()),
    ..Default::default()
  };

  eframe::run_native(
    APP_NAME,
    options,
    Box::new(move |cc| Ok(Box::new(App::new(cc, state, APP_VERSION)))),
  )
}
