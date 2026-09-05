use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::Win32::Foundation::HWND;
use crate::window_manager;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IslandMode {
    Idle,
    Hover,
    Peek,
    DragHover,
    MediaCompact,
    MediaAnnounce,
    MediaHover,
    MediaExpanded { pinned: bool },
    VolumeHud,
    BrightnessHud,
    Clipboard,
    FileShelf,
    AgentAlert,
    Paused,
}

impl IslandMode {
    pub fn get_size_dip(&self) -> (f32, f32) {
        match self {
            IslandMode::Idle => (148.0, 32.0),
            IslandMode::Hover => (148.0, 32.0),
            IslandMode::Peek => (148.0, 32.0),
            IslandMode::MediaCompact => (264.0, 32.0),
            IslandMode::MediaAnnounce => (264.0, 32.0),
            IslandMode::MediaHover => (324.0, 80.0),
            IslandMode::MediaExpanded { .. } => (312.0, 156.0),
            IslandMode::VolumeHud => (264.0, 32.0),
            IslandMode::BrightnessHud => (264.0, 32.0),
            IslandMode::DragHover => (304.0, 72.0),
            IslandMode::FileShelf => (324.0, 120.0),
            IslandMode::AgentAlert => (304.0, 60.0),
            IslandMode::Clipboard => (244.0, 36.0),
            IslandMode::Paused => (0.0, 0.0),
        }
    }

    pub fn priority(&self) -> u8 {
        match self {
            IslandMode::Paused => 100,
            IslandMode::DragHover => 90,
            IslandMode::FileShelf => 80,
            IslandMode::AgentAlert => 75,
            IslandMode::VolumeHud => 70,
            IslandMode::BrightnessHud => 70,
            IslandMode::MediaExpanded { pinned: true } => 65,
            IslandMode::MediaExpanded { pinned: false } => 60,
            IslandMode::MediaHover => 55,
            IslandMode::MediaAnnounce => 50,
            IslandMode::Clipboard => 45,
            IslandMode::Hover => 40,
            IslandMode::MediaCompact => 30,
            IslandMode::Peek => 20,
            IslandMode::Idle => 10,
        }
    }
}

pub struct IslandState {
    pub mode: IslandMode,
    pub return_mode: IslandMode,
    pub current_width_dip: f32,
    pub current_height_dip: f32,
    pub has_media_session: bool,
    pub is_media_playing: bool,
    pub last_pause_time: Option<Instant>,
}

#[derive(Clone)]
pub struct IslandController {
    state: Arc<Mutex<IslandState>>,
    app_handle: AppHandle,
    hwnd_raw: Arc<Mutex<Option<isize>>>,
}

impl IslandController {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            state: Arc::new(Mutex::new(IslandState {
                mode: IslandMode::Idle,
                return_mode: IslandMode::Idle,
                current_width_dip: 185.0,
                current_height_dip: 32.0,
                has_media_session: false,
                is_media_playing: false,
                last_pause_time: None,
            })),
            app_handle,
            hwnd_raw: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_hwnd(&self, hwnd: HWND) {
        let mut h = self.hwnd_raw.lock().unwrap();
        *h = Some(hwnd.0 as isize);
    }

