#![allow(clippy::too_many_arguments)]

mod app;
mod codegen;
mod dxf;
mod foam;
mod gfx;
mod model;
mod platform;

use std::path::PathBuf;

use app::{App, CursorKind};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetDC, InvalidateRect, ReleaseDC, ScreenToClient, UpdateWindow, HDC,
    PAINTSTRUCT,
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
                if !app.screenshot_mode {
                    app.render();
                }
                platform::blit(hdc, &app.painter.pixmap);
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
    UpdateWindow(hwnd);
    hwnd
}

fn parse_args() -> (Option<PathBuf>, Option<String>, Option<usize>, bool, bool, bool) {
    let mut screenshot = None;
    let mut demo_dialog = None;
    let mut demo_menu = None;
    let mut selftest = false;
    let mut uitest = false;
    let mut bench = false;
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
        } else if a == "--selftest" {
            selftest = true;
        } else if a == "--uitest" {
            uitest = true;
        } else if a == "--bench" {
            bench = true;
        }
    }
    (screenshot, demo_dialog, demo_menu, selftest, uitest, bench)
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
    let (screenshot, demo_dialog, demo_menu, do_selftest, do_uitest, do_bench) = parse_args();
    if do_selftest {
        std::process::exit(selftest());
    }
    unsafe {
        SetProcessDPIAware();
    }
    let hwnd = unsafe {
        create_main_window("超越数控线切割编程系统 - 示例零件", 1360, 860)
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
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0, 0, 0);
            if r <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
