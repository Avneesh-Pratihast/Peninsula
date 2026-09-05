use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::island_controller::{IslandController, IslandMode};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentNotification {
    pub source: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub status: Option<String>, // "success", "error", "warning", "info"
    pub duration_ms: Option<u64>,
}

pub fn start_agent_notification_server(app_handle: AppHandle, controller: IslandController) {
    std::thread::spawn(move || {
        let addr = "127.0.0.1:41234";
        let listener = match TcpListener::bind(addr) {
            Ok(l) => {
                log::info!("[Agent Server] Notification listener active on http://{}", addr);
                l
            }
            Err(e) => {
                log::error!("[Agent Server] Failed to bind http://{}: {}", addr, e);
                return;
            }
        };

        for stream in listener.incoming() {
            if let Ok(mut stream) = stream {
                let handle_clone = app_handle.clone();
                let controller_clone = controller.clone();
                std::thread::spawn(move || {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(3)));
                    handle_client_connection(&mut stream, handle_clone, controller_clone);
                });
            }
        }
    });
}

fn handle_client_connection(stream: &mut TcpStream, app_handle: AppHandle, controller: IslandController) {
    let mut header_buf = Vec::new();
    let mut temp_buf = [0u8; 1024];
    let mut content_length: usize = 0;
    let mut body_start_idx: Option<usize> = None;

    // 1. Read headers until \r\n\r\n
    loop {
        match stream.read(&mut temp_buf) {
            Ok(0) => break,
            Ok(n) => {
                header_buf.extend_from_slice(&temp_buf[..n]);
                if let Some(pos) = find_subsequence(&header_buf, b"\r\n\r\n") {
                    body_start_idx = Some(pos + 4);
                    break;
                }
                if header_buf.len() > 16384 {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    let body_start = match body_start_idx {
        Some(pos) => pos,
        None => {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n");
            let _ = stream.flush();
            return;
        }
    };

    let header_str = String::from_utf8_lossy(&header_buf[..body_start]);

    // Handle CORS OPTIONS
    if header_str.starts_with("OPTIONS") {
        let resp = b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(resp);
        let _ = stream.flush();
        return;
    }

    // Extract Content-Length
    for line in header_str.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            if let Some(val_str) = line.split(':').nth(1) {
                if let Ok(len) = val_str.trim().parse::<usize>() {
                    content_length = len;
                }
            }
        }
    }

    // Handle Expect: 100-continue (PowerShell / curl default)
    if header_str.to_lowercase().contains("expect: 100-continue") {
        let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
        let _ = stream.flush();
    }

    // 2. Read remainder of body
    let mut body_bytes = header_buf[body_start..].to_vec();
    while body_bytes.len() < content_length {
        match stream.read(&mut temp_buf) {
            Ok(0) => break,
            Ok(n) => {
                body_bytes.extend_from_slice(&temp_buf[..n]);
            }
            Err(_) => break,
        }
    }

    let body_str = String::from_utf8_lossy(&body_bytes);
    if let Ok(notification) = serde_json::from_str::<AgentNotification>(body_str.trim()) {
        log::info!("[Agent Server] Received notification: {:?}", notification);

        // Emit to UI
        let _ = app_handle.emit("isle://agent_notify", &notification);

        // Switch to AgentAlert mode (dismissal managed by frontend with hover-pause support)
        controller.request_mode(IslandMode::AgentAlert, None);

        let response_body = r#"{"status":"ok"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    } else {
        log::warn!("[Agent Server] Invalid JSON body: {}", body_str);
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nConnection: close\r\n\r\n");
        let _ = stream.flush();
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| window == needle)
}
