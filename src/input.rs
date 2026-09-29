use crate::win32::{SpamAction, async_key_down};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerKind {
  Key(u16),
  MouseLeft,
  MouseRight,
  MouseMiddle,
  MouseX1,
  MouseX2,
}

pub(crate) fn parse_action(token: &str) -> Option<SpamAction> {
  let s = token.to_ascii_lowercase();
  match s.as_str() {
    "lmb" => Some(SpamAction::MouseLeft),
    "rmb" => Some(SpamAction::MouseRight),
    "mmb" => Some(SpamAction::MouseMiddle),
    "mb4" => Some(SpamAction::MouseX1),
    "mb5" => Some(SpamAction::MouseX2),
    "space" => Some(SpamAction::Key(0x20)),

    "shift" => Some(SpamAction::Key(0x10)),
    "ctrl" => Some(SpamAction::Key(0x11)),
    "alt" => Some(SpamAction::Key(0x12)),
    "lshift" => Some(SpamAction::Key(0xA0)),
    "rshift" => Some(SpamAction::Key(0xA1)),
    "lctrl" => Some(SpamAction::Key(0xA2)),
    "rctrl" => Some(SpamAction::Key(0xA3)),
    "lalt" => Some(SpamAction::Key(0xA4)),
    "ralt" => Some(SpamAction::Key(0xA5)),
    "lwin" | "win" => Some(SpamAction::Key(0x5B)),
    "rwin" => Some(SpamAction::Key(0x5C)),

    "enter" | "return" => Some(SpamAction::Key(0x0D)),
    "esc" | "escape" => Some(SpamAction::Key(0x1B)),
    "tab" => Some(SpamAction::Key(0x09)),
    "backspace" | "bksp" => Some(SpamAction::Key(0x08)),
    "capslock" | "caps" => Some(SpamAction::Key(0x14)),
    "ins" | "insert" => Some(SpamAction::Key(0x2D)),
    "del" | "delete" => Some(SpamAction::Key(0x2E)),
    "home" => Some(SpamAction::Key(0x24)),
    "end" => Some(SpamAction::Key(0x23)),
    "pgup" | "pageup" => Some(SpamAction::Key(0x21)),
    "pgdn" | "pagedown" => Some(SpamAction::Key(0x22)),
    "up" => Some(SpamAction::Key(0x26)),
    "down" => Some(SpamAction::Key(0x28)),
    "left" => Some(SpamAction::Key(0x25)),
    "right" => Some(SpamAction::Key(0x27)),
    "printscreen" | "prtsc" => Some(SpamAction::Key(0x2C)),
    "scrolllock" => Some(SpamAction::Key(0x91)),
    "pause" | "break" => Some(SpamAction::Key(0x13)),
    "numlock" => Some(SpamAction::Key(0x90)),
    "apps" | "menu" => Some(SpamAction::Key(0x5D)),

    "num0" => Some(SpamAction::Key(0x60)),
    "num1" => Some(SpamAction::Key(0x61)),
    "num2" => Some(SpamAction::Key(0x62)),
    "num3" => Some(SpamAction::Key(0x63)),
    "num4" => Some(SpamAction::Key(0x64)),
    "num5" => Some(SpamAction::Key(0x65)),
    "num6" => Some(SpamAction::Key(0x66)),
    "num7" => Some(SpamAction::Key(0x67)),
    "num8" => Some(SpamAction::Key(0x68)),
    "num9" => Some(SpamAction::Key(0x69)),
    "numadd" => Some(SpamAction::Key(0x6B)),
    "numsub" => Some(SpamAction::Key(0x6D)),
    "nummul" => Some(SpamAction::Key(0x6A)),
    "numdiv" => Some(SpamAction::Key(0x6F)),
    "numdec" => Some(SpamAction::Key(0x6E)),
    "numenter" => Some(SpamAction::Key(0x0D)),

    "semicolon" => Some(SpamAction::Key(0xBA)),
    "equals" => Some(SpamAction::Key(0xBB)),
    "comma" => Some(SpamAction::Key(0xBC)),
    "minus" => Some(SpamAction::Key(0xBD)),
    "period" => Some(SpamAction::Key(0xBE)),
    "slash" => Some(SpamAction::Key(0xBF)),
    "backtick" => Some(SpamAction::Key(0xC0)),
    "lbracket" => Some(SpamAction::Key(0xDB)),
    "backslash" => Some(SpamAction::Key(0xDC)),
    "rbracket" => Some(SpamAction::Key(0xDD)),
    "quote" => Some(SpamAction::Key(0xDE)),

    ";" => Some(SpamAction::Key(0xBA)),
    "=" => Some(SpamAction::Key(0xBB)),
    "," => Some(SpamAction::Key(0xBC)),
    "-" => Some(SpamAction::Key(0xBD)),
    "." => Some(SpamAction::Key(0xBE)),
    "/" => Some(SpamAction::Key(0xBF)),
    "`" => Some(SpamAction::Key(0xC0)),
    "[" => Some(SpamAction::Key(0xDB)),
    "\\" => Some(SpamAction::Key(0xDC)),
    "]" => Some(SpamAction::Key(0xDD)),
    "'" => Some(SpamAction::Key(0xDE)),

    s if s.starts_with('f') && s.len() > 1 => {
      let n: u32 = s[1..].parse().ok()?;
      (1..=24)
        .contains(&n)
        .then(|| SpamAction::Key(0x6F + n as u16))
    }
    s if s.len() == 1 => {
      let c = s.chars().next()?;
      if c.is_ascii_alphabetic() {
        Some(SpamAction::Key(c.to_ascii_uppercase() as u16))
      } else if c.is_ascii_digit() {
        Some(SpamAction::Key(c as u16))
      } else {
        None
      }
    }
    _ => None,
  }
}

