use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Storage::Streams::{DataReader, InputStreamOptions};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

static ACTIVE_SESSION: Mutex<Option<GlobalSystemMediaTransportControlsSession>> = Mutex::new(None);
static CURRENT_TRACK: Mutex<Option<MediaTrackInfo>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MediaTrackInfo {
    pub title: String,
    pub artist: String,
    pub album_title: String,
    pub is_playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub can_seek: bool,
    pub thumbnail_data: Option<String>,
}

pub fn start_media_service(app_handle: AppHandle) {
    std::thread::Builder::new()
        .name("isle-gsmc-listener".to_string())
        .spawn(move || unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);

            if let Ok(async_op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
                if let Ok(manager) = async_op.get() {
                    let mut last_title = String::new();
                    let mut last_artist = String::new();
                    let mut last_status = false;
                    let mut last_pos = 0i64;
                    let mut is_first_run = true;

                    loop {
                        let mut is_active = false;
                        if let Ok(session) = manager.GetCurrentSession() {
                            let track_info = extract_session_info(&session);
                            is_active = track_info.is_playing || !track_info.title.is_empty();

                            if is_first_run
                                || track_info.title != last_title
                                || track_info.artist != last_artist
                                || track_info.is_playing != last_status
                                || (track_info.position_ms - last_pos).abs() >= 1000
                            {
                                is_first_run = false;
                                last_title = track_info.title.clone();
                                last_artist = track_info.artist.clone();
                                last_status = track_info.is_playing;
                                last_pos = track_info.position_ms;

                                if let Ok(mut lock) = ACTIVE_SESSION.lock() {
                                    *lock = Some(session.clone());
                                }
                                if let Ok(mut lock) = CURRENT_TRACK.lock() {
                                    *lock = Some(track_info.clone());
                                }

                                let _ = app_handle.emit("isle://media_update", &track_info);
                            }
                        } else {
                            if is_first_run {
                                is_first_run = false;
                            }
                            if !last_title.is_empty() {
                                last_title.clear();
                                last_artist.clear();
                                last_status = false;
                                if let Ok(mut lock) = CURRENT_TRACK.lock() {
                                    *lock = None;
                                }
                                let _ = app_handle.emit("isle://media_update", &MediaTrackInfo::default());
                            }
                        }

                        let sleep_duration = if is_active {
                            std::time::Duration::from_millis(500)
                        } else {
                            std::time::Duration::from_millis(1500)
                        };

                        std::thread::sleep(sleep_duration);
                    }
                }
            }

            RoUninitialize();
        })
        .expect("failed to spawn media MTA thread");
}

// Zero blocking! Reads pure cached struct in 0.001ms with zero COM calls
pub fn get_current_media_track() -> Option<MediaTrackInfo> {
    if let Ok(lock) = CURRENT_TRACK.lock() {
        return lock.clone();
    }
    None
}

fn extract_session_info(session: &GlobalSystemMediaTransportControlsSession) -> MediaTrackInfo {
    let mut title = String::new();
    let mut artist = String::new();
    let mut album_title = String::new();
    let mut is_playing = false;
    let mut position_ms = 0;
    let mut duration_ms = 0;
    let mut can_seek = false;
    let mut thumbnail_data = None;

    if let Ok(playback_info) = session.GetPlaybackInfo() {
        if let Ok(status) = playback_info.PlaybackStatus() {
            is_playing = status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
        }
        if let Ok(controls) = playback_info.Controls() {
            can_seek = controls.IsPlaybackPositionEnabled().unwrap_or(false);
        }
    }

    if let Ok(async_props) = session.TryGetMediaPropertiesAsync() {
        if let Ok(props) = async_props.get() {
            if let Ok(h_title) = props.Title() {
                title = h_title.to_string();
            }
            if let Ok(h_artist) = props.Artist() {
                artist = h_artist.to_string();
            }
            if let Ok(h_album) = props.AlbumTitle() {
                album_title = h_album.to_string();
            }

            if let Ok(thumb_ref) = props.Thumbnail() {
                if let Ok(async_stream) = thumb_ref.OpenReadAsync() {
                    if let Ok(stream) = async_stream.get() {
                        if let Ok(size) = stream.Size() {
                            if size > 0 && size < 5_000_000 {
                                let mut buffer = vec![0u8; size as usize];
                                if let Ok(reader) = DataReader::CreateDataReader(&stream) {
                                    reader.SetInputStreamOptions(InputStreamOptions::None).ok();
                                    if let Ok(load_op) = reader.LoadAsync(size as u32) {
                                        if load_op.get().is_ok() {
                                            if reader.ReadBytes(&mut buffer).is_ok() {
                                                let mut hex_or_b64 = String::with_capacity(buffer.len() * 2);
                                                const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                                                for chunk in buffer.chunks(3) {
                                                    let b0 = chunk[0] as usize;
                                                    let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
                                                    let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
                                                    
                                                    let n = (b0 << 16) | (b1 << 8) | b2;
                                                    hex_or_b64.push(B64[(n >> 18) & 63] as char);
                                                    hex_or_b64.push(B64[(n >> 12) & 63] as char);
                                                    if chunk.len() > 1 {
                                                        hex_or_b64.push(B64[(n >> 6) & 63] as char);
                                                    } else {
                                                        hex_or_b64.push('=');
                                                    }
                                                    if chunk.len() > 2 {
                                                        hex_or_b64.push(B64[n & 63] as char);
                                                    } else {
                                                        hex_or_b64.push('=');
                                                    }
                                                }
                                                thumbnail_data = Some(format!("data:image/jpeg;base64,{}", hex_or_b64));
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

    if let Ok(timeline) = session.GetTimelineProperties() {
        if let Ok(pos) = timeline.Position() {
            position_ms = pos.Duration / 10000;
        }
        if let Ok(end) = timeline.EndTime() {
            duration_ms = end.Duration / 10000;
        }
    }

    MediaTrackInfo {
        title,
        artist,
        album_title,
        is_playing,
        position_ms,
        duration_ms,
        can_seek,
        thumbnail_data,
    }
}

pub fn send_playback_command(cmd: &str, seek_ms: Option<i64>) -> bool {
    let lock = ACTIVE_SESSION.lock().unwrap();
    if let Some(ref session) = *lock {
        match cmd {
            "toggle" => {
                let _ = session.TryTogglePlayPauseAsync();
                true
            }
            "play" => {
                let _ = session.TryPlayAsync();
                true
            }
            "pause" => {
                let _ = session.TryPauseAsync();
                true
            }
            "next" => {
                let _ = session.TrySkipNextAsync();
                true
            }
            "prev" => {
                let _ = session.TrySkipPreviousAsync();
                true
            }
            "seek" => {
                if let Some(target_ms) = seek_ms {
                    let ticks = target_ms * 10000;
                    let _ = session.TryChangePlaybackPositionAsync(ticks);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    } else {
        false
    }
}
