# Spike only (throwaway).
# sendinput-sweep: T10 follow-up. Sweeps every pixel column (at the vertical middle) and every
# pixel row (at the horizontal middle) of the virtual desktop, moves the cursor with an absolute
# SendInput (VIRTUALDESK) using formulas A, B and C, reads GetCursorPos back and counts mismatches.
#   A: ((x - vx) * 65535) / (vw - 1)
#   B: ((x - vx) * 65536 + vw / 2) / vw
#   C: ((x - vx) * 65536 + vw - 1) / vw   (ceil; inferred from the T10 CSV)
# Run in a fresh Windows PowerShell 5.1 process so DPI awareness can still be set:
#   powershell -NoProfile -ExecutionPolicy Bypass -File sweep.ps1 [-OutCsv mismatches.csv]
param([string]$OutCsv = "sendinput-sweep-mismatches.csv")

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class Sweep {
    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] struct MOUSEINPUT {
        public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo;
    }
    // INPUT is a union; MOUSEINPUT is its largest member on x64 except padding, so size it explicitly.
    [StructLayout(LayoutKind.Explicit, Size = 40)] struct INPUT {
        [FieldOffset(0)] public uint type; [FieldOffset(8)] public MOUSEINPUT mi;
    }
    [DllImport("user32.dll")] static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] static extern int GetSystemMetrics(int i);
    [DllImport("user32.dll")] static extern uint SendInput(uint n, INPUT[] inputs, int size);
    [DllImport("user32.dll")] static extern bool GetCursorPos(out POINT p);
    [DllImport("user32.dll")] static extern bool SetCursorPos(int x, int y);

    const uint MOVE = 0x0001, ABSOLUTE = 0x8000, VIRTUALDESK = 0x4000;

    static long A(int v, int v0, int span) { return ((long)(v - v0) * 65535) / (span - 1); }
    static long B(int v, int v0, int span) { return ((long)(v - v0) * 65536 + span / 2) / span; }
    static long C(int v, int v0, int span) { return ((long)(v - v0) * 65536 + span - 1) / span; }

    static POINT Move(long nx, long ny) {
        var input = new INPUT[1];
        input[0].type = 0; // INPUT_MOUSE
        input[0].mi.dx = (int)nx; input[0].mi.dy = (int)ny;
        input[0].mi.dwFlags = MOVE | ABSOLUTE | VIRTUALDESK;
        if (SendInput(1, input, Marshal.SizeOf(typeof(INPUT))) != 1)
            throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
        POINT p; GetCursorPos(out p); return p;
    }

    public static List<string> Run(out string summary) {
        bool dpi = SetProcessDpiAwarenessContext(new IntPtr(-4)); // PER_MONITOR_AWARE_V2
        int vx = GetSystemMetrics(76), vy = GetSystemMetrics(77);
        int vw = GetSystemMetrics(78), vh = GetSystemMetrics(79);
        POINT start; GetCursorPos(out start);

        var names = new[] { "A", "B", "C" };
        var fns = new Func<int, int, int, long>[] { A, B, C };
        var miss = new int[3];
        var rows = new List<string>();
        rows.Add("axis,target_x,target_y,formula,nx,ny,got_x,got_y,dx,dy");
        int midX = vx + vw / 2, midY = vy + vh / 2, points = 0;

        for (int pass = 0; pass < 2; pass++) {
            int count = pass == 0 ? vw : vh;
            for (int i = 0; i < count; i++) {
                int tx = pass == 0 ? vx + i : midX;
                int ty = pass == 0 ? midY : vy + i;
                points++;
                for (int f = 0; f < 3; f++) {
                    long nx = fns[f](tx, vx, vw), ny = fns[f](ty, vy, vh);
                    POINT g = Move(nx, ny);
                    if (g.X != tx || g.Y != ty) {
                        miss[f]++;
                        rows.Add(string.Format("{0},{1},{2},{3},{4},{5},{6},{7},{8},{9}",
                            pass == 0 ? "x" : "y", tx, ty, names[f], nx, ny, g.X, g.Y, g.X - tx, g.Y - ty));
                    }
                }
            }
        }
        SetCursorPos(start.X, start.Y);
        summary = string.Format(
            "dpi_v2_set={0} virtual_desktop origin=({1},{2}) size=({3}x{4}) points={5} formulaA_mismatches={6} formulaB_mismatches={7} formulaC_mismatches={8}",
            dpi, vx, vy, vw, vh, points, miss[0], miss[1], miss[2]);
        return rows;
    }
}
'@

$summary = $null
$rows = [Sweep]::Run([ref]$summary)
$rows | Set-Content -Encoding utf8 $OutCsv
$summary
"mismatch_rows=$($rows.Count - 1) csv=$OutCsv"
