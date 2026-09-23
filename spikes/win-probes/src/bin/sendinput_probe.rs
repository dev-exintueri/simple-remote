// Spike only (throwaway).
// sendinput_probe: verify absolute mouse positioning across a multi-monitor virtual desktop.
//
// Sets Per-Monitor-Aware-v2 DPI awareness, enumerates monitors (rects in physical px),
// then for a grid of target points per monitor converts virtual-desktop px -> 0..65535 with
// two formulas and issues an absolute SendInput move (VIRTUALDESK). After each move it reads
// GetCursorPos and prints a CSV row plus a mismatch summary.

use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_MOVE,
    MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

unsafe extern "system" fn enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rc: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let out = &mut *(lparam.0 as *mut Vec<RECT>);
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if GetMonitorInfoW(hmon, &mut mi).as_bool() {
        out.push(mi.rcMonitor);
    }
    BOOL(1) // continue enumeration
}

fn monitors() -> Vec<RECT> {
    let mut rects: Vec<RECT> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut rects as *mut _ as isize),
        );
    }
    rects
}

// Formula A: ((x - vx) * 65535) / (vw - 1)
fn norm_a(x: i32, v0: i32, span: i32) -> i32 {
    if span <= 1 {
        return 0;
    }
    (((x - v0) as i64 * 65535) / (span as i64 - 1)) as i32
}

// Formula B (rounding): ((x - vx) * 65536 + vw/2) / vw
fn norm_b(x: i32, v0: i32, span: i32) -> i32 {
    if span <= 0 {
        return 0;
    }
    (((x - v0) as i64 * 65536 + (span as i64) / 2) / span as i64) as i32
}

fn send_abs(nx: i32, ny: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: nx,
                dy: ny,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
    }
}

fn cursor() -> (i32, i32) {
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

fn targets_for(rc: &RECT) -> Vec<(i32, i32)> {
    let (l, t, r, b) = (rc.left, rc.top, rc.right - 1, rc.bottom - 1);
    let cx = (rc.left + rc.right) / 2;
    let cy = (rc.top + rc.bottom) / 2;
    vec![
        (l, t),
        (r, t),
        (l, b),
        (r, b),
        (cx, cy),
        (l + 1, t + 1),
        (r - 1, b - 1),
        (cx, t),
        (cx, b),
        (l, cy),
        (r, cy),
    ]
}

fn main() {
    unsafe {
        // Per-monitor v2 so all coordinates are physical pixels.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
    eprintln!("virtual_desktop origin=({vx},{vy}) size=({vw}x{vh})");

    let mons = monitors();
    for (i, m) in mons.iter().enumerate() {
        eprintln!(
            "monitor[{i}] rect=({},{})-({},{})",
            m.left, m.top, m.right, m.bottom
        );
    }

    println!("target_x,target_y,formula,nx,ny,got_x,got_y,dx,dy");
    let mut mismatch_a = 0u32;
    let mut mismatch_b = 0u32;
    let mut total = 0u32;

    for m in &mons {
        for (tx, ty) in targets_for(m) {
            for (name, nx, ny) in [
                ("A", norm_a(tx, vx, vw), norm_a(ty, vy, vh)),
                ("B", norm_b(tx, vx, vw), norm_b(ty, vy, vh)),
            ] {
                send_abs(nx, ny);
                let (gx, gy) = cursor();
                let (dx, dy) = (gx - tx, gy - ty);
                println!("{tx},{ty},{name},{nx},{ny},{gx},{gy},{dx},{dy}");
                if dx != 0 || dy != 0 {
                    match name {
                        "A" => mismatch_a += 1,
                        _ => mismatch_b += 1,
                    }
                }
            }
            total += 1;
        }
    }

    eprintln!(
        "points={total} formulaA_mismatches={mismatch_a} formulaB_mismatches={mismatch_b}"
    );
}
