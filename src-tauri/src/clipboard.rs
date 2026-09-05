use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub text: String,
    pub color_swatch: Option<String>,
    pub timestamp_ms: u64,
}

static CLIPBOARD_HISTORY: Mutex<Vec<ClipboardItem>> = Mutex::new(Vec::new());

pub fn handle_clipboard_update(app_handle: &AppHandle) {
    if let Some(item) = read_current_clipboard_text() {
        let mut hist = CLIPBOARD_HISTORY.lock().unwrap();
        // Ignore duplicate consecutive clip events
        if hist.first().map(|h| &h.text) != Some(&item.text) {
            hist.insert(0, item.clone());
            if hist.len() > 10 {
                hist.truncate(10);
            }
            log::info!("[Clipboard STA] Extracted text: {} (Swatch: {:?})", item.text, item.color_swatch);
            let _ = app_handle.emit("isle://clipboard_update", &item);
        }
    }
}

fn read_current_clipboard_text() -> Option<ClipboardItem> {
    unsafe {
        if OpenClipboard(HWND::default()).is_err() {
            return None;
        }

        let text_res = if let Ok(handle) = GetClipboardData(13) { // CF_UNICODETEXT = 13
            if !handle.is_invalid() {
                let ptr = handle.0 as *const u16;
                let mut len = 0;
                while *ptr.add(len) != 0 {
                    len += 1;
                    if len > 5000 { break; } // Cap read length (5k chars)
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                Some(String::from_utf16_lossy(slice))
            } else {
                None
            }
        } else {
            None
        };

        let _ = CloseClipboard();

        if let Some(text) = text_res {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return None;
            }

            let color_swatch = detect_color_swatch(trimmed);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            return Some(ClipboardItem {
                text: trimmed.to_string(),
                color_swatch,
                timestamp_ms: now,
            });
        }

        None
    }
}

fn detect_color_swatch(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    // Hex match (#RGB, #RRGGBB, #RRGGBBAA)
    if (lower.len() == 4 || lower.len() == 7 || lower.len() == 9) && lower.starts_with('#') {
        if lower[1..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(lower);
        }
    }
    // rgb/rgba/hsl match
    if lower.starts_with("rgb(") || lower.starts_with("rgba(") || lower.starts_with("hsl(") {
        if lower.ends_with(')') {
            return Some(lower);
        }
    }
    None
}
