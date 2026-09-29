use std::arch::x86_64::{_MM_HINT_T0, _mm_pause, _mm_prefetch};
use std::ffi::c_void;
use std::mem;
use std::sync::OnceLock;

const INPUT_KEYBOARD: u32 = 1;
const INPUT_MOUSE: u32 = 0;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const KEYEVENTF_SCANCODE: u32 = 0x0008;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const MAPVK_VK_TO_VSC: u32 = 0;
pub(crate) const VK_LBUTTON: i32 = 0x01;
pub(crate) const VK_RBUTTON: i32 = 0x02;
pub(crate) const VK_MBUTTON: i32 = 0x04;
pub(crate) const VK_XBUTTON1: i32 = 0x05;
pub(crate) const VK_XBUTTON2: i32 = 0x06;

const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
const MOUSEEVENTF_XDOWN: u32 = 0x0080;
const MOUSEEVENTF_XUP: u32 = 0x0100;
const XBUTTON1: u32 = 0x0001;
const XBUTTON2: u32 = 0x0002;

const REALTIME_PRIORITY_CLASS: u32 = 0x00000100;
const THREAD_PRIORITY_TIME_CRITICAL: i32 = 15;
const THREAD_PRIORITY_HIGHEST: i32 = 2;

const NT_THREAD_BASE_PRIORITY: u32 = 3;
const NT_THREAD_POWER_THROTTLING: u32 = 49;
const NT_PROCESS_IO_PRIORITY: u32 = 33;
const NT_PROCESS_POWER_THROTTLING: u32 = 77;
const NT_PROCESS_MEMORY_PRIORITY: u32 = 39;

const SPIN_AHEAD_US: i64 = 1_500;
const QPC_CALIB_SAMPLES: usize = 16;
const QPC_CALIB_100NS: i64 = 100_000;

