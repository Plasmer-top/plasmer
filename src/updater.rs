use serde::Deserialize;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug)]
pub(crate) struct AvailableUpdate {
  pub(crate) latest_version: String,
  pub(crate) download_url: String,
}

#[derive(Default)]
struct UpdateStateInner {
  available: Option<AvailableUpdate>,
}

pub(crate) struct UpdateState {
  inner: Mutex<UpdateStateInner>,
}

impl UpdateState {
  fn new() -> Self {
    Self {
      inner: Mutex::new(UpdateStateInner::default()),
    }
  }

  fn set_available(&self, update: AvailableUpdate) {
    if let Ok(mut guard) = self.inner.lock() {
      guard.available = Some(update);
    }
  }

  pub(crate) fn available_update(&self) -> Option<AvailableUpdate> {
    self.inner.lock().ok().and_then(|g| g.available.clone())
  }
}

const UPDATE_CHECK_URL: &str = "https://plasmer.top/api/update/latest";
const SITE_URL: &str = "https://plasmer.top";

#[derive(Deserialize)]
struct UpdateApiResponse {
  update_available: Option<bool>,
  latest_version: Option<String>,
  download_url: Option<String>,
}

pub(crate) fn start_update_check(current_version: &str) -> Arc<UpdateState> {
  let state = Arc::new(UpdateState::new());
  let state_for_thread = Arc::clone(&state);
  let current_version = current_version.to_string();

  thread::Builder::new()
    .name("update-check".into())
    .spawn(move || {
      let response = match ureq::get(UPDATE_CHECK_URL).call() {
        Ok(response) => response,
        Err(_) => return,
      };

      let mut body = response.into_body();
      let text = match body.read_to_string() {
        Ok(text) => text,
        Err(_) => return,
      };

      let api = match serde_json::from_str::<UpdateApiResponse>(&text) {
        Ok(api) => api,
        Err(_) => return,
      };

      let latest_version = api
        .latest_version
        .unwrap_or_else(|| current_version.clone())
        .trim()
        .to_string();

      if latest_version.is_empty() {
        return;
      }

      let has_newer_version = latest_version != current_version;
      if !has_newer_version {
        return;
      }

      if let Some(false) = api.update_available {
        return;
      }

      let download_url = api
        .download_url
        .filter(|url| url.starts_with(SITE_URL))
        .unwrap_or_else(|| SITE_URL.to_string());

      state_for_thread.set_available(AvailableUpdate {
        latest_version,
        download_url,
      });
    })
    .ok();

  state
}