pub(crate) fn action_to_trigger(a: SpamAction) -> TriggerKind {
  match a {
    SpamAction::MouseLeft => TriggerKind::MouseLeft,
    SpamAction::MouseRight => TriggerKind::MouseRight,
    SpamAction::MouseMiddle => TriggerKind::MouseMiddle,
    SpamAction::MouseX1 => TriggerKind::MouseX1,
    SpamAction::MouseX2 => TriggerKind::MouseX2,
    SpamAction::Key(vk) => TriggerKind::Key(vk),
  }
}

pub(crate) fn action_name(a: SpamAction) -> String {
  match a {
    SpamAction::MouseLeft => "LMB".into(),
    SpamAction::MouseRight => "RMB".into(),
    SpamAction::MouseMiddle => "MMB".into(),
    SpamAction::MouseX1 => "MB4".into(),
    SpamAction::MouseX2 => "MB5".into(),
    SpamAction::Key(vk) => {
      if (0x41..=0x5A).contains(&vk) {
        (vk as u8 as char).to_string()
      } else if (0x30..=0x39).contains(&vk) {
        (vk as u8 as char).to_string()
      } else if (0x70..=0x87).contains(&vk) {
        format!("F{}", vk - 0x6F)
      } else if (0x60..=0x69).contains(&vk) {
        format!("NUM{}", vk - 0x60)
      } else {
        match vk {
          0x20 => "SPACE".into(),
          0x10 => "SHIFT".into(),
          0x11 => "CTRL".into(),
          0x12 => "ALT".into(),
          0xA0 => "LSHIFT".into(),
          0xA1 => "RSHIFT".into(),
          0xA2 => "LCTRL".into(),
          0xA3 => "RCTRL".into(),
          0xA4 => "LALT".into(),
          0xA5 => "RALT".into(),
          0x5B => "LWIN".into(),
          0x5C => "RWIN".into(),
          0x0D => "ENTER".into(),
          0x1B => "ESC".into(),
          0x09 => "TAB".into(),
          0x08 => "BKSP".into(),
          0x14 => "CAPS".into(),
          0x2D => "INS".into(),
          0x2E => "DEL".into(),
          0x24 => "HOME".into(),
          0x23 => "END".into(),
          0x21 => "PGUP".into(),
          0x22 => "PGDN".into(),
          0x26 => "UP".into(),
          0x28 => "DOWN".into(),
          0x25 => "LEFT".into(),
          0x27 => "RIGHT".into(),
          0x2C => "PRTSC".into(),
          0x91 => "SCRLK".into(),
          0x13 => "PAUSE".into(),
          0x90 => "NUMLK".into(),
          0x5D => "MENU".into(),
          0x6B => "NUM+".into(),
          0x6D => "NUM-".into(),
          0x6A => "NUM*".into(),
          0x6F => "NUM/".into(),
          0x6E => "NUM.".into(),
          0xBA => ";".into(),
          0xBB => "=".into(),
          0xBC => ",".into(),
          0xBD => "-".into(),
          0xBE => ".".into(),
          0xBF => "/".into(),
          0xC0 => "`".into(),
          0xDB => "[".into(),
          0xDC => "\\".into(),
          0xDD => "]".into(),
          0xDE => "'".into(),
          _ => format!("VK({:#04x})", vk),
        }
      }
    }
  }
}

