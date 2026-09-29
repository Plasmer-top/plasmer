use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;

use crate::input::TriggerKind;
use crate::settings::ActionKeyMode;
use crate::win32::{
  CpuLayout, PressMode, SpamAction, TimingCtx, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_XBUTTON1,
  VK_XBUTTON2, async_key_down, build_input_batch, build_release_batch, cpu_pause,
  elevate_control_thread, elevate_spam_thread, nt_sleep_100ns, prefetch_batch, qpc, rdtsc_now,
  rdtscp_now, send_batch,
};

const CONTROL_SLEEP_IDLE_100NS: i64 = 10_000;
const CONTROL_SLEEP_ACTIVE_100NS: i64 = 5_000;

pub(crate) struct SharedState {
  pub(crate) running: AtomicBool,
  pub(crate) wake: AtomicBool,
  pub(crate) spam_shutdown: AtomicBool,
  pub(crate) app_shutdown: AtomicBool,
  pub(crate) click_count: AtomicU64,
  pub(crate) actual_cps: AtomicU64,
  pub(crate) last_elapsed_secs: AtomicU64,
  pub(crate) last_jitter_ms: AtomicU64,
  pub(crate) last_consistency: AtomicU64,
  pub(crate) listener_started: AtomicBool,
  pub(crate) spam_thread: Mutex<Option<thread::Thread>>,
  pub(crate) spam_join: Mutex<Option<JoinHandle<()>>>,
  pub(crate) listener_join: Mutex<Option<JoinHandle<()>>>,
  pub(crate) trigger: Mutex<Option<TriggerKind>>,
  pub(crate) action_key_mode: Mutex<ActionKeyMode>,
  pub(crate) burst_trigger: Mutex<Option<TriggerKind>>,
  pub(crate) burst_clicks: AtomicU64,
  pub(crate) burst_active: AtomicBool,
  pub(crate) session_click_limit: AtomicU64,
  pub(crate) session_first_fire_qpc: AtomicI64,
  pub(crate) session_last_fire_qpc: AtomicI64,
  pub(crate) session_interval_qpc: AtomicI64,
}

impl SharedState {
  pub(crate) fn new() -> Self {
    Self {
      running: AtomicBool::new(false),
      wake: AtomicBool::new(false),
      spam_shutdown: AtomicBool::new(false),
      app_shutdown: AtomicBool::new(false),
      click_count: AtomicU64::new(0),
      actual_cps: AtomicU64::new(0u64),
      last_elapsed_secs: AtomicU64::new(0u64),
      last_jitter_ms: AtomicU64::new(0u64),
      last_consistency: AtomicU64::new(0u64),
      listener_started: AtomicBool::new(false),
      spam_thread: Mutex::new(None),
      spam_join: Mutex::new(None),
      listener_join: Mutex::new(None),
      trigger: Mutex::new(None),
      action_key_mode: Mutex::new(ActionKeyMode::Hold),
      burst_trigger: Mutex::new(None),
      burst_clicks: AtomicU64::new(0),
      burst_active: AtomicBool::new(false),
      session_click_limit: AtomicU64::new(0),
      session_first_fire_qpc: AtomicI64::new(0),
      session_last_fire_qpc: AtomicI64::new(0),
      session_interval_qpc: AtomicI64::new(0),
    }
  }

  pub(crate) fn reset_session_stats(&self) {
    self.click_count.store(0, Ordering::Relaxed);
    self.actual_cps.store(0f64.to_bits(), Ordering::Relaxed);
    self
      .last_elapsed_secs
      .store(0f64.to_bits(), Ordering::Relaxed);
    self.last_jitter_ms.store(0f64.to_bits(), Ordering::Relaxed);
    self
      .last_consistency
      .store(0f64.to_bits(), Ordering::Relaxed);
    self.session_first_fire_qpc.store(0, Ordering::Relaxed);
    self.session_last_fire_qpc.store(0, Ordering::Relaxed);
    self.session_interval_qpc.store(0, Ordering::Relaxed);
  }

  pub(crate) fn configure_trigger(&self, trigger: TriggerKind, action_key_mode: ActionKeyMode) {
    *self.trigger.lock().unwrap() = Some(trigger);
    *self.action_key_mode.lock().unwrap() = action_key_mode;
  }

