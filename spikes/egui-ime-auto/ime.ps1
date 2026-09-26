# Spike only (throwaway).
# egui-ime-auto: T12 question 2 without a human at the keyboard.
# Drives a running egui_probe window with SendInput key presses that go through the Microsoft
# Korean IME (dubeolsik), and saves a window screenshot after each step so the preedit text,
# committed text and the Ime/Text/Key event lists can be read from the images.
#   powershell -NoProfile -ExecutionPolicy Bypass -File ime.ps1 -OutDir C:\spike\ime [-ToggleHangul]
# Keep this file ASCII: Windows PowerShell 5.1 reads BOM-less files in the ANSI code page, and a
# comment ending in a Hangul syllable can swallow the following line.
param([string]$OutDir = "ime-shots", [switch]$ToggleHangul)

New-Item -ItemType Directory -Force $OutDir | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Threading;

public static class Drive {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT {
        public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Sequential)] struct MOUSEINPUT {
        public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo;
    }
    [StructLayout(LayoutKind.Explicit, Size = 40)] struct INPUT {
        [FieldOffset(0)] public uint type;
        [FieldOffset(8)] public KEYBDINPUT ki;
        [FieldOffset(8)] public MOUSEINPUT mi;
    }
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr c);
    [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] i, int size);
    [DllImport("user32.dll")] static extern uint MapVirtualKey(uint code, uint type);
    [DllImport("user32.dll")] static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);

    const uint KEYUP = 0x0002, LDOWN = 0x0002, LUP = 0x0004;
    public static IntPtr Hwnd;

    public static void Init(IntPtr hwnd) {
        SetProcessDpiAwarenessContext(new IntPtr(-4)); // PER_MONITOR_AWARE_V2, physical px
        Hwnd = hwnd;
        ShowWindow(hwnd, 3); // SW_MAXIMIZE, so the Key event list below the Ime list is on screen
        SetForegroundWindow(hwnd);
        Thread.Sleep(800);
        EnsureForeground();
    }

    // Keys go to whatever window has focus; stop instead of typing into another program.
    public static void EnsureForeground() {
        if (GetForegroundWindow() != Hwnd) throw new Exception("egui_probe is not the foreground window");
    }

    public static double Scale() { return GetDpiForWindow(Hwnd) / 96.0; }

    static void Send(INPUT i) {
        if (SendInput(1, new[] { i }, Marshal.SizeOf(typeof(INPUT))) != 1)
            throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
    }

    public static void Key(ushort vk, bool up) {
        var i = new INPUT(); i.type = 1; // INPUT_KEYBOARD
        i.ki.wVk = vk; i.ki.wScan = (ushort)MapVirtualKey(vk, 0);
        i.ki.dwFlags = up ? KEYUP : 0;
        Send(i);
    }

    public static void Tap(ushort vk, int waitMs) {
        EnsureForeground();
        Key(vk, false); Key(vk, true); Thread.Sleep(waitMs);
    }

    // Letters a-z and space as they sit on a US keyboard; the IME maps them to jamo.
    public static void Type(string keys, int waitMs) {
        foreach (char c in keys) Tap(c == ' ' ? (ushort)0x20 : (ushort)char.ToUpperInvariant(c), waitMs);
    }

    public static void Chord(ushort mod, ushort vk) {
        EnsureForeground();
        Key(mod, false); Key(vk, false); Key(vk, true); Key(mod, true); Thread.Sleep(150);
    }

    // Click at egui points measured from the client area's right edge (fromRight) and top.
    public static void ClickPt(double fromRight, double top) {
        RECT c; GetClientRect(Hwnd, out c);
        var o = new POINT(); ClientToScreen(Hwnd, ref o);
        double s = Scale();
        SetCursorPos(o.X + c.R - (int)(fromRight * s), o.Y + (int)(top * s));
        Thread.Sleep(100);
        EnsureForeground();
        var i = new INPUT(); i.type = 0;
        i.mi.dwFlags = LDOWN; Send(i);
        i.mi.dwFlags = LUP; Send(i);
        Thread.Sleep(250);
    }

    // Screenshot of the side panel (right 480 points of the client area).
    public static void Shot(string path) {
        Thread.Sleep(250); // let egui repaint
        RECT c; GetClientRect(Hwnd, out c);
        var o = new POINT(); ClientToScreen(Hwnd, ref o);
        int w = (int)(480 * Scale());
        using (var bmp = new Bitmap(w, c.B))
        using (var g = Graphics.FromImage(bmp)) {
            g.CopyFromScreen(o.X + c.R - w, o.Y, 0, 0, bmp.Size);
            bmp.Save(path, System.Drawing.Imaging.ImageFormat.Png);
        }
    }
}
'@

$proc = Get-Process egui_probe -ErrorAction Stop | Select-Object -First 1
[Drive]::Init($proc.MainWindowHandle)
"scale=$([Drive]::Scale())"

$VK_CONTROL = 0x11; $VK_A = 0x41; $VK_DELETE = 0x2E; $VK_BACK = 0x08; $VK_HANGUL = 0x15
# egui points: single-line field at y 281, multi-line at y 340, both inside the 465-point side
# panel (measured from screenshots at 125% and 150%). $blank is below the Key event list.
$single = @(200, 281); $multi = @(200, 345); $blank = @(100, 1300)

# Dubeolsik keys for the sentence "wongyeok jiwon teseuteu hangeul ibryeok hwagin".
$sentence = "dnjsrur wldnjs xptmxm gksrmf dlqfur ghkrdls"

function Clear-Field($pos) {
    [Drive]::ClickPt($pos[0], $pos[1])
    [Drive]::Chord($VK_CONTROL, $VK_A)
    [Drive]::Tap($VK_DELETE, 150)
}

Clear-Field $multi
Clear-Field $single
if ($ToggleHangul) { [Drive]::Tap($VK_HANGUL, 300) }

# Single-line: composition visibility and backspace inside a syllable.
[Drive]::Type("dnj", 200)                 # preedit expected: syllable "wo"
[Drive]::Shot("$OutDir\01-single-preedit-wo.png")
[Drive]::Tap($VK_BACK, 300)               # expected: one jamo removed, preedit "u"
[Drive]::Shot("$OutDir\02-single-after-backspace.png")
[Drive]::Type("js", 200)                  # "u" + eo + n -> "won"
[Drive]::Shot("$OutDir\03-single-preedit-won.png")
[Drive]::Type($sentence.Substring(4), 120)
[Drive]::ClickPt($blank[0], $blank[1])    # commit the last syllable
[Drive]::Shot("$OutDir\04-single-done.png")

# Multi-line: the whole sentence, then commit.
[Drive]::ClickPt($multi[0], $multi[1])
[Drive]::Type($sentence, 120)
[Drive]::ClickPt($blank[0], $blank[1])
[Drive]::Shot("$OutDir\05-multi-done.png")
"shots=$OutDir"
