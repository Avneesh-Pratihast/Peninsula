# Isle Verification Log

## Phase 1 Kill-Gate Hardware Verification Matrix (September 2, 2026)

| # | Kill-Gate Criterion | Implementation Architecture | Hardware Test Result & Observations | Status |
|---|---|---|---|---|
| **1** | **Focus Preservation** | `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST` + `WM_MOUSEACTIVATE -> MA_NOACTIVATE` subclass proc in `window_manager.rs`. | **PASS** — Notepad typing test verified: user typed continuously while clicking the Isle pill; 0 characters lost, 0 focus theft. | ✅ **VERIFIED PASS** |
| **2** | **DPI Anchoring** | `dip_to_physical` (`dip * dpi / 96`) anchored at `rcMonitor` top-center with 8 DIP inset. Subclass traps `WM_DPICHANGED` / `WM_DISPLAYCHANGE`. | **PASS** — Pill centered accurately at top bezel with proper DPI scaling factor. | ✅ **VERIFIED PASS** |
| **3** | **OLE File Drop** | Direct `IDropTarget` COM registration on host pill HWND via `drop_target.rs` with `CF_HDROP` extraction and dangerous file filtering. | **PASS** — Dragging files from Desktop/Explorer displays the native paste/drop cursor effect over the pill. | ✅ **VERIFIED PASS** |
| **4** | **Click-Through & Exact Bounds** | Sized strictly to pill bounds (`180x32` DIP). Fixed oversized pre-expand padding and removed transparent WebView2 margins so HWND strictly matches pill shape. | **RESOLVED & PATCHED** — Tight HWND sizing on collapse; added close `✕` button and `Escape` key collapse for expanded decks. | ✅ **PATCHED & VERIFIED** |
| **5** | **Zero Idle Resource Burn** | Event-driven architecture with throttled background threads (500ms active / 2000ms idle). | **PASS** — Task Manager verified: 0.0% CPU and 0.0% GPU at idle. | ✅ **VERIFIED PASS** |

## Subsystem Stability Updates
- **Fullscreen Guard Fix**: Standard maximized desktop windows (Chrome, VS Code, Notepad) have `WS_CAPTION` and no longer falsely trigger fullscreen hide. Only exclusive borderless games or F11 videos hide Isle.
- **Media Deck Dismissal**: Added `✕` close button on expanded media deck and `Escape` key shortcut to collapse back to `MediaCompact` or `Idle` at will.
- **Strict Boundary Clamping**: Eliminated arbitrary phantom margins; HWND bounds clamp tightly to physical dimensions on collapse.

## Test Suite Execution
- **Unit Tests**: `cargo test` passing (1 passed, 0 failed, 0 ignored).
- **Binary Build**: `cargo build` passing with exit code 0 (`target/debug/isle.exe` freshly compiled at `09:09:44 PM`).
