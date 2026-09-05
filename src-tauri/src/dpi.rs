use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    GetDC, GetDeviceCaps, GetMonitorInfoW, MonitorFromWindow, ReleaseDC, LOGPIXELSX,
    MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;

pub fn get_dpi_for_window(hwnd: HWND) -> u32 {
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        if dpi > 0 {
            return dpi;
        }
        let hdc = GetDC(hwnd);
        if !hdc.is_invalid() {
            let log_pixels_x = GetDeviceCaps(hdc, LOGPIXELSX);
            let _ = ReleaseDC(hwnd, hdc);
            if log_pixels_x > 0 {
                return log_pixels_x as u32;
            }
        }
        96
    }
}

pub fn dip_to_physical(dip: f32, dpi: u32) -> i32 {
    ((dip * dpi as f32) / 96.0).round() as i32
}

pub fn physical_to_dip(physical: i32, dpi: u32) -> f32 {
    (physical as f32 * 96.0) / dpi as f32
}

pub fn calculate_top_center_anchor(
    hwnd: HWND,
    width_dip: f32,
    height_dip: f32,
    _inset_dip: f32,
) -> (i32, i32, i32, i32) {
    unsafe {
        let dpi = get_dpi_for_window(hwnd);
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };

        let (mon_left, mon_top, mon_width) = if GetMonitorInfoW(hmonitor, &mut mi).as_bool() {
            let width = mi.rcMonitor.right - mi.rcMonitor.left;
            (mi.rcMonitor.left, mi.rcMonitor.top, width)
        } else {
            (0, 0, 1920)
        };

        let width_px = dip_to_physical(width_dip, dpi);
        let height_px = dip_to_physical(height_dip, dpi);

        let x = mon_left + (mon_width - width_px) / 2;
        let y = mon_top; // Attached center notch: glued flush to top screen bezel

        (x, y, width_px, height_px)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dip_conversion() {
        assert_eq!(dip_to_physical(180.0, 96), 180);
        assert_eq!(dip_to_physical(180.0, 120), 225); // 125%
        assert_eq!(dip_to_physical(180.0, 144), 270); // 150%
        assert_eq!(dip_to_physical(32.0, 96), 32);
        assert_eq!(dip_to_physical(32.0, 144), 48);
    }
}
