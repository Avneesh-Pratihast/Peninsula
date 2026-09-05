use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::Duration;
use tauri::Emitter;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE, HWND_NOTOPMOST, HWND_TOPMOST,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE,
    SW_SHOWNOACTIVATE, WS_EX_LAYERED, WS_EX_TRANSPARENT,
    WS_MAXIMIZEBOX, WS_THICKFRAME,
};

use crate::dpi::{calculate_top_center_anchor, dip_to_physical, get_dpi_for_window};

static HOOK_HWND: AtomicIsize = AtomicIsize::new(0);
static IS_INTERACTIVE: AtomicBool = AtomicBool::new(true);
static CURRENT_PILL_W_DIP: AtomicIsize = AtomicIsize::new(148);
static CURRENT_PILL_H_DIP: AtomicIsize = AtomicIsize::new(32);
static MORPH_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn update_pill_target_size(w_dip: f32, h_dip: f32) {
    CURRENT_PILL_W_DIP.store(w_dip.round() as isize, Ordering::SeqCst);
    CURRENT_PILL_H_DIP.store(h_dip.round() as isize, Ordering::SeqCst);
}

pub fn is_point_in_island(hwnd: HWND, screen_x: i32, screen_y: i32) -> bool {
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return false;
        }

        let win_w = rect.right - rect.left;
        let _win_h = rect.bottom - rect.top;

        if screen_x < rect.left || screen_x >= rect.right || screen_y < rect.top || screen_y >= rect.bottom {
            return false;
        }

        let dpi = get_dpi_for_window(hwnd);
        let pill_w_dip = CURRENT_PILL_W_DIP.load(Ordering::Relaxed) as f32;
        let pill_h_dip = CURRENT_PILL_H_DIP.load(Ordering::Relaxed) as f32;
        let pill_w_px = dip_to_physical(pill_w_dip, dpi);
        let pill_h_px = dip_to_physical(pill_h_dip, dpi);

        if pill_w_px <= 0 || pill_h_px <= 0 {
            return false;
        }

        // Active pill is centered horizontally in the host window and anchored flush at y=0
        let pill_offset_x = (win_w - pill_w_px) / 2;
        let local_x = (screen_x - rect.left) - pill_offset_x;
        let local_y = screen_y - rect.top;

        // Strictly outside the active pill chassis
        if local_x < 0 || local_x >= pill_w_px || local_y < 0 || local_y >= pill_h_px {
            return false;
        }

        let ear_px = dip_to_physical(12.0, dpi);
        let corner_dip = if pill_h_px > dip_to_physical(40.0, dpi) { 24.0 } else { 16.0 };
        let r = dip_to_physical(corner_dip, dpi);

        // Transparent flank below ears
        if local_x < ear_px && local_y > ear_px {
            return false;
        }
        if local_x > (pill_w_px - ear_px) && local_y > ear_px {
            return false;
        }

        // Concave curve of left ear
        if local_x < ear_px && local_y <= ear_px {
            let dx = local_x;
            let dy = ear_px - local_y;
            if dx * dx + dy * dy < ear_px * ear_px {
                return false;
            }
        }

        // Concave curve of right ear
        if local_x > (pill_w_px - ear_px) && local_y <= ear_px {
            let dx = pill_w_px - local_x;
            let dy = ear_px - local_y;
            if dx * dx + dy * dy < ear_px * ear_px {
                return false;
            }
        }

        // Rounded bottom-left corner of the card
        if local_x < (ear_px + r) && local_y > (pill_h_px - r) {
            let dx = (ear_px + r) - local_x;
            let dy = local_y - (pill_h_px - r);
            if dx * dx + dy * dy > r * r {
                return false;
            }
        }

        // Rounded bottom-right corner of the card
        if local_x > (pill_w_px - ear_px - r) && local_y > (pill_h_px - r) {
            let dx = local_x - (pill_w_px - ear_px - r);
            let dy = local_y - (pill_h_px - r);
            if dx * dx + dy * dy > r * r {
                return false;
            }
        }

        true
    }
}

pub fn set_interactive_mode(hwnd: HWND, interactive: bool) {
    let prev = IS_INTERACTIVE.swap(interactive, Ordering::SeqCst);
    if prev == interactive {
        return;
    }
    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let new_ex_style = if interactive {
            (ex_style | (WS_EX_LAYERED.0 as isize)) & !(WS_EX_TRANSPARENT.0 as isize)
        } else {
            ex_style | (WS_EX_TRANSPARENT.0 as isize) | (WS_EX_LAYERED.0 as isize)
        };
        if new_ex_style != ex_style {
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style);
        }
    }
}

pub fn set_interactive_from_ipc(interactive: bool) {
    let hwnd_raw = HOOK_HWND.load(Ordering::SeqCst);
    if hwnd_raw != 0 {
        let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
        set_interactive_mode(hwnd, interactive);
    }
}

