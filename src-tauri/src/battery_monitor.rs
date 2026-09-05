use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BatteryInfo {
    pub percent: u8,
    pub is_charging: bool,
    pub has_battery: bool,
}

pub fn start_battery_monitor(app_handle: AppHandle) {
    std::thread::spawn(move || {
        log::info!("[Battery Monitor] Win32 power status polling started");
        let mut last_info = BatteryInfo {
            percent: 100,
            is_charging: false,
            has_battery: true,
        };
        let mut initial = true;

        loop {
            if let Some(info) = get_battery_status() {
                if initial || info != last_info {
                    log::info!("[Battery Monitor] Update: {:?}", info);
                    let _ = app_handle.emit("isle://battery_update", &info);
                    last_info = info;
                    initial = false;
                }
            }
            std::thread::sleep(Duration::from_secs(3));
        }
    });
}

pub fn get_battery_status() -> Option<BatteryInfo> {
    unsafe {
        let mut status = SYSTEM_POWER_STATUS::default();
        if GetSystemPowerStatus(&mut status).is_ok() {
            let has_battery = status.BatteryFlag != 128 && status.BatteryLifePercent != 255;
            let percent = if status.BatteryLifePercent <= 100 {
                status.BatteryLifePercent
            } else {
                100
            };
            let is_charging = status.ACLineStatus == 1;

            Some(BatteryInfo {
                percent,
                is_charging,
                has_battery,
            })
        } else {
            None
        }
    }
}
