use super::*;
use crate::updater::{AvailableUpdate, fixed_update_state};
use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

type TestApp<'a> = Harness<'a, App>;

fn launch(settings: SavedSettings) -> TestApp<'static> {
  launch_with_update(settings, None)
}

fn launch_with_update(
  settings: SavedSettings,
  update: Option<AvailableUpdate>,
) -> TestApp<'static> {
  let mut app = Harness::builder()
    .with_size(vec2(470.0, 648.0))
    .build_eframe(|cc| {
      App::with_parts(
        &cc.egui_ctx,
        Arc::new(SharedState::new()),
        22,
        settings,
        DiscordRpc::disabled(),
        fixed_update_state(update),
      )
    });
  settle(&mut app);
  app
}

// The app repaints on a timer, so `run()` would never settle.
fn settle(app: &mut TestApp) {
  app.run_steps(10);
}

// Hover first: kittest's one-frame move+click can be swallowed by an instant tooltip.
fn click(app: &mut TestApp, role: Role, label: &str) {
  app.get_by_role_and_label(role, label).hover();
  app.step();
  app.get_by_role_and_label(role, label).click();
  settle(app);
}

fn enabled(app: &TestApp, role: Role, label: &str) -> bool {
  !app
    .get_by_role_and_label(role, label)
    .accesskit_node()
    .is_disabled()
}

fn stat(app: &TestApp, label: &str) -> String {
  let prefix = format!("{label}: ");
  let text = app
    .get_by_label_contains(&prefix)
    .accesskit_node()
    .value()
    .expect("stats tile has text");
  text[prefix.len()..].to_owned()
}

#[test]
fn start_is_ready_with_valid_keys() {
  let app = launch(SavedSettings::default());

  assert!(enabled(&app, Role::Button, "Start"));
  app.get_by_label("Stopped");
}

#[test]
fn conflicting_keys_block_start_and_say_why() {
  let app = launch(SavedSettings {
    actions: "x".into(),
    ..SavedSettings::default()
  });

  assert!(!enabled(&app, Role::Button, "Start"));
  app.get_by_label_contains("Trigger and action can't be the same key");
}

#[test]
fn burst_key_conflict_blocks_start() {
  let app = launch(SavedSettings {
    burst_enabled: true,
    burst_trigger: "x".into(),
    ..SavedSettings::default()
  });

  assert!(!enabled(&app, Role::Button, "Start"));
  app.get_by_label_contains("Burst key can't be the trigger key");
}

#[test]
fn start_locks_settings_and_stop_unlocks_them() {
  let mut app = launch(SavedSettings::default());

  click(&mut app, Role::Button, "Start");
  app.get_by_label_contains("Running  ·");
  assert!(!enabled(&app, Role::Button, "Trigger"));
  assert!(!enabled(&app, Role::RadioButton, "Toggle"));
  assert!(!enabled(&app, Role::Slider, "Clicks per second"));

  click(&mut app, Role::Button, "Stop");
  app.get_by_label("Stopped");
  assert!(enabled(&app, Role::Button, "Trigger"));
  assert!(enabled(&app, Role::RadioButton, "Toggle"));
  assert!(enabled(&app, Role::Slider, "Clicks per second"));
}

#[test]
fn status_follows_the_clicker() {
  let mut app = launch(SavedSettings::default());
  click(&mut app, Role::Button, "Start");

  app.state().state.running.store(true, Ordering::Relaxed);
  settle(&mut app);
  app.get_by_label_contains("Clicking  ·");

  app.state().state.running.store(false, Ordering::Relaxed);
  settle(&mut app);
  app.get_by_label_contains("Running  ·");
}

#[test]
fn stats_show_the_finished_session() {
  let mut app = launch(SavedSettings::default());
  click(&mut app, Role::Button, "Start");
  assert_eq!(stat(&app, "Clicks"), "—");

  let state = &app.state().state;
  let store = |stat: &AtomicU64, value: f64| stat.store(value.to_bits(), Ordering::Relaxed);
  state.click_count.store(1234, Ordering::Relaxed);
  store(&state.actual_cps, 99.8);
  store(&state.last_elapsed_secs, 12.37);
  store(&state.last_jitter_ms, 0.012);
  store(&state.last_consistency, 99.87);
  settle(&mut app);

  assert_eq!(stat(&app, "Clicks"), "1234");
  assert_eq!(stat(&app, "CPS"), "99.8");
  assert_eq!(stat(&app, "Time"), "00:12.37");
  assert_eq!(stat(&app, "Jitter"), "0.012 ms");
  assert_eq!(stat(&app, "Consistency"), "99.87%");
}