fn capture_vks() -> impl Iterator<Item = i32> {
  let fixed = [
    0x01i32, 0x02, 0x04, 0x05, 0x06,
    0x20,
    0x10, 0x11, 0x12, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C,
    0x0D, 0x1B, 0x09, 0x08, 0x14, 0x2D, 0x2E, 0x24, 0x23, 0x21, 0x22, 0x26, 0x28, 0x25, 0x27, 0x2C,
    0x91, 0x13, 0x90, 0x5D,
    0x6A, 0x6B, 0x6D, 0x6E, 0x6F,
    0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xDB, 0xDC, 0xDD, 0xDE,
  ];
  fixed
    .into_iter()
    .chain(0x30..=0x39)
    .chain(0x41..=0x5A)
    .chain(0x60..=0x69)
    .chain(0x70..=0x87)
}

fn vk_to_token(vk: i32) -> Option<&'static str> {
  match vk {
    0x01 => Some("lmb"),
    0x02 => Some("rmb"),
    0x04 => Some("mmb"),
    0x05 => Some("mb4"),
    0x06 => Some("mb5"),
    0x20 => Some("space"),

    0x10 => Some("shift"),
    0x11 => Some("ctrl"),
    0x12 => Some("alt"),
    0xA0 => Some("lshift"),
    0xA1 => Some("rshift"),
    0xA2 => Some("lctrl"),
    0xA3 => Some("rctrl"),
    0xA4 => Some("lalt"),
    0xA5 => Some("ralt"),
    0x5B => Some("lwin"),
    0x5C => Some("rwin"),

    0x0D => Some("enter"),
    0x1B => Some("esc"),
    0x09 => Some("tab"),
    0x08 => Some("backspace"),
    0x14 => Some("capslock"),
    0x2D => Some("insert"),
    0x2E => Some("delete"),
    0x24 => Some("home"),
    0x23 => Some("end"),
    0x21 => Some("pgup"),
    0x22 => Some("pgdn"),
    0x26 => Some("up"),
    0x28 => Some("down"),
    0x25 => Some("left"),
    0x27 => Some("right"),
    0x2C => Some("printscreen"),
    0x91 => Some("scrolllock"),
    0x13 => Some("pause"),
    0x90 => Some("numlock"),
    0x5D => Some("apps"),

    0x6A => Some("nummul"),
    0x6B => Some("numadd"),
    0x6D => Some("numsub"),
    0x6E => Some("numdec"),
    0x6F => Some("numdiv"),

    0xBA => Some("semicolon"),
    0xBB => Some("equals"),
    0xBC => Some("comma"),
    0xBD => Some("minus"),
    0xBE => Some("period"),
    0xBF => Some("slash"),
    0xC0 => Some("backtick"),
    0xDB => Some("lbracket"),
    0xDC => Some("backslash"),
    0xDD => Some("rbracket"),
    0xDE => Some("quote"),

    _ => None,
  }
}

fn is_mouse_vk(vk: i32) -> bool {
  matches!(vk, 0x01 | 0x02 | 0x04 | 0x05 | 0x06)
}

pub(crate) fn any_capture_key_down(allow_mouse: bool) -> bool {
  capture_vks()
    .filter(|vk| allow_mouse || !is_mouse_vk(*vk))
    .any(async_key_down)
}

pub(crate) fn capture_pressed_token(allow_mouse: bool) -> Option<String> {
  for vk in capture_vks() {
    if !allow_mouse && is_mouse_vk(vk) {
      continue;
    }
    if !async_key_down(vk) {
      continue;
    }
    if let Some(t) = vk_to_token(vk) {
      return Some(t.to_string());
    }
    if (0x30..=0x39).contains(&vk) || (0x41..=0x5A).contains(&vk) {
      let c = (vk as u8 as char).to_ascii_lowercase();
      return Some(c.to_string());
    }
    if (0x60..=0x69).contains(&vk) {
      return Some(format!("num{}", vk - 0x60));
    }
    if (0x70..=0x87).contains(&vk) {
      return Some(format!("f{}", vk - 0x6F));
    }
  }
  None
}
