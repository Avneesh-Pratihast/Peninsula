use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    KEY_READ, REG_VALUE_TYPE,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PrivacyStatus {
    pub camera_active: bool,
    pub microphone_active: bool,
}

pub fn start_privacy_monitor(app_handle: AppHandle) {
    std::thread::spawn(move || {
        log::info!("[Privacy Monitor] Windows Camera & Microphone sensor polling started");
        let mut last_status = PrivacyStatus {
            camera_active: false,
            microphone_active: false,
        };
        let mut initial = true;

        loop {
            let status = get_privacy_status();

            if initial || status != last_status {
                log::info!("[Privacy Monitor] Status update: {:?}", status);
                let _ = app_handle.emit("isle://privacy_update", &status);
                last_status = status;
                initial = false;
            }

            std::thread::sleep(Duration::from_millis(250));
        }
    });
}

pub fn get_privacy_status() -> PrivacyStatus {
    let camera_active = is_capability_active("webcam");
    let mic_active = is_external_mic_session_active();
    PrivacyStatus {
        camera_active,
        microphone_active: mic_active,
    }
}

// True real-time CoreAudio check: returns true ONLY when an external app is actively streaming microphone audio!
// Drops to false immediately (0ms) when leaving a call or hanging up.
pub fn is_external_mic_session_active() -> bool {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: Result<IMMDeviceEnumerator, _> = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL);
        if let Ok(enumerator) = enumerator {
            for role in [eConsole, eCommunications] {
                if let Ok(device) = enumerator.GetDefaultAudioEndpoint(eCapture, role) {
                    if let Ok(session_mgr) = device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) {
                        if let Ok(session_enum) = session_mgr.GetSessionEnumerator() {
                            let count = session_enum.GetCount().unwrap_or(0);
                            let my_pid = std::process::id();
                            for i in 0..count {
                                if let Ok(control) = session_enum.GetSession(i) {
                                    if let Ok(state) = control.GetState() {
                                        if state == AudioSessionStateActive {
                                            if let Ok(control2) = control.cast::<IAudioSessionControl2>() {
                                                if let Ok(pid) = control2.GetProcessId() {
                                                    if pid != 0 && pid != my_pid {
                                                        return true;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

fn is_capability_active(capability: &str) -> bool {
    let subkeys = [
        format!(r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{}\NonPackaged", capability),
        format!(r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{}", capability),
    ];

    for subkey in &subkeys {
        if check_registry_capability_branch(subkey) {
            return true;
        }
    }
    false
}

fn check_registry_capability_branch(branch_path: &str) -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let wide_path: Vec<u16> = branch_path.encode_utf16().chain(std::iter::once(0)).collect();

        let res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );

        if res != WIN32_ERROR(0) {
            return false;
        }

        let mut index = 0u32;
        let mut name_buf = [0u16; 256];
        let mut in_use = false;

        loop {
            let mut name_len = name_buf.len() as u32;
            let enum_res = RegEnumKeyExW(
                hkey,
                index,
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                windows::core::PWSTR::null(),
                None,
                None,
            );

            if enum_res != WIN32_ERROR(0) {
                break;
            }

            let subkey_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let full_child_path = format!(r"{}\{}", branch_path, subkey_name);

            if is_app_currently_accessing(&full_child_path) {
                in_use = true;
                break;
            }

            index += 1;
        }

        let _ = RegCloseKey(hkey);
        in_use
    }
}

fn is_app_currently_accessing(app_key_path: &str) -> bool {
    unsafe {
        let mut hkey = HKEY::default();
        let wide_path: Vec<u16> = app_key_path.encode_utf16().chain(std::iter::once(0)).collect();

        let res = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wide_path.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );

        if res != WIN32_ERROR(0) {
            return false;
        }

        let mut stop_val: u64 = 0;
        let mut val_type = REG_VALUE_TYPE(0);
        let mut data_len = std::mem::size_of::<u64>() as u32;

        let stop_name: Vec<u16> = "LastUsedTimeStop".encode_utf16().chain(std::iter::once(0)).collect();
        let query_res = RegQueryValueExW(
            hkey,
            PCWSTR(stop_name.as_ptr()),
            None,
            Some(&mut val_type),
            Some(&mut stop_val as *mut u64 as *mut u8),
            Some(&mut data_len),
        );

        let _ = RegCloseKey(hkey);

        if query_res == WIN32_ERROR(0) {
            stop_val == 0
        } else {
            false
        }
    }
}
