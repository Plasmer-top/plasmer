# Plasmer

A lightweight GUI autoclicker / key spammer for Windows.

> Many games forbid autoclickers in their terms of service. You are responsible for how you use this tool.

## Features

- Simple desktop GUI
- Configurable action key mode (`hold` or `toggle`)
- Configurable action press mode (`down + up` or `down only`)
- Supports mouse buttons, letters, digits, `f1-f24`, modifiers, navigation keys, numpad and punctuation (see [Supported Input Syntax](#supported-input-syntax))
- Press **Capture** next to a field and press a key/button to bind it
- Configurable target CPS (1–1000)
- One-shot burst mode: tap a separate key to fire an exact number of clicks (1–100) at its own CPS
- Displays session stats:
  - Total clicks
  - Actual CPS
  - Elapsed time
  - Jitter
  - Consistency
- Saves settings automatically
- High-resolution timing (QPC + RDTSC spin-wait, raised timer resolution, REALTIME process priority)

## How It Works

- You configure:
  - A trigger key/button
  - One or more actions to spam
  - How actions are emitted (`down + up` or `down only`)
  - A target CPS
- `Hold` mode:
  - Press and hold the trigger
  - While held, the app repeatedly sends the configured actions
  - Releasing the trigger stops the spam loop and updates the final stats
- `Toggle` mode:
  - Press the trigger once to start
  - Press it again to stop
- Burst mode:
  - Tap the burst trigger key to fire exactly the configured number of clicks, then stop
  - The key is locked out until the burst finishes; the normal trigger works independently

## Supported Input Syntax

### Trigger key

The trigger accepts the first valid token from the trigger input field.

Examples:

- `x`
- `f6`
- `space`
- `lmb`
- `mb4`

### Tokens

| Group | Tokens |
|---|---|
| Mouse | `lmb` `rmb` `mmb` `mb4` `mb5` |
| Letters / digits | `a`–`z`, `0`–`9` |
| Function keys | `f1`–`f24` |
| Modifiers | `shift` `ctrl` `alt` `lshift` `rshift` `lctrl` `rctrl` `lalt` `ralt` `lwin`/`win` `rwin` |
| Navigation / editing | `space` `enter` `esc` `tab` `backspace` `capslock` `insert` `delete` `home` `end` `pgup` `pgdn` `up` `down` `left` `right` `printscreen` `scrolllock` `pause` `numlock` `apps` |
| Numpad | `num0`–`num9` `numadd` `numsub` `nummul` `numdiv` `numdec` `numenter` |
| Punctuation | `semicolon` `equals` `comma` `minus` `period` `slash` `backtick` `lbracket` `backslash` `rbracket` `quote` (or the characters themselves: `;` `=` `,` `-` `.` `/` `` ` `` `[` `\` `]` `'`) |

### Actions

The actions field supports one or more space-separated tokens.

Examples:

- `lmb`
- `space`
- `a`
- `1`
- `f5`
- `lmb rmb`
- `space 1 f2`
- `a s d`

## Stats Explained

- `Clicks`  
  Total number of actions sent during the last session.

- `Actual CPS`  
  Measured average actions per second over the session.

- `Elapsed`  
  Total session duration.

- `Jitter`  
  Average deviation between actual send intervals and the mean interval, in milliseconds.

- `Consistency`  
  Percent score computed as (1 − stddev/mean) × 100 over per-tick intervals; higher is steadier.

Note: stats are finalized when the session stops (release in `hold` mode, second press in `toggle` mode).

## Settings Storage

Settings are saved as JSON in your config directory:

```text
%APPDATA%\plasmer-ac\settings.json
```

## Network Access

- On startup the app checks `https://plasmer.top/api/update/latest` for a newer version and shows a download link if one exists.
- If Discord is running, it shows Rich Presence (app version and current CPS). Set `RPC_ENABLED` in `src/discord_rpc.rs` to `false` to disable it.

No other data is sent.

## Building

Windows only.

```bash
cargo build --release
```

Or cross-compile with Docker (output: `target/docker/Plasmer.exe`, icon embedded):

```bat
build.bat
```

## License

[Apache-2.0](LICENSE)