  pub(crate) fn configure_burst(&self, trigger: Option<TriggerKind>, clicks: u64) {
    *self.burst_trigger.lock().unwrap() = trigger;
    self.burst_clicks.store(clicks, Ordering::Relaxed);
    self.burst_active.store(false, Ordering::Release);
  }

  pub(crate) fn clear_trigger(&self) {
    *self.trigger.lock().unwrap() = None;
    *self.burst_trigger.lock().unwrap() = None;
    self.burst_active.store(false, Ordering::Release);
  }

  pub(crate) fn stop_spam_worker(&self) {
    self.running.store(false, Ordering::Release);
    self.spam_shutdown.store(true, Ordering::Release);
    self.wake.store(true, Ordering::Release);

    if let Some(t) = self.spam_thread.lock().unwrap().as_ref() {
      t.unpark();
    }

    if let Some(handle) = self.spam_join.lock().unwrap().take() {
      let _ = handle.join();
    }

    *self.spam_thread.lock().unwrap() = None;
    self.spam_shutdown.store(false, Ordering::Release);
    self.wake.store(false, Ordering::Release);
    self.burst_active.store(false, Ordering::Release);
  }

  pub(crate) fn shutdown_all(&self) {
    self.running.store(false, Ordering::Release);
    self.spam_shutdown.store(true, Ordering::Release);
    self.app_shutdown.store(true, Ordering::Release);
    self.wake.store(true, Ordering::Release);

    if let Some(t) = self.spam_thread.lock().unwrap().as_ref() {
      t.unpark();
    }

    if let Some(handle) = self.spam_join.lock().unwrap().take() {
      let _ = handle.join();
    }

    if let Some(handle) = self.listener_join.lock().unwrap().take() {
      let _ = handle.join();
    }

    *self.spam_thread.lock().unwrap() = None;
  }
}

fn trigger_virtual_key(trigger: TriggerKind) -> i32 {
  match trigger {
    TriggerKind::Key(vk) => vk as i32,
    TriggerKind::MouseLeft => VK_LBUTTON,
    TriggerKind::MouseRight => VK_RBUTTON,
    TriggerKind::MouseMiddle => VK_MBUTTON,
    TriggerKind::MouseX1 => VK_XBUTTON1,
    TriggerKind::MouseX2 => VK_XBUTTON2,
  }
}

fn finalize_session_stats(state: &SharedState, freq: i64, local_intervals: &[f64]) {
  let (jitter, consistency) = if !local_intervals.is_empty() {
    let n = local_intervals.len();
    let mean = local_intervals.iter().sum::<f64>() / n as f64;
    let variance = local_intervals
      .iter()
      .map(|x| (x - mean).powi(2))
      .sum::<f64>()
      / n as f64;
    let std_dev = variance.sqrt();
    let jitter = local_intervals
      .iter()
      .map(|x| (x - mean).abs())
      .sum::<f64>()
      / n as f64;
    let consistency = if mean > 0.0 {
      (1.0 - std_dev / mean).max(0.0) * 100.0
    } else {
      0.0
    };
    (jitter, consistency)
  } else {
    (0.0, 0.0)
  };

  state
    .last_jitter_ms
    .store(jitter.to_bits(), Ordering::Relaxed);
  state
    .last_consistency
    .store(consistency.to_bits(), Ordering::Relaxed);

  let first_fire = state.session_first_fire_qpc.load(Ordering::Relaxed);
  let last_fire = state.session_last_fire_qpc.load(Ordering::Relaxed);
  let interval_qpc = state.session_interval_qpc.load(Ordering::Relaxed);
  let click_count = state.click_count.load(Ordering::Relaxed);

  if first_fire > 0 && last_fire >= first_fire && interval_qpc > 0 && click_count > 0 {
    // The loop fires immediately, so include one nominal interval to measure the emitted cadence.
    let active_ticks = (last_fire - first_fire).max(0) + interval_qpc;
    let active_secs = active_ticks as f64 / freq as f64;
    let actual_cps = if active_secs > 0.0 {
      click_count as f64 / active_secs
    } else {
      0.0
    };

    state
      .last_elapsed_secs
      .store(active_secs.to_bits(), Ordering::Relaxed);
    state
      .actual_cps
      .store(actual_cps.to_bits(), Ordering::Relaxed);
  }
}

