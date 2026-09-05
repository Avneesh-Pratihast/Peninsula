use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE};
use crate::island_controller::{IslandController, IslandMode};

pub fn start_escape_key_listener(controller: IslandController) {
    std::thread::spawn(move || {
        log::info!("[Escape Listener] Global Escape hotkey listener active");
        loop {
            let current_mode = controller.get_current_mode();
            let is_expanded = matches!(
                current_mode,
                IslandMode::MediaExpanded { .. }
                    | IslandMode::FileShelf
                    | IslandMode::Clipboard
                    | IslandMode::AgentAlert
            );

            if is_expanded {
                unsafe {
                    let state = GetAsyncKeyState(VK_ESCAPE.0 as i32);
                    // High bit indicates key is currently pressed down
                    if (state as u16 & 0x8000) != 0 {
                        log::info!("[Escape Listener] VK_ESCAPE detected, collapsing active expanded deck");
                        let is_media_expanded = matches!(controller.get_current_mode(), IslandMode::MediaExpanded { .. });
                        let return_mode = if is_media_expanded {
                            IslandMode::MediaCompact
                        } else {
                            IslandMode::Idle
                        };
                        controller.request_mode(return_mode, None);
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            } else {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
    });
}
