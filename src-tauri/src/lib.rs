mod agent_server;
mod audio;
mod battery_monitor;
mod brightness;
mod clipboard;
mod dpi;
mod drop_target;
mod escape_listener;
mod file_shelf;
mod fullscreen;
mod island_controller;
mod media;
mod privacy_monitor;
mod voice_meter;
mod window_manager;

use island_controller::{IslandController, IslandMode};
use tauri::{Manager, State};
use windows::Win32::Foundation::HWND;

#[tauri::command]
fn ping_host() -> String {
    "pong from Isle host".to_string()
}

#[tauri::command]
fn set_interactive(interactive: bool) {
    window_manager::set_interactive_from_ipc(interactive);
}

#[tauri::command]
fn request_island_mode(
    controller: State<'_, IslandController>,
    mode: IslandMode,
    timeout_ms: Option<u64>,
    force: Option<bool>,
) -> bool {
    let timeout = timeout_ms.map(std::time::Duration::from_millis);
    controller.force_request_mode(mode, timeout, force.unwrap_or(false))
}

#[tauri::command]
fn notify_media_active(
    controller: State<'_, IslandController>,
    has_session: bool,
    is_playing: bool,
) {
    controller.set_media_session(has_session, is_playing);
}

#[tauri::command]
fn media_control(cmd: String, seek_ms: Option<i64>) -> bool {
    media::send_playback_command(&cmd, seek_ms)
}

#[tauri::command]
fn set_system_volume(level: f32) -> bool {
    audio::set_master_volume(level)
}

#[tauri::command]
fn get_system_volume() -> Option<(f32, bool)> {
    audio::get_current_master_volume()
}

#[tauri::command]
fn set_system_brightness(level: f32) -> bool {
    brightness::set_current_brightness(level)
}

#[tauri::command]
fn get_system_brightness() -> Option<f32> {
    brightness::get_current_brightness()
}

#[tauri::command]
fn get_battery_info() -> Option<battery_monitor::BatteryInfo> {
    battery_monitor::get_battery_status()
}

#[tauri::command]
fn get_privacy_info() -> privacy_monitor::PrivacyStatus {
    privacy_monitor::get_privacy_status()
}

#[tauri::command]
fn get_media_info() -> Option<media::MediaTrackInfo> {
    media::get_current_media_track()
}

#[tauri::command]
fn reveal_file_in_explorer(path: String) -> bool {
    file_shelf::reveal_in_explorer(&path)
}

#[tauri::command]
fn stage_dropped_paths(paths: Vec<String>) -> Vec<file_shelf::StagedFile> {
    file_shelf::stage_dropped_files(paths)
}

#[tauri::command]
fn zip_staged_files(paths: Vec<String>) -> Result<String, String> {
    file_shelf::create_zip_archive(paths)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let controller = IslandController::new(handle.clone());
            app.manage(controller.clone());

            let mut host_hwnd = HWND::default();
            if let Some(main_window) = app.get_webview_window("main") {
                #[cfg(windows)]
                {
                    match main_window.hwnd() {
                        Ok(hwnd_val) => {
                            let hwnd = HWND(hwnd_val.0 as _);
                            log::info!("Obtained main window HWND: {:?}", hwnd);
                            controller.set_hwnd(hwnd);
                            host_hwnd = hwnd;
                            let _ = main_window.show();
                            let _ = main_window.set_always_on_top(true);
                            let _ = main_window.set_resizable(false);
                            if let Err(e) = window_manager::configure_host_window(hwnd, 148.0, 32.0, 0.0, handle.clone()) {
                                log::error!("Failed to configure host window: {}", e);
                            }
                            // Register IDropTarget directly on the host pill HWND
                            if let Err(e) = drop_target::register_host_drop_target(hwnd, handle.clone(), controller.clone()) {
                                log::error!("Failed to register IDropTarget on host HWND: {}", e);
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to obtain HWND from main window: {}", e);
                        }
                    }
                }
            }

            #[cfg(windows)]
            {
                // Real GSMTC media listener on dedicated MTA apartment
                media::start_media_service(handle.clone());
                // Real CoreAudio volume listener
                audio::start_audio_volume_monitor(handle.clone());
                // Real WMI Hardware brightness listener
                brightness::start_brightness_monitor(handle.clone());
                // Fullscreen Guard with real HWND hide/unhide
                fullscreen::start_fullscreen_guard(handle.clone(), host_hwnd);
                // Agent & CLI webhook notification server (ping-island style)
                agent_server::start_agent_notification_server(handle.clone(), controller.clone());
                // Global Escape Key listener for instantaneous collapse
                escape_listener::start_escape_key_listener(controller.clone());
                // Battery & Power monitor
                battery_monitor::start_battery_monitor(handle.clone());
                // Camera & Microphone privacy monitor
                privacy_monitor::start_privacy_monitor(handle.clone());
                // Hardware voice detector for audio-reactive equalizer
                voice_meter::start_voice_detection(handle.clone());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping_host,
            set_interactive,
            request_island_mode,
            notify_media_active,
            media_control,
            set_system_volume,
            get_system_volume,
            set_system_brightness,
            get_system_brightness,
            get_battery_info,
            get_privacy_info,
            get_media_info,
            reveal_file_in_explorer,
            stage_dropped_paths,
            zip_staged_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Isle application");
}
