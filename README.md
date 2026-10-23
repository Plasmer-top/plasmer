# <img src="assets/icon.png" alt="" width="40" align="center"> Plasmer

A lightweight autoclicker / key spammer for Windows with precise, steady click timing.

> Many games forbid autoclickers in their terms of service. You are responsible for how you use this tool.

Download the latest version from [plasmer.top](https://plasmer.top).

## Usage

1. Click the **Trigger** box and press the key or mouse button that should start clicking. Do the same for **Action**, the key or button that gets spammed. Click a box again to cancel.
2. Choose the **Trigger mode**:
   - **Hold**: clicks while the trigger is held.
   - **Toggle**: press the trigger to start, press it again to stop.
3. Set **Clicks per second** (1–1000).
4. Press **Start**. Settings are locked until you press **Stop**.

**Press type** decides what each click sends: **Down + Up** is a full press and release, **Down only** sends just the press and releases once clicking stops.

**Burst** fires an exact number of clicks at its own speed when you tap the burst key. The key is locked out until the burst finishes; the normal trigger keeps working.

Any mouse button (left, right, middle, 4, 5) and almost any key can be bound: letters, digits, F1–F24, modifiers, navigation keys, numpad and punctuation.

## Stats

Shown above Start/Stop while running. Clicks, CPS and Time update live. Jitter and Consistency appear when a session ends (trigger released or toggled off, or burst finished):

- **Jitter**: how far the gaps between clicks strayed from their average, in milliseconds. Lower is better.
- **Consistency**: how even the gaps between clicks were. 100% is perfectly even.

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
