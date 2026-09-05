use windows::core::implement;
use windows::Win32::Foundation::{HWND, POINTL};
use windows::Win32::System::Com::{IDataObject, FORMATETC, TYMED_HGLOBAL};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, OleInitialize, OleUninitialize, RegisterDragDrop,
    RevokeDragDrop, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

use crate::island_controller::{IslandController, IslandMode};

#[implement(IDropTarget)]
pub struct HostDropTarget {
    app_handle: tauri::AppHandle,
    controller: IslandController,
}

impl HostDropTarget {
    pub fn new(app_handle: tauri::AppHandle, controller: IslandController) -> Self {
        Self { app_handle, controller }
    }
}

impl IDropTarget_Impl for HostDropTarget_Impl {
    fn DragEnter(
        &self,
        pdataobj: Option<&IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            if let Some(data_obj) = pdataobj {
                if has_hdrop(data_obj) {
                    *pdweffect = DROPEFFECT_COPY;
                    log::info!("[OLE IDropTarget] DragEnter: Valid CF_HDROP over Isle pill");
                    
                    use tauri::Emitter;
                    let _ = self.app_handle.emit("isle://drag_enter", ());
                    return Ok(());
                }
            }
            *pdweffect = DROPEFFECT_NONE;
            Ok(())
        }
    }

    fn DragOver(
        &self,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            *pdweffect = DROPEFFECT_COPY;
            Ok(())
        }
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        log::info!("[OLE IDropTarget] DragLeave");
        use tauri::Emitter;
        let _ = self.app_handle.emit("isle://drag_leave", ());
        Ok(())
    }

    fn Drop(
        &self,
        pdataobj: Option<&IDataObject>,
        _grfkeystate: MODIFIERKEYS_FLAGS,
        _pt: &POINTL,
        pdweffect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            if let Some(data_obj) = pdataobj {
                let files = extract_hdrop_files(data_obj);
                log::info!("[OLE IDropTarget] Drop completed with {} files: {:?}", files.len(), files);
                
                if !files.is_empty() {
                    // Safe to morph HWND now that OLE drag loop has terminated!
                    self.controller.request_mode(IslandMode::FileShelf, None);
                }

                use tauri::Emitter;
                let _ = self.app_handle.emit("isle://drop", serde_json::json!({
                    "files": files
                }));
                *pdweffect = DROPEFFECT_COPY;
            } else {
                *pdweffect = DROPEFFECT_NONE;
            }
            Ok(())
        }
    }
}

pub fn register_host_drop_target(
    hwnd: HWND,
    app_handle: tauri::AppHandle,
    controller: IslandController,
) -> Result<(), String> {
    unsafe {
        let _ = OleInitialize(None);
        let drop_target: IDropTarget = HostDropTarget::new(app_handle, controller).into();
        match RegisterDragDrop(hwnd, &drop_target) {
            Ok(_) => {
                log::info!("[OLE Host] Successfully registered IDropTarget on Isle HWND {:?}", hwnd);
                Ok(())
            }
            Err(e) => {
                log::error!("[OLE Host] CRITICAL: Failed to RegisterDragDrop on Isle HWND {:?}: {:?}", hwnd, e);
                Err(format!("RegisterDragDrop failed: {:?}", e))
            }
        }
    }
}

pub fn unregister_host_drop_target(hwnd: HWND) {
    unsafe {
        let _ = RevokeDragDrop(hwnd);
        OleUninitialize();
    }
}

unsafe fn extract_hdrop_files(data_obj: &IDataObject) -> Vec<String> {
    let format = FORMATETC {
        cfFormat: 15, // CF_HDROP = 15
        ptd: std::ptr::null_mut(),
        dwAspect: 1,  // DVASPECT_CONTENT
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };

    let medium = match data_obj.GetData(&format) {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };

    let hdrop = HDROP(medium.u.hGlobal.0 as _);
    let count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
    let mut paths = Vec::new();

    let blocked_extensions = [
        "exe", "lnk", "bat", "cmd", "ps1", "msi", "scr", "url", "com", "vbs", "wsf"
    ];

    for i in 0..count {
        let len = DragQueryFileW(hdrop, i, None);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            DragQueryFileW(hdrop, i, Some(&mut buf));
            let path_str = String::from_utf16_lossy(&buf[..len as usize]);
            
            if let Ok(canon) = std::fs::canonicalize(&path_str) {
                let canon_str = canon.to_string_lossy().to_string();
                let clean_str = canon_str.strip_prefix(r"\\?\").unwrap_or(&canon_str);
                
                let is_blocked = std::path::Path::new(clean_str)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| blocked_extensions.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false);

                if !is_blocked {
                    paths.push(clean_str.to_string());
                } else {
                    log::warn!("Blocked dangerous file drop: {}", clean_str);
                }
            } else {
                paths.push(path_str);
            }
        }
    }

    paths
}

unsafe fn has_hdrop(data_obj: &IDataObject) -> bool {
    let format = FORMATETC {
        cfFormat: 15,
        ptd: std::ptr::null_mut(),
        dwAspect: 1,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    data_obj.QueryGetData(&format).is_ok()
}
