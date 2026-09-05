use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StagedFile {
    pub id: String,
    pub original_path: String,
    pub file_name: String,
    pub extension: String,
    pub size_bytes: u64,
    pub formatted_size: String,
    pub is_image: bool,
    pub thumbnail_data: Option<String>,
}

pub fn stage_dropped_files(paths: Vec<String>) -> Vec<StagedFile> {
    let mut staged = Vec::new();
    let blocked_extensions = [
        "exe", "lnk", "bat", "cmd", "ps1", "msi", "scr", "url", "com", "vbs", "wsf"
    ];
    let image_extensions = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "ico"];

    for p_str in paths {
        let path = Path::new(&p_str);
        if path.exists() {
            let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let extension = path.extension().unwrap_or_default().to_string_lossy().to_string().to_lowercase();

            if blocked_extensions.contains(&extension.as_str()) {
                log::warn!("Blocked dangerous executable staged file: {}", file_name);
                continue;
            }

            let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let formatted_size = format_size(size_bytes);
            let is_image = image_extensions.contains(&extension.as_str());
            
            // Extract lightweight base64 preview for images (< 8MB)
            let mut thumbnail_data = None;
            if is_image && size_bytes > 0 && size_bytes < 8 * 1024 * 1024 {
                if let Ok(mut f) = File::open(path) {
                    let mut img_bytes = Vec::new();
                    if f.read_to_end(&mut img_bytes).is_ok() {
                        let mime = match extension.as_str() {
                            "png" => "image/png",
                            "webp" => "image/webp",
                            "gif" => "image/gif",
                            "ico" => "image/x-icon",
                            _ => "image/jpeg",
                        };
                        let b64 = base64_encode(&img_bytes);
                        thumbnail_data = Some(format!("data:{};base64,{}", mime, b64));
                    }
                }
            }

            staged.push(StagedFile {
                id: format!("{}_{}", file_name, size_bytes),
                original_path: p_str,
                file_name,
                extension,
                size_bytes,
                formatted_size,
                is_image,
                thumbnail_data,
            });
        }
    }
    staged
}

pub fn create_zip_archive(paths: Vec<String>) -> Result<String, String> {
    if paths.is_empty() {
        return Err("No files to archive".to_string());
    }

    let mut stage_dir = get_stage_directory();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let zip_filename = format!("Isle_Archive_{}.zip", timestamp);
    stage_dir.push(&zip_filename);

    let zip_file = File::create(&stage_dir).map_err(|e| format!("Failed to create zip file: {}", e))?;
    let mut zip_writer = ZipWriter::new(zip_file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    for p_str in paths {
        let src_path = Path::new(&p_str);
        if src_path.is_file() {
            let file_name = src_path.file_name().unwrap_or_default().to_string_lossy();
            zip_writer
                .start_file(file_name, options)
                .map_err(|e| format!("Failed to add file to zip: {}", e))?;

            let mut f = File::open(src_path).map_err(|e| format!("Failed to read file {}: {}", p_str, e))?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).map_err(|e| format!("Failed to read content: {}", e))?;
            zip_writer
                .write_all(&buffer)
                .map_err(|e| format!("Failed to write zip entry: {}", e))?;
        }
    }

    zip_writer.finish().map_err(|e| format!("Failed to finalize zip: {}", e))?;
    Ok(stage_dir.to_string_lossy().to_string())
}

pub fn reveal_in_explorer(path: &str) -> bool {
    let mut cmd = std::process::Command::new("explorer");
    cmd.arg(format!("/select,{}", path));
    cmd.spawn().is_ok()
}

fn get_stage_directory() -> PathBuf {
    let mut dir = dirs_local_data_path();
    dir.push("Isle");
    dir.push("stage");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn dirs_local_data_path() -> PathBuf {
    std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        result.push(CHARSET[((n >> 18) & 63) as usize] as char);
        result.push(CHARSET[((n >> 12) & 63) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARSET[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARSET[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