type HANDLE = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct KEYBDINPUT {
  w_vk: u16,
  w_scan: u16,
  dw_flags: u32,
  time: u32,
  dw_extra_info: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MOUSEINPUT {
  dx: i32,
  dy: i32,
  mouse_data: u32,
  dw_flags: u32,
  time: u32,
  _pad: u32,
  dw_extra_info: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
union InputUnion {
  ki: KEYBDINPUT,
  mi: MOUSEINPUT,
  _padding: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct INPUT {
  type_: u32,
  _align_pad: u32,
  u: InputUnion,
}

#[repr(C)]
struct ThreadPowerThrottlingState {
  version: u32,
  control_mask: u32,
  state_mask: u32,
}

#[repr(C)]
struct ProcessPowerThrottlingState {
  version: u32,
  control_mask: u32,
  state_mask: u32,
}

#[repr(C)]
struct MemoryPriorityInformation {
  memory_priority: u32,
}

#[repr(C)]
struct SystemInfo {
  _w_processor_arch: u16,
  _w_reserved: u16,
  _dw_page_size: u32,
  _lp_min_app_addr: usize,
  _lp_max_app_addr: usize,
  _dw_active_proc_mask: usize,
  dw_number_of_processors: u32,
  _rest: [u32; 4],
}

unsafe extern "system" {
  fn SendInput(n_inputs: u32, p_inputs: *mut INPUT, cb_size: i32) -> u32;
  fn MapVirtualKeyW(u_code: u32, u_map_type: u32) -> u32;
  fn GetAsyncKeyState(v_key: i32) -> i16;
  fn QueryPerformanceCounter(c: *mut i64) -> i32;
  fn QueryPerformanceFrequency(f: *mut i64) -> i32;
  fn GetCurrentProcess() -> HANDLE;
  fn GetCurrentThread() -> HANDLE;
  fn SetPriorityClass(h: HANDLE, c: u32) -> i32;
  fn SetThreadPriority(h: HANDLE, p: i32) -> i32;
  fn SetThreadAffinityMask(h: HANDLE, mask: usize) -> usize;
  fn SetThreadIdealProcessor(h: HANDLE, p: u32) -> u32;
  fn GetSystemInfo(si: *mut SystemInfo);
  fn LoadLibraryA(name: *const u8) -> *mut c_void;
  fn GetProcAddress(
    lib: *mut c_void,
    name: *const u8,
  ) -> Option<unsafe extern "system" fn() -> isize>;
}

#[link(name = "winmm")]
unsafe extern "system" {
  fn timeBeginPeriod(p: u32) -> u32;
  fn timeEndPeriod(p: u32) -> u32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
  fn NtSetTimerResolution(desired: u32, set: u8, current: *mut u32) -> u32;
  fn NtQueryTimerResolution(min: *mut u32, max: *mut u32, cur: *mut u32) -> u32;
  fn NtDelayExecution(alertable: u8, interval: *const i64) -> u32;
  fn NtSetInformationThread(h: HANDLE, c: u32, info: *const c_void, len: u32) -> u32;
  fn NtSetInformationProcess(h: HANDLE, c: u32, info: *const c_void, len: u32) -> u32;
}

type FnSendInput = unsafe extern "system" fn(u32, *mut INPUT, i32) -> u32;
type FnAvSetMmThreadChars = unsafe extern "system" fn(*const u8, *mut u32) -> HANDLE;
type FnAvSetMmThreadPrio = unsafe extern "system" fn(HANDLE, i32) -> i32;

struct LowFns {
  send_input: Option<FnSendInput>,
  avrt_chars: Option<FnAvSetMmThreadChars>,
  avrt_prio: Option<FnAvSetMmThreadPrio>,
}

unsafe impl Send for LowFns {}
unsafe impl Sync for LowFns {}

static LOW_FNS: OnceLock<LowFns> = OnceLock::new();

pub(crate) fn init_low_fns() {
  LOW_FNS.get_or_init(|| unsafe {
    let w32u = LoadLibraryA(b"win32u.dll\0".as_ptr());
    let avrt = LoadLibraryA(b"avrt.dll\0".as_ptr());

    let send_input: Option<FnSendInput> = if !w32u.is_null() {
      GetProcAddress(w32u, b"NtUserSendInput\0".as_ptr()).map(|f| mem::transmute(f))
    } else {
      None
    };

    let avrt_chars: Option<FnAvSetMmThreadChars> = if !avrt.is_null() {
      GetProcAddress(avrt, b"AvSetMmThreadCharacteristicsA\0".as_ptr()).map(|f| mem::transmute(f))
    } else {
      None
    };

    let avrt_prio: Option<FnAvSetMmThreadPrio> = if !avrt.is_null() {
      GetProcAddress(avrt, b"AvSetMmThreadPriority\0".as_ptr()).map(|f| mem::transmute(f))
    } else {
      None
    };

    LowFns {
      send_input,
      avrt_chars,
      avrt_prio,
    }
  });
}

pub(crate) fn using_send_input_fallback() -> bool {
  LOW_FNS.get().map_or(true, |f| f.send_input.is_none())
}

#[inline(always)]
pub(crate) fn qpc() -> i64 {
  let mut v = 0i64;
  unsafe { QueryPerformanceCounter(&mut v) };
  v
}

pub(crate) fn qpc_freq() -> i64 {
  let mut v = 0i64;
  unsafe { QueryPerformanceFrequency(&mut v) };
  v
}

#[inline(always)]
unsafe fn rdtsc() -> u64 {
  let lo: u32;
  let hi: u32;
  unsafe {
    core::arch::asm!(
      "rdtsc",
      lateout("eax") lo,
      lateout("edx") hi,
      options(nostack, nomem, preserves_flags),
    );
  }
  ((hi as u64) << 32) | (lo as u64)
}

#[inline(always)]
unsafe fn rdtscp() -> u64 {
  let lo: u32;
  let hi: u32;
  unsafe {
    core::arch::asm!(
      "rdtscp",
      lateout("eax") lo,
      lateout("edx") hi,
      lateout("ecx") _,
      options(nostack, nomem, preserves_flags),
    );
  }
  ((hi as u64) << 32) | (lo as u64)
}

#[inline(always)]
pub(crate) unsafe fn nt_sleep_100ns(ticks: i64) {
  let interval = -ticks;
  unsafe {
    NtDelayExecution(0, &interval);
  }
}
pub(crate) struct TimingCtx {
  pub freq: i64,
  pub inv_100ns: f64,
  pub tsc_per_tick: f64,
  pub spin_ahead: i64,
  pub tight_tsc: u64,
}

impl TimingCtx {
  pub fn calibrate() -> Self {
    let freq = qpc_freq().max(1);
    let mut samples = [(0u64, 0i64); QPC_CALIB_SAMPLES];

    for slot in samples.iter_mut() {
      let q0 = qpc();
      let t0 = unsafe { rdtscp() };
      unsafe { nt_sleep_100ns(QPC_CALIB_100NS) };
      let q1 = qpc();
      let t1 = unsafe { rdtscp() };
      *slot = (t1.wrapping_sub(t0), (q1 - q0).max(1));
    }

    samples.sort_unstable_by(|a, b| {
      let ra = a.0 as f64 / a.1 as f64;
      let rb = b.0 as f64 / b.1 as f64;
      ra.partial_cmp(&rb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let trim = QPC_CALIB_SAMPLES / 4;
    let (mut ts, mut qs) = (0u64, 0i64);
    for &(t, q) in &samples[trim..QPC_CALIB_SAMPLES - trim] {
      ts += t;
      qs += q;
    }
    let tsc_per_tick = if qs > 0 { ts as f64 / qs as f64 } else { 1.0 };

    Self {
      freq,
      inv_100ns: 1e7 / freq as f64,
      tsc_per_tick,
      spin_ahead: freq * SPIN_AHEAD_US / 1_000_000,
      tight_tsc: (freq as f64 / 2_000_000.0 * tsc_per_tick) as u64,
    }
  }

  #[inline(always)]
  pub fn ticks_to_100ns(&self, t: i64) -> i64 {
    (t as f64 * self.inv_100ns) as i64
  }

  #[inline(always)]
  pub fn to_tsc(&self, ticks: i64) -> u64 {
    (ticks.max(0) as f64 * self.tsc_per_tick) as u64
  }
}

pub(crate) struct CpuLayout {
  pub spam_mask: usize,
  pub other_mask: usize,
}

impl CpuLayout {
  pub fn detect() -> Self {
    let count = unsafe {
      let mut si: SystemInfo = mem::zeroed();
      GetSystemInfo(&mut si);
      si.dw_number_of_processors as usize
    }
    .max(1)
    .min(usize::BITS as usize);

    let full = if count == usize::BITS as usize {
      usize::MAX
    } else {
      (1usize << count) - 1
    };
    let smt = count >= 4 && count % 2 == 0;

    if count >= 4 {
      let spam_idx = if smt { count - 2 } else { count - 1 };
      let spam_mask = 1usize << spam_idx;
      let sibling = if smt && spam_idx + 1 < count {
        1usize << (spam_idx + 1)
      } else {
        0
      };
      let other = full & !spam_mask & !sibling;
      let other = if other == 0 { full & !spam_mask } else { other };
      let other = if other == 0 { full } else { other };
      Self {
        spam_mask,
        other_mask: other,
      }
    } else {
      let spam_mask = 1usize << (count - 1);
      let other = full & !spam_mask;
      let other = if other == 0 { full } else { other };
      Self {
        spam_mask,
        other_mask: other,
      }
    }
  }
}

pub(crate) unsafe fn elevate_spam_thread(mask: usize) {
  unsafe {
    let h = GetCurrentThread();
    SetThreadPriority(h, THREAD_PRIORITY_TIME_CRITICAL);
    SetThreadAffinityMask(h, mask);
    SetThreadIdealProcessor(h, mask.trailing_zeros());

    let prio: i32 = 15;
    NtSetInformationThread(h, NT_THREAD_BASE_PRIORITY, &prio as *const _ as _, 4);

    let throttle = ThreadPowerThrottlingState {
      version: 1,
      control_mask: 1,
      state_mask: 0,
    };
    NtSetInformationThread(
      h,
      NT_THREAD_POWER_THROTTLING,
      &throttle as *const _ as _,
      mem::size_of::<ThreadPowerThrottlingState>() as u32,
    );

    if let Some(fns) = LOW_FNS.get() {
      if let (Some(chars), Some(prio_fn)) = (fns.avrt_chars, fns.avrt_prio) {
        let mut idx = 0u32;
        let h_av = chars(b"Pro Audio\0".as_ptr(), &mut idx);
        if !h_av.is_null() {
          prio_fn(h_av, 2);
        }
      }
    }
  }
}

pub(crate) unsafe fn elevate_control_thread(mask: usize) {
  unsafe {
    let h = GetCurrentThread();
    SetThreadPriority(h, THREAD_PRIORITY_HIGHEST);
    SetThreadAffinityMask(h, mask);
    SetThreadIdealProcessor(h, mask.trailing_zeros());

    let throttle = ThreadPowerThrottlingState {
      version: 1,
      control_mask: 1,
      state_mask: 0,
    };
    NtSetInformationThread(
      h,
      NT_THREAD_POWER_THROTTLING,
      &throttle as *const _ as _,
      mem::size_of::<ThreadPowerThrottlingState>() as u32,
    );
  }
}

pub(crate) unsafe fn elevate_process(other_mask: usize) {
  unsafe {
    let proc = GetCurrentProcess();
    SetPriorityClass(proc, REALTIME_PRIORITY_CLASS);

    let io: u32 = 3;
    NtSetInformationProcess(proc, NT_PROCESS_IO_PRIORITY, &io as *const _ as _, 4);

    let mem_prio = MemoryPriorityInformation { memory_priority: 5 };
    NtSetInformationProcess(
      proc,
      NT_PROCESS_MEMORY_PRIORITY,
      &mem_prio as *const _ as _,
      mem::size_of::<MemoryPriorityInformation>() as u32,
    );

    let throttle = ProcessPowerThrottlingState {
      version: 1,
      control_mask: 0x7,
      state_mask: 0,
    };
    NtSetInformationProcess(
      proc,
      NT_PROCESS_POWER_THROTTLING,
      &throttle as *const _ as _,
      mem::size_of::<ProcessPowerThrottlingState>() as u32,
    );

    SetThreadAffinityMask(GetCurrentThread(), other_mask);
  }
}

pub(crate) struct TimerGuard {
  nt_res: u32,
}

impl TimerGuard {
  pub unsafe fn install() -> Self {
    unsafe {
      timeBeginPeriod(1);
      let mut min = 0u32;
      let mut max = 0u32;
      let mut cur = 0u32;
      NtQueryTimerResolution(&mut min, &mut max, &mut cur);
      NtSetTimerResolution(max, 1, &mut cur);
      Self { nt_res: max }
    }
  }
}

impl Drop for TimerGuard {
  fn drop(&mut self) {
    unsafe {
      let mut cur = 0u32;
      NtSetTimerResolution(self.nt_res, 0, &mut cur);
      timeEndPeriod(1);
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpamAction {
  Key(u16),
  MouseLeft,
  MouseRight,
  MouseMiddle,
  MouseX1,
  MouseX2,
}

#[derive(Clone, Copy)]
pub(crate) enum PressMode {
  DownUp,
  DownOnly,
}

#[inline(always)]
fn vk_needs_extended(vk: u16) -> bool {
  matches!(
    vk as i32,
    0x21
      | 0x22
      | 0x23
      | 0x24
      | 0x25
      | 0x26
      | 0x27
      | 0x28
      | 0x2D
      | 0x2E
      | 0x5B
      | 0x5C
      | 0x5D
      | 0x6F
      | 0x90
      | 0xA1
      | 0xA3
      | 0xA5
  )
}

pub(crate) fn build_input_batch(action: SpamAction, press_mode: PressMode) -> Vec<INPUT> {
  let per = match press_mode {
    PressMode::DownUp => 2,
    PressMode::DownOnly => 1,
  };
  let mut batch = Vec::with_capacity(per);

  match action {
    SpamAction::Key(vk) => {
      let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 };
      let ext = if vk_needs_extended(vk) {
        KEYEVENTF_EXTENDEDKEY
      } else {
        0
      };
      batch.push(make_kb(vk, scan, KEYEVENTF_SCANCODE | ext));
      if matches!(press_mode, PressMode::DownUp) {
        batch.push(make_kb(
          vk,
          scan,
          KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP | ext,
        ));
      }
    }
    SpamAction::MouseLeft => {
      batch.push(make_mouse(MOUSEEVENTF_LEFTDOWN));
      if matches!(press_mode, PressMode::DownUp) {
        batch.push(make_mouse(MOUSEEVENTF_LEFTUP));
      }
    }
    SpamAction::MouseRight => {
      batch.push(make_mouse(MOUSEEVENTF_RIGHTDOWN));
      if matches!(press_mode, PressMode::DownUp) {
        batch.push(make_mouse(MOUSEEVENTF_RIGHTUP));
      }
    }
    SpamAction::MouseMiddle => {
      batch.push(make_mouse(MOUSEEVENTF_MIDDLEDOWN));
      if matches!(press_mode, PressMode::DownUp) {
        batch.push(make_mouse(MOUSEEVENTF_MIDDLEUP));
      }
    }
    SpamAction::MouseX1 => {
      batch.push(make_mouse_x(MOUSEEVENTF_XDOWN, XBUTTON1));
      if matches!(press_mode, PressMode::DownUp) {
        batch.push(make_mouse_x(MOUSEEVENTF_XUP, XBUTTON1));
      }
    }
    SpamAction::MouseX2 => {
      batch.push(make_mouse_x(MOUSEEVENTF_XDOWN, XBUTTON2));
      if matches!(press_mode, PressMode::DownUp) {
        batch.push(make_mouse_x(MOUSEEVENTF_XUP, XBUTTON2));
      }
    }
  }
  batch
}

/// Without this after a DownOnly session, Windows treats the key/button as held
/// and other windows (e.g. title bars) stop responding to clicks.
pub(crate) fn build_release_batch(action: SpamAction) -> Vec<INPUT> {
  let mut batch = Vec::with_capacity(1);
  match action {
    SpamAction::Key(vk) => {
      let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) as u16 };
      let ext = if vk_needs_extended(vk) {
        KEYEVENTF_EXTENDEDKEY
      } else {
        0
      };
      batch.push(make_kb(
        vk,
        scan,
        KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP | ext,
      ));
    }
    SpamAction::MouseLeft => batch.push(make_mouse(MOUSEEVENTF_LEFTUP)),
    SpamAction::MouseRight => batch.push(make_mouse(MOUSEEVENTF_RIGHTUP)),
    SpamAction::MouseMiddle => batch.push(make_mouse(MOUSEEVENTF_MIDDLEUP)),
    SpamAction::MouseX1 => batch.push(make_mouse_x(MOUSEEVENTF_XUP, XBUTTON1)),
    SpamAction::MouseX2 => batch.push(make_mouse_x(MOUSEEVENTF_XUP, XBUTTON2)),
  }
  batch
}

fn make_kb(vk: u16, scan: u16, flags: u32) -> INPUT {
  INPUT {
    type_: INPUT_KEYBOARD,
    _align_pad: 0,
    u: InputUnion {
      ki: KEYBDINPUT {
        w_vk: vk,
        w_scan: scan,
        dw_flags: flags,
        time: 0,
        dw_extra_info: 0,
      },
    },
  }
}

fn make_mouse(flags: u32) -> INPUT {
  INPUT {
    type_: INPUT_MOUSE,
    _align_pad: 0,
    u: InputUnion {
      mi: MOUSEINPUT {
        dx: 0,
        dy: 0,
        mouse_data: 0,
        dw_flags: flags,
        time: 0,
        _pad: 0,
        dw_extra_info: 0,
      },
    },
  }
}

fn make_mouse_x(flags: u32, xbutton: u32) -> INPUT {
  INPUT {
    type_: INPUT_MOUSE,
    _align_pad: 0,
    u: InputUnion {
      mi: MOUSEINPUT {
        dx: 0,
        dy: 0,
        mouse_data: xbutton,
        dw_flags: flags,
        time: 0,
        _pad: 0,
        dw_extra_info: 0,
      },
    },
  }
}

#[inline(always)]
pub(crate) fn send_batch(batch: &mut [INPUT]) {
  unsafe {
    let n = batch.len() as u32;
    let p = batch.as_mut_ptr();
    let sz = mem::size_of::<INPUT>() as i32;
    match LOW_FNS.get().and_then(|x| x.send_input) {
      Some(f) => {
        f(n, p, sz);
      }
      None => {
        SendInput(n, p, sz);
      }
    }
  }
}

#[inline(always)]
pub(crate) fn prefetch_batch(batch: &[INPUT]) {
  unsafe {
    _mm_prefetch(batch.as_ptr() as *const i8, _MM_HINT_T0);
  }
}

#[inline(always)]
pub(crate) unsafe fn cpu_pause() {
  _mm_pause();
}

#[inline(always)]
pub(crate) unsafe fn rdtsc_now() -> u64 {
  unsafe { rdtsc() }
}

#[inline(always)]
pub(crate) unsafe fn rdtscp_now() -> u64 {
  unsafe { rdtscp() }
}

#[inline(always)]
pub(crate) fn async_key_down(vk: i32) -> bool {
  unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}
