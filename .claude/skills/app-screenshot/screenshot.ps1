param(
  [string]$Exe = "target\debug\Plasmer.exe",
  [string]$Out = "assets\screenshot.webp",
  [int]$Quality = 90,  # WebP quality; 90 keeps text crisp at ~1/4 the PNG size
  [int]$Radius = 8,
  [int]$WaitMs = 2500,
  [double]$Zoom = 0  # 0 = largest egui zoom that still fits on screen
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
}
"@
# Without this, coordinates are DPI-virtualized and the capture is offset/cropped.
[W]::SetProcessDPIAware() | Out-Null

$proc = Start-Process -FilePath $Exe -PassThru
try {
  # The window first appears tiny before it is sized, so wait for real dimensions.
  # DWMWA_EXTENDED_FRAME_BOUNDS (9): visible frame, excluding the invisible resize border/shadow.
  $r = New-Object W+RECT
  $w = 0
  for ($i = 0; $i -lt 200 -and $w -lt 100; $i++) {
    Start-Sleep -Milliseconds 100
    $proc.Refresh()
    $h = $proc.MainWindowHandle
    if ($h -ne [IntPtr]::Zero) {
      [W]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) | Out-Null
      $w = $r.R - $r.L
    }
  }
  if ($w -lt 100) { throw "no sized window for $Exe" }
  [W]::SetForegroundWindow($h) | Out-Null
  Start-Sleep -Milliseconds 500

  # More pixels = render bigger: egui zooms +0.1 per Ctrl+NumpadAdd (layout-independent),
  # and the window is enlarged by the same factor so the layout stays identical.
  $wr = New-Object W+RECT; $cr = New-Object W+RECT
  [W]::GetWindowRect($h, [ref]$wr) | Out-Null
  [W]::GetClientRect($h, [ref]$cr) | Out-Null
  $frameW = ($wr.R - $wr.L) - $cr.R; $frameH = ($wr.B - $wr.T) - $cr.B
  $work = [System.Windows.Forms.Screen]::FromHandle($h).WorkingArea
  if ($Zoom -le 0) {
    $Zoom = [Math]::Min(($work.Width - $frameW) / $cr.R, ($work.Height - $frameH) / $cr.B)
  }
  $steps = [int][Math]::Floor(($Zoom - 1) * 10 + 1e-6)
  $Zoom = 1 + $steps / 10
  for ($i = 0; $i -lt $steps; $i++) {
    [System.Windows.Forms.SendKeys]::SendWait("^{ADD}")
    Start-Sleep -Milliseconds 50
  }
  $newW = [int]($cr.R * $Zoom) + $frameW; $newH = [int]($cr.B * $Zoom) + $frameH
  # Invisible resize border sits outside the work area edge, so offset by it to keep the frame on screen.
  $bx = $r.L - $wr.L; $by = $r.T - $wr.T
  [W]::SetWindowPos($h, [IntPtr]::Zero, $work.X - $bx, $work.Y - $by, $newW, $newH, 0x4) | Out-Null
  Start-Sleep -Milliseconds $WaitMs
  [W]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) | Out-Null
  # Drop the 1px Win11 border: it is translucent and picks up whatever is behind the window.
  $w = $r.R - $r.L - 2; $hgt = $r.B - $r.T - 2

  $src = New-Object System.Drawing.Bitmap $w, $hgt
  $g = [System.Drawing.Graphics]::FromImage($src)
  $g.CopyFromScreen($r.L + 1, $r.T + 1, 0, 0, $src.Size)
  $g.Dispose()

  $rad = [int]($Radius * [W]::GetDpiForWindow($h) / 96 * $Zoom)
  $d = $rad * 2
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $path.AddArc(0, 0, $d, $d, 180, 90)
  $path.AddArc($w - $d, 0, $d, $d, 270, 90)
  $path.AddArc($w - $d, $hgt - $d, $d, $d, 0, 90)
  $path.AddArc(0, $hgt - $d, $d, $d, 90, 90)
  $path.CloseFigure()

  $dst = New-Object System.Drawing.Bitmap $w, $hgt, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($dst)
  $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
  $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $g.Clear([System.Drawing.Color]::Transparent)
  $brush = New-Object System.Drawing.TextureBrush $src
  $g.FillPath($brush, $path)
  $g.Dispose(); $brush.Dispose(); $src.Dispose()

  $full = [System.IO.Path]::GetFullPath((Join-Path (Get-Location) $Out))
  New-Item -ItemType Directory -Force (Split-Path $full) | Out-Null
  # System.Drawing can't encode WebP, so go through a temp PNG and ffmpeg.
  $png = Join-Path $env:TEMP "plasmer-screenshot.png"
  $dst.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
  $dst.Dispose()
  ffmpeg -hide_banner -loglevel error -y -i $png -c:v libwebp -quality $Quality -compression_level 6 $full
  if ($LASTEXITCODE -ne 0) { throw "ffmpeg WebP encode failed" }
  Remove-Item $png
  "saved $full (${w}x${hgt}, zoom $Zoom, radius ${rad}px, $([int]((Get-Item $full).Length / 1KB)) KB)"
} finally {
  Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
}
