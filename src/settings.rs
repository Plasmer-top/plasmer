use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ActionKeyMode {
  Hold,
  Toggle,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActionPressMode {
  DownUp,
  DownOnly,
}

fn default_action_key_mode() -> ActionKeyMode {
  ActionKeyMode::Hold
}

fn default_action_press_mode() -> ActionPressMode {
  ActionPressMode::DownUp
}

fn default_burst_enabled() -> bool {
  false
}

fn default_burst_trigger() -> String {
  "v".to_string()
}

fn default_burst_clicks() -> u32 {
  10
}

fn default_burst_cps() -> u32 {
  20
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct SavedSettings {
  #[serde(default = "default_trigger")]
  pub(crate) trigger: String,
  #[serde(default = "default_actions")]
  pub(crate) actions: String,
  #[serde(default = "default_cps")]
  pub(crate) cps: u32,
  #[serde(default = "default_action_key_mode")]
  pub(crate) action_key_mode: ActionKeyMode,
  #[serde(default = "default_action_press_mode")]
  pub(crate) action_press_mode: ActionPressMode,
  #[serde(default = "default_burst_enabled")]
  pub(crate) burst_enabled: bool,
  #[serde(default = "default_burst_trigger")]
  pub(crate) burst_trigger: String,
  #[serde(default = "default_burst_clicks")]
  pub(crate) burst_clicks: u32,
  #[serde(default = "default_burst_cps")]
  pub(crate) burst_cps: u32,
}

fn default_trigger() -> String {
  "x".to_string()
}

fn default_actions() -> String {
  "f".to_string()
}

fn default_cps() -> u32 {
  20
}

impl Default for SavedSettings {
  fn default() -> Self {
    Self {
      trigger: default_trigger(),
      actions: default_actions(),
      cps: default_cps(),
      action_key_mode: default_action_key_mode(),
      action_press_mode: default_action_press_mode(),
      burst_enabled: default_burst_enabled(),
      burst_trigger: default_burst_trigger(),
      burst_clicks: default_burst_clicks(),
      burst_cps: default_burst_cps(),
    }
  }
}

pub(crate) fn config_dir() -> std::path::PathBuf {
  let dir = dirs::config_dir()
    .unwrap_or_else(|| std::path::PathBuf::from("."))
    .join("plasmer-ac");

  std::fs::create_dir_all(&dir).ok();
  dir
}

pub(crate) fn settings_path() -> std::path::PathBuf {
  config_dir().join("settings.json")
}

pub(crate) fn load_settings() -> SavedSettings {
  let path = settings_path();
  let data = match std::fs::read_to_string(path) {
    Ok(data) => data,
    Err(_) => return SavedSettings::default(),
  };

  let json = match serde_json::from_str::<serde_json::Value>(&data) {
    Ok(json) => json,
    Err(_) => return SavedSettings::default(),
  };

  let default = SavedSettings::default();

  let trigger = json
    .get("trigger")
    .and_then(serde_json::Value::as_str)
    .map(str::to_owned)
    .unwrap_or(default.trigger);

  let actions = json
    .get("actions")
    .and_then(serde_json::Value::as_str)
    .and_then(|s| s.split_whitespace().next())
    .map(str::to_owned)
    .unwrap_or(default.actions);

  let cps = json
    .get("cps")
    .and_then(serde_json::Value::as_u64)
    .and_then(|v| u32::try_from(v).ok())
    .unwrap_or(default.cps);

  let action_key_mode = match json
    .get("action_key_mode")
    .and_then(serde_json::Value::as_str)
  {
    Some("hold") => ActionKeyMode::Hold,
    Some("toggle") => ActionKeyMode::Toggle,
    _ => default.action_key_mode,
  };

  let action_press_mode = match json
    .get("action_press_mode")
    .and_then(serde_json::Value::as_str)
  {
    Some("down_up") => ActionPressMode::DownUp,
    Some("down_only") => ActionPressMode::DownOnly,
    _ => default.action_press_mode,
  };

  let burst_enabled = json
    .get("burst_enabled")
    .and_then(serde_json::Value::as_bool)
    .unwrap_or(default.burst_enabled);

  let burst_trigger = json
    .get("burst_trigger")
    .and_then(serde_json::Value::as_str)
    .and_then(|s| s.split_whitespace().next())
    .map(str::to_owned)
    .unwrap_or(default.burst_trigger);

  let burst_clicks = json
    .get("burst_clicks")
    .and_then(serde_json::Value::as_u64)
    .and_then(|v| u32::try_from(v).ok())
    .unwrap_or(default.burst_clicks);

  let burst_cps = json
    .get("burst_cps")
    .and_then(serde_json::Value::as_u64)
    .and_then(|v| u32::try_from(v).ok())
    .unwrap_or(default.burst_cps);

  SavedSettings {
    trigger,
    actions,
    cps,
    action_key_mode,
    action_press_mode,
    burst_enabled,
    burst_trigger,
    burst_clicks,
    burst_cps,
  }
}

pub(crate) fn save_settings(settings: &SavedSettings) {
  if let Ok(json) = serde_json::to_string_pretty(settings) {
    std::fs::write(settings_path(), json).ok();
  }
}