pub(crate) fn spawn_spam_thread(
  state: Arc<SharedState>,
  action: SpamAction,
  press_mode: PressMode,
  target_cps: f64,
  burst_cps: f64,
) {
  let thread_state = state.clone();
  let cpu = CpuLayout::detect();
  let spam_mask = cpu.spam_mask;

  let handle = thread::Builder::new()
    .name("spam-loop".into())
    .spawn(move || {
      unsafe { elevate_spam_thread(spam_mask) };

      let ctx = TimingCtx::calibrate();
      let mut input_batch = build_input_batch(action, press_mode);
      let normal_interval_ticks = (ctx.freq as f64 / target_cps) as i64;
      let burst_interval_ticks = (ctx.freq as f64 / burst_cps) as i64;
      let ticks_to_ms = 1000.0 / ctx.freq as f64;

      *thread_state.spam_thread.lock().unwrap() = Some(thread::current());

      loop {
        while !thread_state.wake.load(Ordering::Acquire) {
          if thread_state.spam_shutdown.load(Ordering::Relaxed) {
            return;
          }
          thread::park_timeout(std::time::Duration::from_millis(100));
        }
        if thread_state.spam_shutdown.load(Ordering::Acquire) {
          return;
        }
        thread_state.wake.store(false, Ordering::Relaxed);

        // 0 = unlimited (normal session); N = burst of exactly N inputs.
        let click_limit = thread_state.session_click_limit.load(Ordering::Acquire);
        let mut fired: u64 = 0;
        let interval_ticks = if click_limit != 0 {
          burst_interval_ticks
        } else {
          normal_interval_ticks
        };

        thread_state
          .session_interval_qpc
          .store(interval_ticks, Ordering::Relaxed);

        let mut local_intervals: Vec<f64> = Vec::with_capacity(100_000);
        let mut next_fire = qpc() + interval_ticks;
        let mut chk = 0u32;

        let first_fire = qpc();
        thread_state
          .session_first_fire_qpc
          .store(first_fire, Ordering::Relaxed);
        thread_state
          .session_last_fire_qpc
          .store(first_fire, Ordering::Relaxed);
        let mut last_fire_ticks = first_fire;

        send_batch(&mut input_batch);
        thread_state.click_count.fetch_add(1, Ordering::Relaxed);
        fired += 1;

        while thread_state.running.load(Ordering::Relaxed) {
          if thread_state.spam_shutdown.load(Ordering::Acquire) {
            return;
          }
          if click_limit != 0 && fired >= click_limit {
            break;
          }

          let now = qpc();
          if next_fire < now - interval_ticks / 128 {
            next_fire = now + interval_ticks;
            continue;
          }

          let slack = next_fire - now;
          if slack > ctx.spin_ahead {
            let s100 = ctx.ticks_to_100ns(slack - ctx.spin_ahead);
            if s100 > 0 {
              unsafe { nt_sleep_100ns(s100) };
            }
          }

          prefetch_batch(&input_batch);

          let remaining = (next_fire - qpc()).max(0);
          let target_tsc = unsafe { rdtscp_now() }.wrapping_add(ctx.to_tsc(remaining));
          let tight_threshold = ctx.tight_tsc;

          loop {
            let now_tsc = unsafe { rdtsc_now() };
            if now_tsc >= target_tsc {
              break;
            }
            if target_tsc - now_tsc <= tight_threshold {
              loop {
                if unsafe { rdtsc_now() } >= target_tsc {
                  break;
                }
              }
              break;
            }
            unsafe { cpu_pause() };
            chk = chk.wrapping_add(1);
            if chk & 0x07 == 0
              && (!thread_state.running.load(Ordering::Relaxed)
                || thread_state.spam_shutdown.load(Ordering::Relaxed))
            {
              break;
            }
          }

          if !thread_state.running.load(Ordering::Acquire) {
            break;
          }

          let fire_ticks = qpc();
          local_intervals.push((fire_ticks - last_fire_ticks) as f64 * ticks_to_ms);
          last_fire_ticks = fire_ticks;
          thread_state
            .session_last_fire_qpc
            .store(fire_ticks, Ordering::Relaxed);

          send_batch(&mut input_batch);
          thread_state.click_count.fetch_add(1, Ordering::Relaxed);
          fired += 1;

          next_fire += interval_ticks;
        }

        if matches!(press_mode, PressMode::DownOnly) {
          let mut release = build_release_batch(action);
          send_batch(&mut release);
        }

        finalize_session_stats(&thread_state, ctx.freq, &local_intervals);

        if click_limit != 0 {
          thread_state.running.store(false, Ordering::Release);
          thread_state.burst_active.store(false, Ordering::Release);
        }
      }
    })
    .expect("Failed to spawn spam thread");

  *state.spam_join.lock().unwrap() = Some(handle);
}