pub fn start_hittest_guard(hwnd: HWND) {
    let hwnd_raw = hwnd.0 as isize;
    std::thread::Builder::new()
        .name("isle-hittest-guard".to_string())
        .spawn(move || unsafe {
            let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
            let mut was_inside = false;
            set_interactive_mode(hwnd, false);

            loop {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    let inside = is_point_in_island(hwnd, pt.x, pt.y);
                    if inside != was_inside {
                        was_inside = inside;
                        set_interactive_mode(hwnd, inside);
                    }
                }
                std::thread::sleep(Duration::from_millis(16));
            }
        })
        .expect("failed to spawn hittest guard thread");
}

const WM_NCHITTEST: u32 = 0x0084;
const HTTRANSPARENT: isize = -1;

unsafe extern "system" fn corner_passthrough_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _ref_data: usize,
) -> LRESULT {
    if msg == WM_NCHITTEST {
        let screen_x = (lparam.0 & 0xFFFF) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
        if !is_point_in_island(hwnd, screen_x, screen_y) {
            return LRESULT(HTTRANSPARENT);
        }
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

pub fn attach_corner_subclass(hwnd: HWND) {
    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(corner_passthrough_subclass), 100, 0);
    }
}

pub fn configure_host_window(hwnd: HWND, initial_width_dip: f32, initial_height_dip: f32, inset_dip: f32, _app_handle: tauri::AppHandle) -> Result<(), String> {
    unsafe {
        // Strip resizing borders so Windows never displays drag/resize cursors on window edges
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, style & !(WS_THICKFRAME.0 as isize) & !(WS_MAXIMIZEBOX.0 as isize));
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED);

        // Windows 11 DWM: Disable system window frame rounding and border colors to ensure clean borderless overlay
        let do_not_round: u32 = 1; // DWMWCP_DONOTROUND
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &do_not_round as *const _ as _,
            std::mem::size_of::<u32>() as u32,
        );
        let color_none: u32 = 0xFFFFFFFE; // DWMWA_COLOR_NONE
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color_none as *const _ as _,
            std::mem::size_of::<u32>() as u32,
        );

        HOOK_HWND.store(hwnd.0 as isize, Ordering::SeqCst);

        // Subclass host HWND for per-pixel corner click-through
        attach_corner_subclass(hwnd);

        // Initial positioning to physical top-center
        reposition_window(hwnd, initial_width_dip, initial_height_dip, inset_dip);

        // Start dedicated 60 FPS hittest guard for 100% reliable per-pixel click-through
        start_hittest_guard(hwnd);

        Ok(())
    }
}


pub fn reposition_window(hwnd: HWND, width_dip: f32, height_dip: f32, _inset_dip: f32) {
    update_pill_target_size(width_dip, height_dip);
    unsafe {
        let (x, y, w, h) = calculate_top_center_anchor(hwnd, width_dip, height_dip, 0.0);
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );

        log::info!("Repositioned Isle top-flush notch: x={}, y={}, w={}, h={}", x, y, w, h);
    }
}

pub fn execute_morph_bounds(
    hwnd: HWND,
    target_w_dip: f32,
    target_h_dip: f32,
    _inset_dip: f32,
    duration_ms: u64,
    app_handle: Option<tauri::AppHandle>,
) {
    update_pill_target_size(target_w_dip, target_h_dip);
    let current_gen = MORPH_GEN.fetch_add(1, Ordering::SeqCst) + 1;

    unsafe {
        let (target_x, target_y, target_w_px, target_h_px) =
            calculate_top_center_anchor(hwnd, target_w_dip, target_h_dip, 0.0);

        let mut current_rect = windows::Win32::Foundation::RECT::default();
        let _ = windows::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut current_rect);
        let current_w_px = current_rect.right - current_rect.left;
        let current_h_px = current_rect.bottom - current_rect.top;

        // Size strictly to cover both current and target bounds during animation - zero extra padding!
        let union_w_px = current_w_px.max(target_w_px);
        let union_h_px = current_h_px.max(target_h_px);
        let morph_y = target_y;
        let morph_x = target_x - (union_w_px - target_w_px) / 2;

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            morph_x,
            morph_y,
            union_w_px,
            union_h_px,
            SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
        );

        // Notify webview of morph start
        if let Some(ref handle) = app_handle {
            let _ = handle.emit("isle://morph_start", serde_json::json!({
                "duration_ms": duration_ms,
                "dir": "down",
                "target_width": target_w_dip,
                "target_height": target_h_dip,
            }));
        }

        let hwnd_raw = hwnd.0 as usize;
        let handle_clone = app_handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(duration_ms));
            if MORPH_GEN.load(Ordering::SeqCst) != current_gen {
                return;
            }
            let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                target_x,
                target_y,
                target_w_px,
                target_h_px,
                SWP_NOACTIVATE | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
            );

            if let Some(ref handle) = handle_clone {
                let _ = handle.emit("isle://morph_done", ());
            }
        });
    }
}

pub fn set_window_visibility(hwnd: HWND, visible: bool) {
    unsafe {
        if visible {
            let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE | SWP_NOACTIVATE);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        } else {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE | windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }
}