    pub fn set_media_session(&self, has_session: bool, is_playing: bool) {
        let mut state = self.state.lock().unwrap();
        state.has_media_session = has_session;
        state.is_media_playing = is_playing;

        if !has_session {
            state.last_pause_time = None;
            state.return_mode = IslandMode::Idle;
            if state.mode != IslandMode::Idle
                && !matches!(state.mode, IslandMode::Paused | IslandMode::DragHover | IslandMode::FileShelf)
            {
                drop(state);
                self.force_request_mode(IslandMode::Idle, None, true);
            }
        } else if is_playing {
            state.last_pause_time = None;
            state.return_mode = IslandMode::MediaCompact;
            // When media is actively playing and island is in Idle, transition to compact media
            if state.mode == IslandMode::Idle {
                drop(state);
                self.force_request_mode(IslandMode::MediaCompact, None, true);
            }
        } else {
            // Paused: start 20-second delay before auto-collapsing to Idle clock
            state.last_pause_time = Some(Instant::now());
            let controller_clone = self.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(20));
                let mut s = controller_clone.state.lock().unwrap();
                if let Some(t) = s.last_pause_time {
                    if t.elapsed() >= Duration::from_secs(20) && !s.is_media_playing {
                        // Do not force collapse if the user has pinned the expanded media deck
                        if let IslandMode::MediaExpanded { pinned: true } = s.mode {
                            return;
                        }
                        s.last_pause_time = None;
                        s.return_mode = IslandMode::Idle;
                        drop(s);
                        controller_clone.force_request_mode(IslandMode::Idle, None, true);
                    }
                }
            });
        }
    }

    pub fn get_current_mode(&self) -> IslandMode {
        self.state.lock().unwrap().mode
    }

    pub fn request_mode(&self, target_mode: IslandMode, timeout: Option<Duration>) -> bool {
        self.force_request_mode(target_mode, timeout, false)
    }

    pub fn force_request_mode(&self, target_mode: IslandMode, timeout: Option<Duration>, force: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        
        if !force && target_mode.priority() < state.mode.priority() {
            log::info!(
                "Rejected transition to {:?} (priority {}) because current is {:?} (priority {})",
                target_mode, target_mode.priority(), state.mode, state.mode.priority()
            );
            return false;
        }

        // Return mode resolution:
        // When entering a transient HUD (Volume, Brightness, Clipboard, Agent), preserve the previous non-transient mode (e.g. MediaExpanded).
        let is_target_transient = target_mode == IslandMode::VolumeHud
            || target_mode == IslandMode::BrightnessHud
            || target_mode == IslandMode::Clipboard
            || target_mode == IslandMode::AgentAlert;

        let is_current_transient = state.mode == IslandMode::VolumeHud
            || state.mode == IslandMode::BrightnessHud
            || state.mode == IslandMode::Clipboard
            || state.mode == IslandMode::AgentAlert;

        if is_target_transient && !is_current_transient {
            match state.mode {
                IslandMode::MediaExpanded { pinned } => {
                    state.return_mode = IslandMode::MediaExpanded { pinned };
                }
                IslandMode::FileShelf => {
                    state.return_mode = IslandMode::FileShelf;
                }
                _ => {
                    let is_within_pause_window = state.last_pause_time.map_or(false, |t| t.elapsed() < Duration::from_secs(20));
                    if state.is_media_playing || (state.has_media_session && is_within_pause_window) {
                        state.return_mode = IslandMode::MediaCompact;
                    } else {
                        state.return_mode = IslandMode::Idle;
                    }
                }
            }
        }

        let (target_w, target_h) = target_mode.get_size_dip();
        state.mode = target_mode;
        state.current_width_dip = target_w;
        state.current_height_dip = target_h;

        let hwnd_opt = *self.hwnd_raw.lock().unwrap();
        let handle = self.app_handle.clone();
        let target_mode_clone = target_mode;

        drop(state);

        let _ = handle.emit("isle://mode_changed", serde_json::json!({
            "mode": target_mode_clone,
            "width": target_w,
            "height": target_h
        }));

        if let Some(hwnd_val) = hwnd_opt {
            let hwnd = HWND(hwnd_val as *mut core::ffi::c_void);
            let _ = window_manager::execute_morph_bounds(hwnd, target_w, target_h, 0.0, 220, Some(handle.clone()));
        }

        if let Some(duration) = timeout {
            let controller_clone = self.clone();
            std::thread::spawn(move || {
                std::thread::sleep(duration);
                let current = controller_clone.get_current_mode();
                if current == target_mode_clone {
                    let return_to = {
                        let state = controller_clone.state.lock().unwrap();
                        state.return_mode
                    };
                    controller_clone.force_request_mode(return_to, None, true);
                }
            });
        }

        true
    }
}
