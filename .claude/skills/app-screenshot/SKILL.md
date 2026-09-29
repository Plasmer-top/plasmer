---
name: app-screenshot
description: Take a screenshot of the Plasmer app window on Windows with rounded, transparent corners (WebP). Use when asked for a screenshot of the app, e.g. for the README.
---

From the repo root:

```powershell
cargo build
powershell -NoProfile -ExecutionPolicy Bypass -File F:\Dokumente\plasmer-ac\.claude\skills\app-screenshot\screenshot.ps1
```

Launches the exe, captures its window, rounds the corners, saves `assets\screenshot.webp`, kills the app. Takes about 5s. Read the WebP afterwards to check it.

Params: `-Exe` (default `target\debug\Plasmer.exe`), `-Out`, `-Quality` (WebP quality, default 90: about 30 KB, text stays crisp; lossless would be about 47 KB), `-Radius` (logical px, default 8 = Win11 corner, scaled by DPI and zoom), `-WaitMs` (settle time after resizing), `-Zoom` (default 0 = largest that fits the screen; `1` = native size).

High resolution: a screen grab can't exceed what the app draws. So the script presses Ctrl+NumpadAdd (egui's built-in zoom, +0.1 per press, works on any keyboard layout) and enlarges the window by the same factor. On 2560x1440 at 125% this gives zoom 1.6, about 941x1317. The OS title bar does not zoom, so it looks proportionally smaller.

Notes:
- Needs `ffmpeg` (with libwebp) on PATH; System.Drawing can't encode WebP. Transparency survives.
- It screen-grabs (`CopyFromScreen`), so the window must be visible on screen and not covered. The script brings it to the foreground.
- The UI shows the user's saved settings from `%APPDATA%\plasmer-ac\settings.json`.
- Pass an absolute script path; the PowerShell tool's cwd can drift if Bash ran `cd`.
- Pitfalls already handled in the script: DPI awareness (otherwise the capture is offset), the window appearing tiny before egui sizes it, the translucent 1px Win11 border (cropped), and `FindWindow($null, ...)`, which fails because PowerShell passes `$null` as `""` (so it uses `MainWindowHandle` instead).
