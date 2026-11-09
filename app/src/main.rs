#![allow(clippy::too_many_arguments)]

mod app;
mod codegen;
mod commands;
mod dxf;
mod foam;
mod gfx;
mod model;
mod platform;
mod spatial;

use std::path::PathBuf;

use app::{App, CursorKind};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetDC, GetUpdateRect, InvalidateRect, ReleaseDC, ScreenToClient,
    UpdateWindow, HDC, PAINTSTRUCT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

static mut APP_PTR: *mut App = std::ptr::null_mut();

#[link(name = "user32")]
extern "system" {
    fn SetProcessDPIAware() -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    platform::wide(s)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let app = APP_PTR;
    let app = if app.is_null() { None } else { Some(&mut *app) };
    match msg {
        WM_PAINT => {
            if let Some(app) = app {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                let empty = ps.rcPaint.right <= ps.rcPaint.left
                    || ps.rcPaint.bottom <= ps.rcPaint.top;
                if !app.screenshot_mode && !empty {
                    let now = std::time::Instant::now();
                    if let Some(prev) = app.perf_last_paint {
                        let gap = now.duration_since(prev).as_secs_f64() * 1000.0;
                        if gap > app.perf_max_gap_ms {
                            app.perf_max_gap_ms = gap;
                        }
                    }
                    app.perf_last_paint = Some(now);
                    app.perf_paints += 1;
                    app.render();
                    platform::blit(hdc, &app.painter.pixmap);
                }
                let _ = hdc;
                EndPaint(hwnd, &ps);
            } else {
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_SIZE => {
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_MOUSEMOVE => {
            if let Some(app) = app {
                app.perf_input_events += 1;
                let x = (lp & 0xffff) as i16 as f32;
                let y = ((lp >> 16) & 0xffff) as i16 as f32;
                app.on_mouse_move(x, y);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONDOWN => {
            if let Some(app) = app {
                let x = (lp & 0xffff) as i16 as f32;
                let y = ((lp >> 16) & 0xffff) as i16 as f32;
                let shift = (wp & 0x0004) != 0;
                SetCapture(hwnd);
                app.on_left_down(x, y, shift);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONUP => {
            if let Some(app) = app {
                let x = (lp & 0xffff) as i16 as f32;
                let y = ((lp >> 16) & 0xffff) as i16 as f32;
                app.on_left_up(x, y);
                ReleaseCapture();
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            if let Some(app) = app {
                app.on_double_click();
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_RBUTTONDOWN => {
            if let Some(app) = app {
                let x = (lp & 0xffff) as i16 as f32;
                let y = ((lp >> 16) & 0xffff) as i16 as f32;
                app.on_right_down(x, y);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_MBUTTONDOWN => {
            if let Some(app) = app {
                let x = (lp & 0xffff) as i16 as f32;
                let y = ((lp >> 16) & 0xffff) as i16 as f32;
                SetCapture(hwnd);
                app.on_middle_down(x, y);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_MBUTTONUP => {
            if let Some(app) = app {
                app.on_middle_up();
                ReleaseCapture();
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_MOUSEWHEEL => {
            if let Some(app) = app {
                let delta = ((wp >> 16) & 0xffff) as i16 as i32;
                let mut pt = POINT {
                    x: (lp & 0xffff) as i16 as i32,
                    y: ((lp >> 16) & 0xffff) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut pt);
                app.on_wheel(pt.x as f32, pt.y as f32, delta);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_KEYDOWN => {
            if let Some(app) = app {
                let ctrl = GetKeyState(VK_CONTROL as i32) < 0;
                let shift = GetKeyState(VK_SHIFT as i32) < 0;
                app.on_key(wp as u32, ctrl, shift);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_CHAR => {
            if let Some(app) = app {
                let c = wp as u32;
                if c >= 32 && c != 127 {
                    if let Some(ch) = char::from_u32(c) {
                        app.on_char(ch);
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
            }
            0
        }
        WM_SETCURSOR => {
            if let Some(app) = app {
                let cursor = match app.cursor_kind() {
                    CursorKind::Arrow => IDC_ARROW,
                    CursorKind::Cross => IDC_CROSS,
                    CursorKind::Hand => IDC_HAND,
                    CursorKind::IBeam => IDC_IBEAM,
                    CursorKind::SizeAll => IDC_SIZEALL,
                };
                SetCursor(LoadCursorW(0, cursor));
                return 1;
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_GETMINMAXINFO => {
            let mmi = &mut *(lp as *mut MINMAXINFO);
            mmi.ptMinTrackSize.x = 1000;
            mmi.ptMinTrackSize.y = 700;
            0
        }
        WM_CLOSE => {
            if let Some(app) = app {
                app.request_close();
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn create_main_window(title: &str, client_w: i32, client_h: i32) -> HWND {
    let hinst = GetModuleHandleW(std::ptr::null());
    let class_name = wide("ChaoyueCNCWindow");
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: LoadIconW(0, IDI_APPLICATION),
        hCursor: LoadCursorW(0, IDC_ARROW),
        hbrBackground: 0,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    RegisterClassW(&wc);
    let mut rect = RECT { left: 0, top: 0, right: client_w, bottom: client_h };
    AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, 0);
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let x = ((sw - w) / 2).max(0);
    let y = ((sh - h) / 2 - 16).max(0);
    let title_w = wide(title);
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        title_w.as_ptr(),
        WS_OVERLAPPEDWINDOW,
        x,
        y,
        w,
        h,
        0,
        0,
        hinst,
        std::ptr::null_mut(),
    );
    ShowWindow(hwnd, SW_SHOW);
    hwnd
}

/// Run the normal editor loop. Input is drained first, then the newest state is
/// presented at a rate chosen for the display. Clicks and keys are presented
/// immediately; continuous motion (mouse moves, wheel and scrollbar drags) is
/// coalesced into the next display frame, which is now cheap enough to keep up
/// with every notch instead of making the person wait for a full redraw.
fn run_message_loop(hwnd: HWND) {
    use std::time::{Duration, Instant};
    const MAX_DRAIN: usize = 512;
    let refresh = platform::display_refresh_rate(hwnd).clamp(30, 360);
    let frame_interval =
        Duration::from_micros(1_000_000 / refresh as u64).max(Duration::from_micros(2000));
    let mut last_frame = Instant::now() - frame_interval;
    // Paint-related wake-ups are deliberately excluded: WM_PAINT is handled by
    // UpdateWindow on our own schedule, so waiting for it would spin.
    let wake_mask = QS_ALLINPUT & !QS_PAINT;
    let mut urgent = false;
    unsafe {
        loop {
            let mut quit = false;
            for _ in 0..MAX_DRAIN {
                let mut msg: MSG = std::mem::zeroed();
                if PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) == 0 {
                    break;
                }
                if msg.message == WM_QUIT {
                    quit = true;
                    break;
                }
                if msg.message == WM_PAINT {
                    continue;
                }
                if matches!(
                    msg.message,
                    WM_LBUTTONDOWN
                        | WM_LBUTTONUP
                        | WM_LBUTTONDBLCLK
                        | WM_RBUTTONDOWN
                        | WM_RBUTTONUP
                        | WM_MBUTTONDOWN
                        | WM_MBUTTONUP
                        | WM_KEYDOWN
                        | WM_KEYUP
                        | WM_CHAR
                ) {
                    urgent = true;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if quit {
                break;
            }
            let mut update: RECT = std::mem::zeroed();
            if GetUpdateRect(hwnd, &mut update, 0) != 0 {
                let elapsed = last_frame.elapsed();
                if urgent || elapsed >= frame_interval {
                    // Time the frame from its start, so the time spent drawing
                    // counts toward the display period instead of being added
                    // on top of it.
                    last_frame = Instant::now();
                    UpdateWindow(hwnd);
                    urgent = false;
                    continue;
                }
                // Wait only until the next frame is due; any click, key or
                // wheel arriving meanwhile wakes the loop and is handled at
                // once instead of being delayed by a sleep.
                let remain = frame_interval.saturating_sub(elapsed);
                let ms = ((remain.as_micros() + 999) / 1000).clamp(1, u32::MAX as u128) as u32;
                let _ = MsgWaitForMultipleObjectsEx(0, std::ptr::null(), ms, wake_mask, 0);
            } else {
                let _ = MsgWaitForMultipleObjectsEx(
                    0,
                    std::ptr::null(),
                    u32::MAX,
                    wake_mask,
                    0,
                );
            }
        }
    }
}

fn parse_args() -> (
    Option<PathBuf>,
    Option<String>,
    Option<usize>,
    Option<(i32, i32)>,
    bool,
    bool,
    bool,
    bool,
    Option<usize>,
) {
    let mut screenshot = None;
    let mut demo_dialog = None;
    let mut demo_menu = None;
    let mut window_size = None;
    let mut selftest = false;
    let mut uitest = false;
    let mut bench = false;
    let mut bench_input = false;
    let mut bench_large = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--screenshot" {
            screenshot = it.next().map(PathBuf::from);
        } else if let Some(v) = a.strip_prefix("--screenshot=") {
            screenshot = Some(PathBuf::from(v));
        } else if a == "--demo-dialog" {
            demo_dialog = it.next();
        } else if let Some(v) = a.strip_prefix("--demo-dialog=") {
            demo_dialog = Some(v.to_string());
        } else if a == "--demo-menu" {
            demo_menu = it.next().and_then(|s| s.parse::<usize>().ok());
        } else if a == "--size" {
            window_size = it.next().and_then(|s| parse_size(&s));
        } else if let Some(v) = a.strip_prefix("--size=") {
            window_size = parse_size(v);
        } else if a == "--selftest" {
            selftest = true;
        } else if a == "--uitest" {
            uitest = true;
        } else if a == "--bench" {
            bench = true;
        } else if a == "--bench-input" {
            bench_input = true;
        } else if a == "--bench-large" {
            bench_large = it
                .next()
                .and_then(|s| s.parse::<usize>().ok())
                .or(Some(20_000));
        } else if let Some(v) = a.strip_prefix("--bench-large=") {
            bench_large = v.parse::<usize>().ok().or(Some(20_000));
        }
    }
    (
        screenshot,
        demo_dialog,
        demo_menu,
        window_size,
        selftest,
        uitest,
        bench,
        bench_input,
        bench_large,
    )
}

/// A dense, spread-out drawing used to measure interactions at scale: one
/// line or circle every few millimetres, like a detailed imported DXF.
fn stress_doc(n: usize) -> model::Doc {
    use model::{Doc, Kind, Pt};
    let mut doc = Doc::new();
    doc.title = format!("压力测试-{n}.cnc");
    doc.part = "STRESS".to_string();
    let side = (n as f64).sqrt().ceil() as i64;
    let step = 4.0f32;
    let x0 = -(side as f32) * step * 0.5;
    let y0 = -(side as f32) * step * 0.5;
    for i in 0..n as i64 {
        let (col, row) = (i % side, i / side);
        let x = x0 + col as f32 * step;
        let y = y0 + row as f32 * step;
        match i % 5 {
            0 => {
                doc.add(Kind::Circle {
                    c: Pt::new(x, y),
                    r: step * 0.32,
                });
            }
            1 => {
                doc.add(Kind::Arc {
                    c: Pt::new(x, y),
                    r: step * 0.40,
                    a0: 0.35,
                    sweep: std::f32::consts::PI * 1.35,
                });
            }
            2 => {
                doc.add(Kind::Point { p: Pt::new(x, y) });
            }
            _ => {
                doc.add(Kind::Line {
                    a: Pt::new(x - step * 0.45, y - step * 0.35),
                    b: Pt::new(x + step * 0.45, y + step * 0.35),
                });
            }
        }
    }
    doc
}

fn parse_size(s: &str) -> Option<(i32, i32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let w = w.parse::<i32>().ok()?.clamp(1000, 7680);
    let h = h.parse::<i32>().ok()?.clamp(700, 4320);
    Some((w, h))
}

fn selftest() -> i32 {
    use codegen::{build_job, generate, validate, CodeKind, CodeOptions};
    use foam::FoamOptions;
    use model::{Doc, Kind, Pt};
    let (doc, outer) = Doc::demo();
    let job = match build_job(&doc, &outer, Pt::new(-90.0, -77.5), 0.09, -1) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("SELFTEST FAIL build_job: {e}");
            return 1;
        }
    };
    println!(
        "outer cut path: {} segments, {:.3} mm, closed={}",
        job.paths.iter().map(|p| p.segs.len()).sum::<usize>(),
        job.length(),
        job.paths.first().map(|p| p.closed).unwrap_or(false)
    );
    let all = doc.all_ids();
    let all_job = match build_job(&doc, &all, Pt::new(-90.0, -77.5), 0.09, -1) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("SELFTEST FAIL all paths: {e}");
            return 1;
        }
    };
    println!("all selected: {} paths, {:.3} mm", all_job.paths.len(), all_job.length());
    for kind in [CodeKind::ThreeB, CodeKind::GCode, CodeKind::Mach3] {
        let opt = CodeOptions {
            kind,
            pierce: Pt::new(-90.0, -77.5),
            comp: 0.09,
            side: -1,
            part: "SAMPLE".to_string(),
            optimize: true,
            feed: 180.0,
            rapid: 3000.0,
        };
        match generate(&doc, &all, &opt) {
            Ok(out) => {
                match validate(&out.text, out.kind) {
                    Ok(msg) => println!(
                        "{}: {} lines, {} - {}",
                        kind.label(),
                        out.line_count,
                        out.path_count,
                        msg
                    ),
                    Err(e) => {
                        eprintln!("SELFTEST FAIL validate: {e}");
                        return 1;
                    }
                }
                println!("--- {} sample ---", kind.label());
                for line in out.text.lines().take(6) {
                    println!("{line}");
                }
            }
            Err(e) => {
                eprintln!("SELFTEST FAIL generate: {e}");
                return 1;
            }
        }
    }

    // Foam: two-axis and four-axis Mach3 output, path-order optimisation.
    for four_axis in [false, true] {
        let opt = FoamOptions {
            four_axis,
            side: -1,
            start: Pt::new(-140.0, -95.0),
            kerf: 0.4,
            feed: 900.0,
            rapid: 4000.0,
            lead: 3.0,
            taper_x: if four_axis { 2.0 } else { 0.0 },
            taper_y: 0.0,
            continuous: false,
            optimize: true,
            part: "FOAM-SAMPLE".to_string(),
        };
        match foam::generate(&doc, &all, &opt) {
            Ok(out) => {
                match validate(&out.text, CodeKind::Mach3) {
                    Ok(msg) => println!(
                        "foam {}: {} lines, {} paths, {:.1} mm cut, {:.1} mm travel, {:.1} min - {}",
                        if four_axis { "4-axis" } else { "2-axis" },
                        out.line_count,
                        out.job.paths.len(),
                        out.cut_len,
                        out.travel_len,
                        out.minutes,
                        msg
                    ),
                    Err(e) => {
                        eprintln!("SELFTEST FAIL foam validate: {e}");
                        return 1;
                    }
                }
                if four_axis && (!out.text.contains(" A") || !out.text.contains(" B")) {
                    eprintln!("SELFTEST FAIL foam 4-axis missing A/B words");
                    return 1;
                }
                if !four_axis && out.text.contains(" A") {
                    eprintln!("SELFTEST FAIL foam 2-axis unexpectedly has A axis");
                    return 1;
                }
            }
            Err(e) => {
                eprintln!("SELFTEST FAIL foam generate: {e}");
                return 1;
            }
        }
    }
    let unoptimized = foam::generate(
        &doc,
        &all,
        &FoamOptions {
            four_axis: false,
            side: -1,
            start: Pt::new(-140.0, -95.0),
            kerf: 0.4,
            feed: 900.0,
            rapid: 4000.0,
            lead: 3.0,
            taper_x: 0.0,
            taper_y: 0.0,
            continuous: false,
            optimize: false,
            part: "FOAM-RAW".to_string(),
        },
    );
    if let Ok(raw) = unoptimized {
        println!("foam unoptimized travel: {:.1} mm", raw.travel_len);
    }

    // A mirrored arc must keep its reflected start and reverse its sweep.
    let mut mirror_doc = Doc::new();
    let id = mirror_doc.add(Kind::Arc {
        c: Pt::new(0.0, 0.0),
        r: 10.0,
        a0: 0.0,
        sweep: std::f32::consts::FRAC_PI_2,
    });
    if let Some(e) = mirror_doc.entities.iter_mut().find(|e| e.id == id) {
        let before_start = e.start().unwrap();
        let before_end = e.end().unwrap();
        e.mirror(Pt::new(0.0, 0.0), Pt::new(0.0, 1.0));
        let after_start = e.start().unwrap();
        let after_end = e.end().unwrap();
        let expected_start = Pt::new(-before_start.x, before_start.y);
        let expected_end = Pt::new(-before_end.x, before_end.y);
        if after_start.dist(expected_start) > 1e-3 || after_end.dist(expected_end) > 1e-3 {
            eprintln!("SELFTEST FAIL mirrored arc endpoints");
            return 1;
        }
        if let Kind::Arc { sweep, .. } = e.kind {
            if sweep >= 0.0 {
                eprintln!("SELFTEST FAIL mirrored arc did not reverse direction");
                return 1;
            }
        }
        println!("mirror arc: reflected and reversed OK");
    }

    let sample = "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0\n20\n0\n11\n100\n21\n0\n0\nCIRCLE\n10\n50\n20\n50\n40\n10\n0\nARC\n10\n0\n20\n0\n40\n20\n50\n0\n51\n90\n0\nLWPOLYLINE\n90\n2\n70\n0\n10\n0\n20\n0\n42\n0.4142\n10\n10\n20\n0\n0\nENDSEC\n0\nEOF";
    match crate::dxf::import(sample) {
        Ok(v) if v.len() == 4 => println!("DXF import: {} entities OK", v.len()),
        Ok(v) => {
            eprintln!("SELFTEST FAIL dxf count {}", v.len());
            return 1;
        }
        Err(e) => {
            eprintln!("SELFTEST FAIL dxf: {e}");
            return 1;
        }
    }

    // Spatial index: compare a grid query with a brute-force bounds scan and
    // exercise handle lookup, add, translate and remove invalidation.
    {
        use model::BBox;
        let mut stress = Doc::new();
        for i in 0..2000 {
            let x = (i % 50) as f32 * 4.0;
            let y = (i / 50) as f32 * 4.0;
            stress.add(Kind::Line {
                a: Pt::new(x, y),
                b: Pt::new(x + 3.0, y + 2.0),
            });
        }
        let rect = BBox {
            min: Pt::new(20.0, 16.0),
            max: Pt::new(90.0, 60.0),
        };
        let mut got = Vec::new();
        stress.query_rect(rect, &mut got);
        let mut expected: Vec<u64> = stress
            .entities
            .iter()
            .filter(|e| e.bbox().intersects(&rect))
            .map(|e| e.id)
            .collect();
        got.sort_unstable();
        expected.sort_unstable();
        if got != expected {
            eprintln!("SELFTEST FAIL spatial query mismatch");
            return 1;
        }
        let id = stress.entities[0].id;
        if stress.pick(Pt::new(0.0, 0.0), 1.0) != Some(id) {
            eprintln!("SELFTEST FAIL spatial pick");
            return 1;
        }
        stress.translate_ids(&[id], Pt::new(500.0, 500.0));
        if stress.pick(Pt::new(0.0, 0.0), 1.0) == Some(id) {
            eprintln!("SELFTEST FAIL spatial index kept a moved entity in place");
            return 1;
        }
        if stress.pick(Pt::new(500.0, 500.0), 1.0) != Some(id) {
            eprintln!("SELFTEST FAIL spatial index lost a moved entity");
            return 1;
        }
        stress.remove_ids(&[id]);
        if stress.get(id).is_some() {
            eprintln!("SELFTEST FAIL handle map survived remove");
            return 1;
        }
        println!("spatial index: 2000 entities query/pick/translate/remove OK");
    }
    let saved = doc.save_text();
    match Doc::from_text(&saved) {
        Ok(loaded) => {
            if loaded.entities.len() != doc.entities.len() {
                eprintln!("SELFTEST FAIL save/load entity count");
                return 1;
            }
            println!("save/load: {} entities OK", loaded.entities.len());
        }
        Err(e) => {
            eprintln!("SELFTEST FAIL save/load: {e}");
            return 1;
        }
    }
    println!("SELFTEST OK");
    0
}
fn pump_messages() {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, 0, 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn ui_test(app: &mut App) -> i32 {
    use app::{Act, Dialog, Tool};
    app.screenshot_mode = true;
    app.render();
    // The shared command table must drive the status-bar command line.
    app.command = "line".to_string();
    app.on_key(13, false, false);
    if app.tool != Tool::Line {
        eprintln!("UITEST FAIL command 'line' did not select the line tool");
        return 1;
    }
    app.command = "直线".to_string();
    app.on_key(13, false, false);
    if app.tool != Tool::Line {
        eprintln!("UITEST FAIL Chinese command alias did not resolve");
        return 1;
    }
    app.command = "?".to_string();
    app.on_key(13, false, false);
    if !matches!(app.dialog, Some(Dialog::Message { .. })) {
        eprintln!("UITEST FAIL command help did not open");
        return 1;
    }
    app.on_key(27, false, false);
    app.do_act(Act::SetTool(Tool::Select));
    let Some((x, y)) = app.menu_tab_center(6) else {
        eprintln!("UITEST FAIL no menu tab");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    if app.menu_open != Some(6) {
        eprintln!("UITEST FAIL menu did not open");
        return 1;
    }
    app.render();
    let Some((x, y)) = app.hit_center(Act::CutProgram) else {
        eprintln!("UITEST FAIL cut menu item not found");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    if !matches!(app.dialog, Some(Dialog::Cut(_))) {
        eprintln!("UITEST FAIL cut dialog did not open");
        return 1;
    }
    app.render();
    let Some((x, y)) = app.dialog_button_center(0) else {
        eprintln!("UITEST FAIL generate button not found");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    if app.last_code.is_none() {
        eprintln!("UITEST FAIL no code generated");
        return 1;
    }
    app.render();
    let Some((x, y)) = app.dialog_button_center(2) else {
        eprintln!("UITEST FAIL code close button not found");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let before = app.doc.entities.len();
    app.do_act(Act::SetTool(Tool::Line));
    app.render();
    let p1 = (app.canvas.x + 80.0, app.canvas.y + 80.0);
    let p2 = (app.canvas.x + 180.0, app.canvas.y + 120.0);
    app.on_left_down(p1.0, p1.1, false);
    app.on_left_up(p1.0, p1.1);
    app.on_left_down(p2.0, p2.1, false);
    app.on_left_up(p2.0, p2.1);
    if app.doc.entities.len() != before + 1 {
        eprintln!(
            "UITEST FAIL line not drawn ({} -> {})",
            before,
            app.doc.entities.len()
        );
        return 1;
    }
    app.do_act(Act::Undo);
    if app.doc.entities.len() != before {
        eprintln!("UITEST FAIL undo did not restore");
        return 1;
    }
    // Settings dialog: open from the 选项 menu, switch tab, toggle a flag, apply.
    let Some((x, y)) = app.menu_tab_center(5) else {
        eprintln!("UITEST FAIL options menu missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let Some((x, y)) = app.hit_center(Act::Settings) else {
        eprintln!("UITEST FAIL settings item missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if !matches!(app.dialog, Some(Dialog::Settings(_))) {
        eprintln!("UITEST FAIL settings dialog missing");
        return 1;
    }
    let Some((x, y)) = app.dialog_hit_center(Act::DlgTab(1)) else {
        eprintln!("UITEST FAIL settings display tab missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let old_center = app.settings.show_center;
    let Some((x, y)) = app.dialog_hit_center(Act::DlgToggle(0)) else {
        eprintln!("UITEST FAIL center toggle missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let Some((x, y)) = app.dialog_button_center(0) else {
        eprintln!("UITEST FAIL settings ok missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if app.settings.show_center == old_center {
        eprintln!("UITEST FAIL settings toggle not applied");
        return 1;
    }
    // File information dialog and text replacement.
    let Some((x, y)) = app.menu_tab_center(0) else {
        eprintln!("UITEST FAIL file menu missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let Some((x, y)) = app.hit_center(Act::FileInfo) else {
        eprintln!("UITEST FAIL file info item missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let Some((x, y)) = app.field_center("fi_part") else {
        eprintln!("UITEST FAIL part field missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    for ch in "UITEST".chars() {
        app.on_char(ch);
    }
    app.render();
    let Some((x, y)) = app.dialog_button_center(0) else {
        eprintln!("UITEST FAIL file info ok missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if app.doc.part != "UITEST" {
        eprintln!("UITEST FAIL file info not applied: {}", app.doc.part);
        return 1;
    }
    // Array and offset use the selection.
    app.selected = vec![app.doc.entities[0].id];
    let base = app.doc.entities.len();
    app.do_act(Act::EditArray);
    app.render();
    let Some((x, y)) = app.dialog_button_center(0) else {
        eprintln!("UITEST FAIL array ok missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if app.doc.entities.len() != base + 3 {
        eprintln!(
            "UITEST FAIL array count ({} -> {})",
            base,
            app.doc.entities.len()
        );
        return 1;
    }
    app.do_act(Act::Undo);
    app.render();
    app.selected = vec![app.doc.entities[0].id];
    let base = app.doc.entities.len();
    app.do_act(Act::EditOffset);
    app.render();
    let Some((x, y)) = app.dialog_button_center(0) else {
        eprintln!("UITEST FAIL offset ok missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if app.doc.entities.len() != base + 1 {
        eprintln!("UITEST FAIL offset count");
        return 1;
    }
    app.do_act(Act::Undo);
    app.render();
    // Copy tool with two picked points.
    app.selected = vec![app.doc.entities[0].id];
    let base = app.doc.entities.len();
    app.do_act(Act::SetTool(Tool::Copy));
    app.render();
    let p1 = (app.canvas.x + 30.0, app.canvas.y + 300.0);
    let p2 = (app.canvas.x + 60.0, app.canvas.y + 330.0);
    app.on_left_down(p1.0, p1.1, false);
    app.on_left_up(p1.0, p1.1);
    app.on_left_down(p2.0, p2.1, false);
    app.on_left_up(p2.0, p2.1);
    if app.doc.entities.len() != base + 1 {
        eprintln!("UITEST FAIL copy count");
        return 1;
    }
    app.do_act(Act::Undo);
    // Coordinate entry through the command line.
    let base = app.doc.entities.len();
    app.do_act(Act::SetTool(Tool::Line));
    for ch in "10,10".chars() {
        app.on_char(ch);
    }
    app.on_key(13, false, false);
    for ch in "50,20".chars() {
        app.on_char(ch);
    }
    app.on_key(13, false, false);
    if app.doc.entities.len() != base + 1 {
        eprintln!("UITEST FAIL coordinate line not drawn");
        return 1;
    }
    app.do_act(Act::Undo);
    // Polyline via clicks and a right-click finish.
    let base = app.doc.entities.len();
    app.do_act(Act::SetTool(Tool::Polyline));
    let p1 = (app.canvas.x + 60.0, app.canvas.y + 60.0);
    let p2 = (app.canvas.x + 120.0, app.canvas.y + 100.0);
    let p3 = (app.canvas.x + 200.0, app.canvas.y + 70.0);
    for p in [p1, p2, p3] {
        app.on_left_down(p.0, p.1, false);
        app.on_left_up(p.0, p.1);
    }
    app.on_right_down(p3.0, p3.1);
    if app.doc.entities.len() != base + 2 {
        eprintln!("UITEST FAIL polyline not drawn");
        return 1;
    }
    app.do_act(Act::Undo);
    // Trim: three crossing lines, remove the middle segment of the horizontal one.
    let base = app.doc.entities.len();
    app.do_act(Act::SetTool(Tool::Line));
    for (a, b) in [("0,0", "100,0"), ("30,-50", "30,50"), ("70,-50", "70,50")] {
        for ch in a.chars() {
            app.on_char(ch);
        }
        app.on_key(13, false, false);
        for ch in b.chars() {
            app.on_char(ch);
        }
        app.on_key(13, false, false);
    }
    if app.doc.entities.len() != base + 3 {
        eprintln!("UITEST FAIL trim setup");
        return 1;
    }
    app.do_act(Act::SetTool(Tool::Trim));
    app.render();
    let s = app.w2s_public(model::Pt::new(50.0, 0.0));
    app.on_left_down(s.0, s.1, false);
    app.on_left_up(s.0, s.1);
    if app.doc.entities.len() != base + 4 {
        eprintln!(
            "UITEST FAIL trim count ({} -> {})",
            base + 3,
            app.doc.entities.len()
        );
        return 1;
    }
    app.do_act(Act::Undo);
    // Foam / Mach3 dialog: switch to four-axis, generate and check the output.
    app.settings.foam_four_axis = false;
    app.settings.foam_side = 0;
    let Some((x, y)) = app.menu_tab_center(6) else {
        eprintln!("UITEST FAIL programming menu missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let Some((x, y)) = app.hit_center(Act::FoamProgram) else {
        eprintln!("UITEST FAIL foam menu item missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if !matches!(app.dialog, Some(Dialog::Foam(_))) {
        eprintln!("UITEST FAIL foam dialog did not open");
        return 1;
    }
    let Some((x, y)) = app.dialog_hit_center(Act::DlgCycle(20)) else {
        eprintln!("UITEST FAIL foam mode combo missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    if !matches!(app.dialog, Some(Dialog::Foam(_))) {
        eprintln!("UITEST FAIL foam dialog closed unexpectedly after mode click");
        return 1;
    }
    let Some((x, y)) = app.dialog_button_center(0) else {
        eprintln!("UITEST FAIL foam generate button missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    let Some(code) = app.last_code.as_ref() else {
        eprintln!("UITEST FAIL foam generated no code");
        return 1;
    };
    if code.kind != codegen::CodeKind::Mach3 || !code.text.contains(" A") {
        eprintln!(
            "UITEST FAIL foam code is not four-axis Mach3 (kind={:?}, status={}, text={:?})",
            code.kind,
            app.status,
            code.text.lines().take(10).collect::<Vec<_>>()
        );
        return 1;
    }
    if !matches!(app.dialog, Some(Dialog::Code(_))) {
        eprintln!("UITEST FAIL foam code dialog missing");
        return 1;
    }
    // The code page behaves like a normal scrollable document: wheel over the
    // dialog, paging keys, Home/End and a draggable scrollbar.
    let Some(area) = app.code_page_area() else {
        eprintln!("UITEST FAIL code page area missing");
        return 1;
    };
    let limit = app.code_scroll_limit();
    if limit <= 4.0 {
        eprintln!("UITEST FAIL code page is not scrollable (limit {limit})");
        return 1;
    }
    let (cx, cy) = area.center();
    if app.code_scroll_value().unwrap_or(0.0) != 0.0 {
        eprintln!("UITEST FAIL code page did not start at the top");
        return 1;
    }
    app.on_wheel(cx, cy, -120);
    app.render();
    let after_wheel = app.code_scroll_value().unwrap_or(0.0);
    if after_wheel <= 0.0 {
        eprintln!("UITEST FAIL wheel did not scroll the code page");
        return 1;
    }
    app.on_key(34, false, false); // PageDown
    app.render();
    let after_page = app.code_scroll_value().unwrap_or(0.0);
    if after_page <= after_wheel {
        eprintln!("UITEST FAIL PageDown did not scroll the code page");
        return 1;
    }
    app.on_key(35, false, false); // End
    app.render();
    if (app.code_scroll_value().unwrap_or(0.0) - limit).abs() > 1.0 {
        eprintln!("UITEST FAIL End did not reach the bottom");
        return 1;
    }
    app.on_key(36, false, false); // Home
    app.render();
    if app.code_scroll_value().unwrap_or(0.0) != 0.0 {
        eprintln!("UITEST FAIL Home did not return to the top");
        return 1;
    }
    let Some(thumb) = app.code_scroll_thumb() else {
        eprintln!("UITEST FAIL code scrollbar thumb missing");
        return 1;
    };
    let (tx, ty) = thumb.center();
    let was_screenshot = app.screenshot_mode;
    app.screenshot_mode = false;
    app.on_left_down(tx, ty, false);
    app.on_mouse_move(tx, ty + 90.0);
    app.on_left_up(tx, ty + 90.0);
    app.screenshot_mode = was_screenshot;
    app.render();
    let after_drag = app.code_scroll_value().unwrap_or(0.0);
    if after_drag <= 0.0 || after_drag > limit + 0.5 {
        eprintln!("UITEST FAIL dragging the code scrollbar ({after_drag} of {limit})");
        return 1;
    }
    app.on_key(36, false, false); // Home
    app.render();
    let Some(track) = app.code_scroll_track() else {
        eprintln!("UITEST FAIL code scrollbar track missing");
        return 1;
    };
    app.on_left_down(track.center().0, track.bottom() - 4.0, false);
    app.on_left_up(track.center().0, track.bottom() - 4.0);
    app.render();
    if app.code_scroll_value().unwrap_or(0.0) <= 0.0 {
        eprintln!("UITEST FAIL track click did not page the code view");
        return 1;
    }
    for _ in 0..30 {
        app.on_wheel(cx, cy, 120);
    }
    app.render();
    if app.code_scroll_value().unwrap_or(0.0) != 0.0 {
        eprintln!("UITEST FAIL code scroll did not clamp at the top");
        return 1;
    }
    // Timed proof that scrolling repaints are cheap; also exercises many
    // different line bitmaps, which used to trigger periodic cache rebuilds.
    let scroll_start = std::time::Instant::now();
    for _ in 0..60 {
        app.on_wheel(cx, cy, -120);
        app.render();
    }
    let scroll_ms = scroll_start.elapsed().as_secs_f64() * 1000.0 / 60.0;
    println!("UITEST INFO code scroll frame: {scroll_ms:.2} ms");
    if scroll_ms > 12.0 {
        eprintln!("UITEST FAIL code scrolling is still slow ({scroll_ms:.2} ms/frame)");
        return 1;
    }
    let Some((x, y)) = app.dialog_button_center(2) else {
        eprintln!("UITEST FAIL foam code close button missing");
        return 1;
    };
    app.on_left_down(x, y, false);
    app.on_left_up(x, y);
    app.render();
    println!("UITEST OK");
    0
}

fn main() {
    let (
        screenshot,
        demo_dialog,
        demo_menu,
        window_size,
        do_selftest,
        do_uitest,
        do_bench,
        do_bench_input,
        bench_large,
    ) = parse_args();
    if do_selftest {
        std::process::exit(selftest());
    }
    unsafe {
        SetProcessDPIAware();
    }
    platform::begin_high_res_timer();
    let (win_w, win_h) = window_size.unwrap_or((1360, 860));
    let hwnd = unsafe {
        create_main_window("超越数控线切割编程系统 - 示例零件", win_w, win_h)
    };
    if hwnd == 0 {
        eprintln!("failed to create window");
        std::process::exit(1);
    }
    let mut app = App::new(hwnd);
    app.screenshot_mode = screenshot.is_some();
    if let Some(name) = demo_dialog {
        app.open_demo(&name);
    }
    if let Some(idx) = demo_menu {
        app.menu_open = Some(idx);
    }
    unsafe {
        APP_PTR = &mut app as *mut App;
    }
    if do_uitest {
        let code = ui_test(&mut app);
        unsafe {
            DestroyWindow(hwnd);
        }
        std::process::exit(code);
    }
    if let Some(n) = bench_large {
        app.doc = stress_doc(n);
        app.fitted = false;
        app.render();
        for _ in 0..3 {
            app.render();
        }
        if let Some(path) = &screenshot {
            if let Ok(png) = app.painter.png() {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(path, png);
                println!("stress screenshot saved: {}", path.display());
            }
        }
        let canvas = app.canvas;
        let events = 300;
        let t = std::time::Instant::now();
        for i in 0..events {
            let x = canvas.x + 40.0 + (i * 71 % 900) as f32;
            let y = canvas.y + 30.0 + (i * 53 % 620) as f32;
            app.on_mouse_move(x, y);
        }
        let input = t.elapsed().as_secs_f64() * 1000.0 / events as f64;
        let frames = 30;
        let t = std::time::Instant::now();
        for i in 0..frames {
            app.view_center.x += 0.8 + (i % 3) as f32 * 0.4;
            app.render();
        }
        let frame = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        // A typical working view is zoomed into one detail rather than fitted
        // to the whole plate. Measure that case separately: the spatial index
        // should keep the redraw proportional to what is on screen.
        app.scale *= 4.0;
        app.render();
        let t = std::time::Instant::now();
        for i in 0..frames {
            app.view_center.x += 0.35 + (i % 3) as f32 * 0.2;
            app.render();
        }
        let zoomed = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        // Panning should reuse the last composed frame instead of rebuilding.
        app.scale /= 4.0;
        app.render();
        app.on_middle_down(canvas.x + 20.0, canvas.y + 20.0);
        let t = std::time::Instant::now();
        for _ in 0..frames {
            app.view_center.x += 0.5;
            app.render();
        }
        let pan = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        app.on_middle_up();
        // Dragging a selection should composite the drag snapshot instead of
        // re-rasterizing the whole drawing on every mouse message.
        let drag = if let Some(e) = app.doc.entities.first() {
            let p = e.bbox().center();
            let s = app.w2s_public(p);
            app.selected = vec![e.id];
            app.on_left_down(s.0, s.1, false);
            app.on_mouse_move(s.0 + 8.0, s.1 + 3.0);
            let t = std::time::Instant::now();
            for i in 0..frames {
                app.on_mouse_move(s.0 + 8.0 + i as f32 * 0.7, s.1 + 3.0);
                app.render();
            }
            let ms = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
            app.on_left_up(s.0 + 8.0, s.1 + 3.0);
            Some(ms)
        } else {
            None
        };
        println!(
            "large drawing benchmark: {n} entities, hover+snap {:.3} ms/event, changed-view frame {:.2} ms ({:.0} fps), zoomed changed-view {:.2} ms ({:.0} fps), pan frame {:.2} ms ({:.0} fps), drag frame {}",
            input,
            frame,
            1000.0 / frame.max(0.001),
            zoomed,
            1000.0 / zoomed.max(0.001),
            pan,
            1000.0 / pan.max(0.001),
            drag.map(|ms| format!("{ms:.2} ms ({:.0} fps)", 1000.0 / ms.max(0.001)))
                .unwrap_or_else(|| "n/a".to_string())
        );
        unsafe {
            DestroyWindow(hwnd);
        }
        return;
    }
    if do_bench {
        app.render();
        for _ in 0..20 {
            app.render();
        }
        app.profile = true;
        app.render();
        app.profile = false;
        let hdc: HDC = unsafe { GetDC(hwnd) };
        let t = std::time::Instant::now();
        let frames = 200;
        for _ in 0..frames {
            app.render();
            platform::blit(hdc, &app.painter.pixmap);
        }
        let avg = t.elapsed().as_secs_f64() * 1000.0 / frames as f64;
        unsafe {
            ReleaseDC(hwnd, hdc);
        }
        println!(
            "render + present benchmark: {frames} frames, {:.2} ms/frame ({:.0} fps)",
            avg,
            1000.0 / avg.max(0.001)
        );
        unsafe {
            DestroyWindow(hwnd);
        }
        return;
    }
    if do_bench_input {
        // Feed a realistic burst of mouse movement through the actual Win32
        // message queue and count the frames the paint path actually presents.
        let hwnd_bits = hwnd as usize;
        unsafe {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
        let started = std::time::Instant::now();
        let feeder = std::thread::spawn(move || {
            let hwnd = hwnd_bits as HWND;
            for i in 0..2400usize {
                let x = 460 + (i * 37 % 780) as i16;
                let y = 210 + (i * 23 % 480) as i16;
                let lp = ((y as isize) & 0xffff) << 16 | ((x as isize) & 0xffff);
                unsafe {
                    PostMessageW(hwnd, WM_MOUSEMOVE, 0, lp);
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            unsafe {
                PostMessageW(hwnd, WM_QUIT, 0, 0);
            }
        });
        run_message_loop(hwnd);
        let _ = feeder.join();
        let secs = started.elapsed().as_secs_f64();
        println!(
            "input benchmark: {} mouse messages dispatched, {} paints in {:.2}s = {:.1} fps, max paint gap {:.1} ms",
            app.perf_input_events,
            app.perf_paints,
            secs,
            app.perf_paints as f64 / secs.max(0.001),
            app.perf_max_gap_ms
        );
        unsafe {
            DestroyWindow(hwnd);
        }
        return;
    }
    if let Some(path) = screenshot {
        // Lay out, place the demo cursor over the part, then draw the final
        // picture once. The same frame is presented 120 times so the window is
        // a normal, responsive window (and the picture is stable).
        app.render();
        app.screenshot_pointer();
        app.render();
        for _ in 0..120 {
            pump_messages();
            unsafe {
                let hdc: HDC = GetDC(hwnd);
                platform::blit(hdc, &app.painter.pixmap);
                ReleaseDC(hwnd, hdc);
            }
            std::thread::sleep(std::time::Duration::from_millis(3));
        }
        let png = match app.painter.png() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("png encode failed: {e}");
                std::process::exit(1);
            }
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&path, png) {
            Ok(_) => println!("screenshot saved: {}", path.display()),
            Err(e) => {
                eprintln!("screenshot write failed: {e}");
                std::process::exit(1);
            }
        }
        unsafe {
            DestroyWindow(hwnd);
        }
        return;
    }
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    run_message_loop(hwnd);
}
