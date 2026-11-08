//! Thin Win32 layer: one window, a software blit and the native file dialogs.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use tiny_skia::Pixmap;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HDC, SRCCOPY,
};
use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_EXPLORER, OFN_FILEMUSTEXIST,
    OFN_NOCHANGEDIR, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetClientRect, SetWindowTextW};

pub fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub fn client_size(hwnd: HWND) -> (u32, u32) {
    unsafe {
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetClientRect(hwnd, &mut r) != 0 {
            ((r.right - r.left).max(1) as u32, (r.bottom - r.top).max(1) as u32)
        } else {
            (1, 1)
        }
    }
}

pub fn set_title(hwnd: HWND, title: &str) {
    unsafe {
        let w = wide(title);
        SetWindowTextW(hwnd, w.as_ptr());
    }
}

/// Copy the RGBA pixmap to the window (or printer) DC as a top-down DIB.
/// The DIB and its memory DC are kept between frames; recreating them (and
/// allocating a fresh bitmap) on every mouse move was the main source of
/// choppy painting in the first version.
pub fn blit(hdc: HDC, pixmap: &Pixmap) {
    unsafe {
        let w = pixmap.width() as i32;
        let h = pixmap.height() as i32;
        if w <= 0 || h <= 0 {
            return;
        }
        struct BlitCache {
            w: i32,
            h: i32,
            mem: HDC,
            bmp: isize,
            old: isize,
            bits: *mut u8,
        }
        static mut CACHE: Option<BlitCache> = None;
        let slot = &mut *std::ptr::addr_of_mut!(CACHE);
        let recreate = slot
            .as_ref()
            .map(|c| c.w != w || c.h != h)
            .unwrap_or(true);
        if recreate {
            if let Some(old) = slot.take() {
                SelectObject(old.mem, old.old);
                DeleteObject(old.bmp);
                DeleteDC(old.mem);
            }
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            let mut header: BITMAPINFOHEADER = std::mem::zeroed();
            header.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            header.biWidth = w;
            header.biHeight = -h;
            header.biPlanes = 1;
            header.biBitCount = 32;
            header.biCompression = BI_RGB;
            bmi.bmiHeader = header;
            let mem = CreateCompatibleDC(hdc);
            if mem == 0 {
                return;
            }
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, 0, 0);
            if bmp == 0 || bits.is_null() {
                DeleteDC(mem);
                return;
            }
            let old = SelectObject(mem, bmp);
            *slot = Some(BlitCache {
                w,
                h,
                mem,
                bmp,
                old,
                bits: bits as *mut u8,
            });
        }
        let Some(cache) = slot.as_mut() else { return };
        let src = pixmap.data();
        let dst = std::slice::from_raw_parts_mut(cache.bits, (w * h * 4) as usize);
        // RGBA (premultiplied) -> BGRA, which is what a BI_RGB DIB expects.
        for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 255;
        }
        BitBlt(hdc, 0, 0, w, h, cache.mem, 0, 0, SRCCOPY);
    }
}

fn filter_string(name: &str, ext: &str) -> Vec<u16> {
    let s = format!("{name} (*.{ext})\0*.{ext}\0所有文件 (*.*)\0*.*\0\0");
    s.encode_utf16().collect()
}

pub fn run_dialog(hwnd: HWND, save: bool, title: &str, ext: &str, default_name: &str) -> Option<PathBuf> {
    unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture();
        let mut buf = [0u16; 2048];
        let name: Vec<u16> = OsStr::new(default_name).encode_wide().collect();
        for (i, c) in name.iter().take(buf.len() - 1).enumerate() {
            buf[i] = *c;
        }
        let filter = filter_string("图形 / 代码文件", ext);
        let title_w = wide(title);
        let ext_w = wide(ext);
        let mut ofn: OPENFILENAMEW = std::mem::zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.hwndOwner = hwnd;
        ofn.lpstrFilter = filter.as_ptr();
        ofn.lpstrFile = buf.as_mut_ptr();
        ofn.nMaxFile = buf.len() as u32;
        ofn.lpstrTitle = title_w.as_ptr();
        ofn.lpstrDefExt = ext_w.as_ptr();
        let mut flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if save {
            flags |= OFN_OVERWRITEPROMPT;
        } else {
            flags |= OFN_FILEMUSTEXIST;
        }
        ofn.Flags = flags;
        let ok = if save {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        };
        if ok == 0 {
            return None;
        }
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        let s = String::from_utf16_lossy(&buf[..end]);
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
}

pub fn open_file_dialog(hwnd: HWND, title: &str, filter_name: &str, ext: &str) -> Option<PathBuf> {
    let _ = filter_name;
    run_dialog(hwnd, false, title, ext, "")
}

pub fn save_file_dialog(
    hwnd: HWND,
    title: &str,
    filter_name: &str,
    ext: &str,
    default_name: &str,
) -> Option<PathBuf> {
    let _ = filter_name;
    run_dialog(hwnd, true, title, ext, default_name)
}

pub fn destroy_window(hwnd: HWND) {
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
    }
}

