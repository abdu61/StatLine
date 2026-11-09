param(
    [string]$OutFile = "$env:TEMP\statline-screen.png",
    [int]$X = 0,
    [int]$Y = 0,
    [int]$Width = 0,
    [int]$Height = 0
)

# Capture a screen region with BitBlt using CAPTUREBLT, which includes layered
# windows. `CopyFromScreen` (and GDI without CAPTUREBLT) can miss the taskbar and
# layered surfaces on some display drivers.

$code = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public class StatLineScreen {
  [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr hwnd, IntPtr hdc);
  [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr hdcDest, int x, int y, int w, int h,
      IntPtr hdcSrc, int srcX, int srcY, uint rop);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleDC(IntPtr hdc);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int w, int h);
  [DllImport("gdi32.dll")] public static extern IntPtr SelectObject(IntPtr hdc, IntPtr obj);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr obj);
  [DllImport("gdi32.dll")] public static extern bool DeleteDC(IntPtr hdc);
  const uint SRCCOPY = 0x00CC0020;
  const uint CAPTUREBLT = 0x40000000;

  public static string Capture(int x, int y, int w, int h, string path) {
    if (w <= 0) w = 1536;
    if (h <= 0) h = 864;
    IntPtr screen = GetDC(IntPtr.Zero);
    IntPtr mem = CreateCompatibleDC(screen);
    IntPtr bmp = CreateCompatibleBitmap(screen, w, h);
    IntPtr old = SelectObject(mem, bmp);
    bool ok = BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT);
    SelectObject(mem, old);
    using (var image = Image.FromHbitmap(bmp)) {
      image.Save(path, ImageFormat.Png);
    }
    DeleteObject(bmp);
    DeleteDC(mem);
    ReleaseDC(IntPtr.Zero, screen);
    return ok ? "ok " + w + "x" + h : "BitBlt failed";
  }
}
'@
Add-Type -TypeDefinition $code -Language CSharp -ReferencedAssemblies System.Drawing
$result = [StatLineScreen]::Capture($X, $Y, $Width, $Height, $OutFile)
Write-Output $result
if (Test-Path $OutFile) { Write-Output $OutFile }
