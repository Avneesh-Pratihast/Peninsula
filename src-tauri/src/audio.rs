use tauri::{AppHandle, Emitter};
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

pub fn start_audio_volume_monitor(app_handle: AppHandle) {
    std::thread::Builder::new()
        .name("isle-audio-monitor".to_string())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            
            let mut last_vol: f32 = -1.0;
            let mut last_mute = false;

            loop {
                if let Some((vol, mute)) = get_current_master_volume() {
                    if (vol - last_vol).abs() > 0.01 || mute != last_mute {
                        if last_vol >= 0.0 {
                            log::info!("[CoreAudio] Volume changed: {:.0}%, Muted: {}", vol * 100.0, mute);
                            let _ = app_handle.emit("isle://volume_changed", serde_json::json!({
                                "level": vol,
                                "muted": mute
                            }));
                        }
                        last_vol = vol;
                        last_mute = mute;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        })
        .expect("failed to spawn audio volume thread");
}

pub fn get_current_master_volume() -> Option<(f32, bool)> {
    unsafe {
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let default_device = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia).ok()?;
        let endpoint_vol: IAudioEndpointVolume = default_device.Activate(CLSCTX_ALL, None).ok()?;
        
        if let (Ok(level), Ok(mute)) = (endpoint_vol.GetMasterVolumeLevelScalar(), endpoint_vol.GetMute()) {
            return Some((level, mute.as_bool()));
        }
        None
    }
}

pub fn set_master_volume(level: f32) -> bool {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        if let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            if let Ok(default_device) = enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
                if let Ok(endpoint_vol) = default_device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
                    let clamped = level.clamp(0.0, 1.0);
                    return endpoint_vol.SetMasterVolumeLevelScalar(clamped, std::ptr::null()).is_ok();
                }
            }
        }
        false
    }
}
