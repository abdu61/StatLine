param(
    [string]$OutFile = "$env:TEMP\statline-taskbar.png",
    [string]$ClassName = "Shell_TrayWnd"
)

# Capture a window's pixels with PrintWindow(PW_RENDERFULLCONTENT).
#
# CopyFromScreen misses the Windows 11 taskbar on some systems (including the
# reference dev machine), so this is the reliable way to review taskbar layout.

$code = @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
public class StatLineCapture {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string win);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  public static string Capture(string className, string outFile) {
    IntPtr h = FindWindow(className, null);
    if (h == IntPtr.Zero) return "window not found: " + className;
    RECT r; GetWindowRect(h, out r);
    int w = r.Right - r.Left, ht = r.Bottom - r.Top;
    if (w <= 0 || ht <= 0) return "empty rect";
    using (var bmp = new Bitmap(w, ht, PixelFormat.Format32bppArgb)) {
      using (var g = Graphics.FromImage(bmp)) {
        IntPtr hdc = g.GetHdc();
        bool ok = PrintWindow(h, hdc, 2); // PW_RENDERFULLCONTENT
        g.ReleaseHdc(hdc);
        if (!ok) return "PrintWindow failed";
      }
      bmp.Save(outFile, ImageFormat.Png);
    }
    return "ok " + w + "x" + ht;
  }
}
'@
Add-Type -TypeDefinition $code -Language CSharp -ReferencedAssemblies System.Drawing
$result = [StatLineCapture]::Capture($ClassName, $OutFile)
Write-Output $result
if (Test-Path $OutFile) { Write-Output $OutFile }