#[test]
fn segments_select_exactly_one_option() {
  let mut app = launch(SavedSettings::default());

  click(&mut app, Role::RadioButton, "Toggle");
  let toggled = |label| {
    app
      .get_by_role_and_label(Role::RadioButton, label)
      .accesskit_node()
      .toggled()
  };
  assert_eq!(toggled("Toggle"), Some(Toggled::True));
  assert_eq!(toggled("Hold"), Some(Toggled::False));
  assert!(app.state().settings.action_key_mode == ActionKeyMode::Toggle);

  click(&mut app, Role::RadioButton, "Down only");
  assert!(app.state().settings.action_press_mode == ActionPressMode::DownOnly);
}

#[test]
fn burst_switch_reveals_burst_settings() {
  let mut app = launch(SavedSettings::default());
  assert!(
    app
      .query_by_role_and_label(Role::Button, "Burst key")
      .is_none()
  );

  click(&mut app, Role::CheckBox, "Burst");
  app.get_by_role_and_label(Role::Button, "Burst key");
  app.get_by_role_and_label(Role::Slider, "Clicks per burst");
  app.get_by_role_and_label(Role::Slider, "Burst speed");
  assert!(app.state().settings.burst_enabled);
}

#[test]
fn cps_slider_is_linear() {
  let mut app = launch(SavedSettings::default());

  click(&mut app, Role::Slider, "Clicks per second");
  assert_eq!(app.state().settings.cps, 501);
}

#[test]
fn key_capture_is_modal_and_cancellable() {
  let mut app = launch(SavedSettings::default());

  click(&mut app, Role::Button, "Trigger");
  let trigger_text = |app: &TestApp| {
    app
      .get_by_role_and_label(Role::Button, "Trigger")
      .accesskit_node()
      .value()
  };
  assert_eq!(trigger_text(&app).as_deref(), Some("Press a key…"));
  app.get_by_label(CAPTURE_HINT);
  assert!(!enabled(&app, Role::Button, "Start"));
  assert!(!enabled(&app, Role::Button, "Action"));
  assert!(!enabled(&app, Role::RadioButton, "Toggle"));

  click(&mut app, Role::Button, "Trigger");
  assert_eq!(trigger_text(&app).as_deref(), Some("X"));
  assert!(enabled(&app, Role::Button, "Start"));
}

#[test]
fn update_banner_offers_the_download() {
  let app = launch_with_update(
    SavedSettings::default(),
    Some(AvailableUpdate {
      latest_version: "23".into(),
      download_url: "https://plasmer.top/download".into(),
    }),
  );

  app.get_by_label("Update available: v23");
  // Hyperlinks report as labels.
  app.get_by_label("Download");
}

#[test]
fn elapsed_time_format() {
  assert_eq!(format_elapsed(0.0), "00:00.00");
  assert_eq!(format_elapsed(12.37), "00:12.37");
  assert_eq!(format_elapsed(3723.0), "1:02:03");
}

#[test]
fn start_and_stop_keep_widget_ids() {
  // egui's debug outline for a widget whose id changed in place.
  let red_outlines = |app: &TestApp| {
    app
      .output()
      .shapes
      .iter()
      .filter(|c| matches!(&c.shape, egui::Shape::Rect(r) if r.stroke.color == Color32::RED))
      .count()
  };
  let mut app = launch(SavedSettings::default());

  for label in ["Start", "Stop"] {
    app.get_by_role_and_label(Role::Button, label).hover();
    app.step();
    app.get_by_role_and_label(Role::Button, label).click();
    for _ in 0..5 {
      app.step();
      assert_eq!(red_outlines(&app), 0, "a widget changed id after {label}");
    }
  }
}

#[test]
fn keyboard_cannot_press_stop() {
  let mut app = launch(SavedSettings::default());
  click(&mut app, Role::Button, "Start");

  let mut press = |key| {
    for pressed in [true, false] {
      app.event(egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
      });
      app.step();
    }
    settle(&mut app);
    assert!(app.state().active, "{key:?} stopped the clicker");
  };
  for key in [
    egui::Key::Tab,
    egui::Key::Space,
    egui::Key::Tab,
    egui::Key::Enter,
  ] {
    press(key);
  }
}

#[test]
fn graph_shows_the_last_session() {
  let mut app = launch(SavedSettings::default());
  assert!(app.query_by_label("Last session graph").is_none());

  *app.state().state.last_session.lock().unwrap() = (10.0, vec![9.9, 10.1, 10.0, 12.5]);
  settle(&mut app);
  click(&mut app, Role::Button, "Last session graph");

  assert_eq!(stat(&app, "Clicks"), "5");
  assert_eq!(stat(&app, "Target"), "10.000 ms");
  assert_eq!(stat(&app, "Max"), "12.500 ms");
}

#[test]
fn interval_buckets_take_min_mean_max() {
  assert_eq!(
    interval_buckets(&[1.0, 3.0, 2.0, 2.0, 5.0], 2),
    vec![(1.0, 2.0, 3.0), (2.0, 2.0, 2.0), (5.0, 5.0, 5.0)]
  );
}