fn start_session(state: &SharedState, click_limit: u64) {
  state.reset_session_stats();
  // Published before `wake` so the spam thread reads it after waking (Acquire).
  state
    .session_click_limit
    .store(click_limit, Ordering::Release);
  state.running.store(true, Ordering::Release);
  state.wake.store(true, Ordering::Release);

  if let Some(t) = state.spam_thread.lock().unwrap().as_ref() {
    t.unpark();
  }
}

fn stop_session(state: &SharedState) {
  if state.running.load(Ordering::Acquire) {
    state.running.store(false, Ordering::Release);
  }
}

pub(crate) fn spawn_listener_thread(state: Arc<SharedState>) {
  if state.listener_started.swap(true, Ordering::AcqRel) {
    return;
  }

  let thread_state = state.clone();

  let handle = thread::Builder::new()
    .name("trigger-loop".into())
    .spawn(move || {
      let cpu = CpuLayout::detect();
      unsafe { elevate_control_thread(cpu.other_mask) };

      let mut trigger_pressed = false;
      let mut last_trigger = None;
      let mut burst_pressed = false;
      let mut last_burst_trigger = None;

      loop {
        if thread_state.app_shutdown.load(Ordering::Acquire) {
          break;
        }

        let current_trigger = *thread_state.trigger.lock().unwrap();
        let action_key_mode = *thread_state.action_key_mode.lock().unwrap();
        let current_burst = *thread_state.burst_trigger.lock().unwrap();

        if current_trigger != last_trigger {
          trigger_pressed = false;
          last_trigger = current_trigger;
        }
        if current_burst != last_burst_trigger {
          burst_pressed = false;
          last_burst_trigger = current_burst;
        }

        let is_down = current_trigger
          .map(trigger_virtual_key)
          .map(async_key_down)
          .unwrap_or(false);

        let burst_down = current_burst
          .map(trigger_virtual_key)
          .map(async_key_down)
          .unwrap_or(false);

        // One-shot burst: fires on the press edge, then stays locked out until
        // the spam thread clears `burst_active` after the last input of the burst.
        let mut burst_active = thread_state.burst_active.load(Ordering::Acquire);
        if burst_down
          && !burst_pressed
          && !burst_active
          && !thread_state.running.load(Ordering::Acquire)
        {
          let clicks = thread_state.burst_clicks.load(Ordering::Relaxed);
          if clicks > 0 {
            thread_state.burst_active.store(true, Ordering::Release);
            start_session(&thread_state, clicks);
            burst_active = true;
          }
        }
        burst_pressed = burst_down;

        // Hold/Toggle handling is paused while a burst runs so a released
        // hold-trigger can't cut the burst short.
        if !burst_active {
          match action_key_mode {
            ActionKeyMode::Hold => {
              if is_down && !thread_state.running.load(Ordering::Acquire) {
                start_session(&thread_state, 0);
              } else if !is_down {
                stop_session(&thread_state);
              }
            }
            ActionKeyMode::Toggle => {
              if is_down && !trigger_pressed {
                if thread_state.running.load(Ordering::Acquire) {
                  stop_session(&thread_state);
                } else {
                  start_session(&thread_state, 0);
                }
              }
            }
          }
        }

        trigger_pressed = is_down;

        let sleep_len = if thread_state.running.load(Ordering::Relaxed) {
          CONTROL_SLEEP_ACTIVE_100NS
        } else {
          CONTROL_SLEEP_IDLE_100NS
        };
        unsafe { nt_sleep_100ns(sleep_len) };
      }

      thread_state
        .listener_started
        .store(false, Ordering::Release);
    })
    .expect("Failed to spawn listener thread");

  *state.listener_join.lock().unwrap() = Some(handle);
}
