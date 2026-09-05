use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GWL_STYLE, WS_CAPTION,
};

pub fn start_fullscreen_guard(app_handle: AppHandle, host_hwnd: HWND) {
    let hwnd_raw = host_hwnd.0 as isize;
    std::thread::Builder::new()
        .name("isle-fullscreen-guard".to_string())
        .spawn(move || unsafe {
            let mut is_fullscreen = false;

            loop {
                let fg_hwnd = GetForegroundWindow();
                if !fg_hwnd.is_invalid() && fg_hwnd != HWND(hwnd_raw as _) {
                    let mut win_rect = windows::Win32::Foundation::RECT::default();
                    if GetWindowRect(fg_hwnd, &mut win_rect).is_ok() {
                        let hmonitor = MonitorFromWindow(fg_hwnd, MONITOR_DEFAULTTOPRIMARY);
                        let mut mi = MONITORINFO {
                            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                            ..Default::default()
                        };

                        if GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
                            let mon_rect = mi.rcMonitor;
                            
                            // Check if foreground window strictly covers the monitor bounds
                            let matches_screen = win_rect.left <= mon_rect.left
                                && win_rect.top <= mon_rect.top
                                && win_rect.right >= mon_rect.right
                                && win_rect.bottom >= mon_rect.bottom;

                            let mut class_buf = [0u16; 64];
                            let len = windows::Win32::UI::WindowsAndMessaging::GetClassNameW(fg_hwnd, &mut class_buf);
                            let class_name = String::from_utf16_lossy(&class_buf[..len as usize]);
                            let is_desktop_or_shell = class_name == "Progman"
                                || class_name == "WorkerW"
                                || class_name == "Shell_TrayWnd"
                                || class_name == "Shell_SecondaryTrayWnd";

                            let style = GetWindowLongPtrW(fg_hwnd, GWL_STYLE);
                            let has_caption = (style & (WS_CAPTION.0 as isize)) == (WS_CAPTION.0 as isize);

                            let is_zoomed = windows::Win32::UI::WindowsAndMessaging::IsZoomed(fg_hwnd).as_bool();
                            
                            // True exclusive fullscreen (games, borderless video) covers monitor,
                            // strips caption, is NOT a standard maximized window, and is NOT the desktop.
                            let is_exclusive_fullscreen = matches_screen && !has_caption && !is_zoomed && !is_desktop_or_shell;
                            let is_maximized = is_zoomed;

                            let mut pt = windows::Win32::Foundation::POINT::default();
                            let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);
                            let cursor_at_top = pt.y <= mon_rect.top + 8;

                            if is_exclusive_fullscreen != is_fullscreen {
                                is_fullscreen = is_exclusive_fullscreen;
                                log::info!("[Fullscreen Guard] Exclusive Fullscreen active: {}", is_fullscreen);
                                
                                // Emit event without aggressively SW_HIDEing the native window
                                let _ = app_handle.emit("isle://fullscreen_changed", is_fullscreen);
                            }

                            if !is_fullscreen {
                                let _ = app_handle.emit("isle://maximized_state", serde_json::json!({
                                    "is_maximized": is_maximized,
                                    "cursor_at_top": cursor_at_top,
                                }));
                            }
                        }
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        })
        .expect("failed to spawn fullscreen guard thread");
}
