use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use crate::privacy_monitor;

pub fn start_voice_detection(app_handle: AppHandle) {
    std::thread::Builder::new()
        .name("isle-voice-detector".to_string())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

            loop {
                // Only activate capture when another app is actively streaming microphone audio!
                if !privacy_monitor::is_external_mic_session_active() {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }

                let enumerator: Option<IMMDeviceEnumerator> =
                    CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok();
                let device = enumerator.and_then(|e| e.GetDefaultAudioEndpoint(eCapture, eConsole).ok());

                if let Some(dev) = device {
                    if let Ok(client) = dev.Activate::<IAudioClient>(CLSCTX_ALL, None) {
                        if let Ok(pwfx) = client.GetMixFormat() {
                            let channels = (*pwfx).nChannels;
                            let init_res = client.Initialize(
                                AUDCLNT_SHAREMODE_SHARED,
                                0,
                                10_000_000,
                                0,
                                pwfx,
                                None,
                            );

                            if init_res.is_ok() {
                                if let Ok(capture) = client.GetService::<IAudioCaptureClient>() {
                                    if client.Start().is_ok() {
                                        let mut last_speaking = false;
                                        let mut last_emit_time = Instant::now();
                                        let mut silence_start: Option<Instant> = None;

                                        loop {
                                            // The instant you leave the call, drop voice detection immediately!
                                            if !privacy_monitor::is_external_mic_session_active() {
                                                if last_speaking {
                                                    let _ = app_handle.emit("isle://voice_meter", serde_json::json!({
                                                        "speaking": false,
                                                        "level": 0.0
                                                    }));
                                                }
                                                break;
                                            }

                                            let mut pdata = std::ptr::null_mut();
                                            let mut num_frames = 0u32;
                                            let mut flags = 0u32;
                                            let mut max_rms = 0.0f32;

                                            while let Ok(_) = capture.GetBuffer(&mut pdata, &mut num_frames, &mut flags, None, None) {
                                                if num_frames > 0 && !pdata.is_null() && flags == 0 {
                                                    let total_samples = (num_frames * channels as u32) as usize;
                                                    let samples = std::slice::from_raw_parts(pdata as *const f32, total_samples);
                                                    let mut sum = 0.0f32;
                                                    for &s in samples {
                                                        sum += s * s;
                                                    }
                                                    let rms = (sum / samples.len() as f32).sqrt();
                                                    if rms > max_rms {
                                                        max_rms = rms;
                                                    }
                                                }
                                                let _ = capture.ReleaseBuffer(num_frames);
                                                if num_frames == 0 {
                                                    break;
                                                }
                                            }

                                            // Sweet spot: 0.0035 ignores typing & fan hum, triggers immediately on voice
                                            let is_speaking = max_rms > 0.0035;

                                            if is_speaking {
                                                silence_start = None;
                                                if !last_speaking || last_emit_time.elapsed() >= Duration::from_millis(60) {
                                                    last_emit_time = Instant::now();
                                                    last_speaking = true;
                                                    let _ = app_handle.emit("isle://voice_meter", serde_json::json!({
                                                        "speaking": true,
                                                        "level": max_rms
                                                    }));
                                                }
                                            } else {
                                                if last_speaking {
                                                    if silence_start.is_none() {
                                                        silence_start = Some(Instant::now());
                                                    } else if silence_start.unwrap().elapsed() >= Duration::from_millis(150) {
                                                        last_speaking = false;
                                                        silence_start = None;
                                                        let _ = app_handle.emit("isle://voice_meter", serde_json::json!({
                                                            "speaking": false,
                                                            "level": 0.0
                                                        }));
                                                    }
                                                }
                                            }

                                            std::thread::sleep(Duration::from_millis(30));
                                        }
                                        let _ = client.Stop();
                                    }
                                }
                            }
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(300));
            }
        })
        .expect("failed to spawn voice detector");
}
