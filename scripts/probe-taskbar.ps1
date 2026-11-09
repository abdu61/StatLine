# Print the Windows 11 taskbar geometry, DPI and shell markers.
#
# Used to design and debug the taskbar-strip placement. Run it before and after
# changing taskbar settings to see exactly what the shell reports.

$code = @'
using System;
using System.Runtime.InteropServices;
public class StatLineTaskbarProbe {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct APPBARDATA { public int cbSize; public IntPtr hWnd; public int uCallbackMessage; public int uEdge; public RECT rc; public IntPtr lParam; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string win);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr parent, IntPtr child, string cls, string win);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("shell32.dll")] public static extern IntPtr SHAppBarMessage(int msg, ref APPBARDATA data);
  [DllImport("user32.dll")] public static extern bool SystemParametersInfo(int a, int b, ref RECT r, int c);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public static string Report() {
    IntPtr tb = FindWindow("Shell_TrayWnd", null);
    if (tb == IntPtr.Zero) return "taskbar: NOT FOUND";
    RECT r; GetWindowRect(tb, out r);
    var d = new APPBARDATA(); d.cbSize = Marshal.SizeOf(d); SHAppBarMessage(5, ref d);
    var st = new APPBARDATA(); st.cbSize = Marshal.SizeOf(st); IntPtr state = SHAppBarMessage(4, ref st);
    RECT work = new RECT(); SystemParametersInfo(48, 0, ref work, 0);
    IntPtr tray = FindWindowEx(tb, IntPtr.Zero, "TrayNotifyWnd", null);
    IntPtr start = FindWindowEx(tb, IntPtr.Zero, "Start", null);
    IntPtr tasklist = FindWindowEx(tb, IntPtr.Zero, "MSTaskSwWClass", null);
    RECT tr = new RECT(); if (tray != IntPtr.Zero) GetWindowRect(tray, out tr);
    RECT sr = new RECT(); if (start != IntPtr.Zero) GetWindowRect(start, out sr);
    return "taskbar hwnd=" + tb + " visible=" + IsWindowVisible(tb) + " rect=(" + r.Left + "," + r.Top + "," + r.Right + "," + r.Bottom + ") wh=(" + (r.Right-r.Left) + "x" + (r.Bottom-r.Top) + ")\n" +
      "ABM_GETTASKBARPOS rc=(" + d.rc.Left + "," + d.rc.Top + "," + d.rc.Right + "," + d.rc.Bottom + ") edge=" + d.uEdge + "\n" +
      "ABM_GETSTATE=" + state + " (1=AUTOHIDE, 2=ALWAYS_ON_TOP)\n" +
      "workarea=(" + work.Left + "," + work.Top + "," + work.Right + "," + work.Bottom + ")\n" +
      "screen=" + GetSystemMetrics(0) + "x" + GetSystemMetrics(1) + " monitors=" + GetSystemMetrics(80) + " dpi=" + GetDpiForWindow(tb) + "\n" +
      "TrayNotifyWnd=" + tray + " rect=(" + tr.Left + "," + tr.Top + "," + tr.Right + "," + tr.Bottom + ")\n" +
      "Start=" + start + " rect=(" + sr.Left + "," + sr.Top + "," + sr.Right + "," + sr.Bottom + ")\n" +
      "MSTaskSwWClass=" + tasklist + " (0 on the Windows 11 XAML taskbar)";
  }
}
'@
Add-Type -TypeDefinition $code -Language CSharp
[StatLineTaskbarProbe]::Report()
$v = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
"windows: $($v.DisplayVersion) build $($v.CurrentBuild).$($v.UBR)"
