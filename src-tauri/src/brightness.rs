use std::io::{BufRead, BufReader, Write};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const CREATE_NO_WINDOW: u32 = 0x08000000;

static CURRENT_BRIGHTNESS: AtomicU32 = AtomicU32::new(100);
static BRIGHTNESS_TX: Mutex<Option<Sender<u8>>> = Mutex::new(None);

pub fn start_brightness_monitor(app_handle: AppHandle) {
    let (tx, rx): (Sender<u8>, Receiver<u8>) = channel();
    {
        let mut global_tx = BRIGHTNESS_TX.lock().unwrap();
        *global_tx = Some(tx);
    }

    std::thread::Builder::new()
        .name("isle-brightness-worker".to_string())
        .spawn(move || {
            let ps_script = r#"
$wmiMethods = (Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightnessMethods -ErrorAction SilentlyContinue)
while ($line = [Console]::In.ReadLine()) {
    if ($line.StartsWith("set ")) {
        $val = [int]($line.Substring(4))
        if ($wmiMethods) {
            $wmiMethods | Invoke-CimMethod -MethodName WmiSetBrightness -Arguments @{ Timeout = 1; Brightness = $val } -ErrorAction SilentlyContinue | Out-Null
        }
        [Console]::Out.WriteLine("OK")
    } elseif ($line -eq "get") {
        $cur = (Get-CimInstance -Namespace root/wmi -ClassName WmiMonitorBrightness -ErrorAction SilentlyContinue).CurrentBrightness
        if ($null -ne $cur) { [Console]::Out.WriteLine($cur) } else { [Console]::Out.WriteLine("ERR") }
    }
}
"#;

            let mut child = match Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(err) => {
                    log::error!("[Brightness] Failed to spawn powershell worker: {:?}", err);
                    return;
                }
            };

            let mut stdin = match child.stdin.take() {
                Some(s) => s,
                None => return,
            };
            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => return,
            };
            let mut reader = BufReader::new(stdout);

            let mut last_poll = Instant::now() - Duration::from_secs(10);
            let mut last_reported_val: i32 = -1;
            let mut line_buf = String::new();

            loop {
                // Check if any set command was received (coalescing to latest)
                let mut latest_set: Option<u8> = None;
                loop {
                    match rx.try_recv() {
                        Ok(val) => latest_set = Some(val),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => return,
                    }
                }

                if let Some(target_val) = latest_set {
                    let cmd = format!("set {}\n", target_val);
                    if stdin.write_all(cmd.as_bytes()).is_ok() && stdin.flush().is_ok() {
                        line_buf.clear();
                        let _ = reader.read_line(&mut line_buf);
                        CURRENT_BRIGHTNESS.store(target_val as u32, Ordering::Relaxed);
                        last_reported_val = target_val as i32;
                    }
                    // Briefly sleep to prevent flooding
                    std::thread::sleep(Duration::from_millis(30));
                    continue;
                }

                // Periodic Poll every 350ms to detect hardware/system brightness keys
                if last_poll.elapsed() >= Duration::from_millis(350) {
                    last_poll = Instant::now();
                    if stdin.write_all(b"get\n").is_ok() && stdin.flush().is_ok() {
                        line_buf.clear();
                        if reader.read_line(&mut line_buf).is_ok() {
                            let trimmed = line_buf.trim();
                            if let Ok(parsed) = trimmed.parse::<i32>() {
                                if parsed != last_reported_val {
                                    CURRENT_BRIGHTNESS.store(parsed.clamp(0, 100) as u32, Ordering::Relaxed);
                                    if last_reported_val >= 0 {
                                        let level_f32 = (parsed as f32) / 100.0;
                                        log::info!("[Brightness] Hardware changed to: {}%", parsed);
                                        let _ = app_handle.emit("isle://brightness_changed", serde_json::json!({
                                            "level": level_f32
                                        }));
                                    }
                                    last_reported_val = parsed;
                                }
                            }
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(50));
            }
        })
        .expect("failed to spawn brightness worker thread");
}

pub fn get_current_brightness() -> Option<f32> {
    let val = CURRENT_BRIGHTNESS.load(Ordering::Relaxed);
    Some((val as f32) / 100.0)
}

pub fn set_current_brightness(level: f32) -> bool {
    let clamped = (level * 100.0).round().clamp(0.0, 100.0) as u8;
    CURRENT_BRIGHTNESS.store(clamped as u32, Ordering::Relaxed);

    if let Ok(guard) = BRIGHTNESS_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(clamped);
            return true;
        }
    }
    false
}
