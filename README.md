<div align="center">

# 🏞️ Peninsula

### *Not an island. A peninsula for your desktop.*

[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%7C%2011-0078D6?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/Avneesh-Pratihast/Peninsula)
[![Built with Rust](https://img.shields.io/badge/Built%20With-Rust-DEA584?style=for-the-badge&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![Tauri v2](https://img.shields.io/badge/Framework-Tauri%20v2-24C8DB?style=for-the-badge&logo=tauri&logoColor=white)](https://tauri.app/)
[![Release](https://img.shields.io/github/v/release/Avneesh-Pratihast/Peninsula?color=7C3AED&style=for-the-badge&logo=github)](https://github.com/Avneesh-Pratihast/Peninsula/releases/latest)
[![License](https://img.shields.io/badge/License-MIT-success?style=for-the-badge)](LICENSE)
[![CPU Usage](https://img.shields.io/badge/Idle%20CPU-0.0%25-brightgreen?style=for-the-badge)](https://github.com/Avneesh-Pratihast/Peninsula)

<br />

<p align="center">
  <b>Peninsula</b> brings the fluid, contextual magic of modern floating HUDs to Windows — completely native, hardware-accelerated, and engineered with zero focus theft.
  <br />
  Enjoy adaptive media playback, hardware volume & brightness HUDs, drag-and-drop file shelves, microphone/camera privacy indicators, and local AI agent alerts anchored seamlessly to your screen's top bezel.
</p>

<p align="center">
  <a href="#-the-idea">The Idea</a> •
  <a href="#-quick-download">Download .exe</a> •
  <a href="#-key-features">Key Features</a> •
  <a href="#-architecture--performance">Architecture</a> •
  <a href="#-ai-agent--cli-integration">Agent API</a> •
  <a href="#-building-from-source">Build from Source</a>
</p>

---

</div>

## 💡 The Idea

> An iPhone's Dynamic Island floats detached in display space around a camera cutout. But on a monitor, your interface grows directly out of the top screen bezel.
>
> Geographically speaking: **a piece of land attached to the mainland isn't an island — it’s a peninsula.**
>
> Witty? Yes. Technically accurate? Absolutely. Beautiful on your desktop? You bet.


<img width="667" height="53" alt="image" src="https://github.com/user-attachments/assets/7fe48ee3-15cb-491c-af24-fd298d95a09a" />

## ✨ Key Features

### 🎵 Adaptive Media Island (GSMTC)
- **Universal Playback**: Automatically syncs with **Spotify, Apple Music, Tidal, YouTube, Chrome, Edge, and Firefox** via Windows System Media Transport Controls (GSMTC).
- **visionOS-inspired Squircle Artwork**: Features dynamic album art, glass glint reflections, and orbital playback progress arcs.
- **Rich Expanded Drawer**: Click to expand into an immersive audio card with ambient color bloom, live animated equalizers, and a precision hover-scrubber.
- **Glanceable Hover Card**: Hover over the notch for quick-access play/pause and track skipping without expanding the full drawer.

### 🎙️ Privacy Guard & Audio-Reactive Voice Equalizer
- **Hardware Privacy Lights**: Instant visual indicator dots whenever your webcam or microphone is accessed by any background app or browser.
- **Live Voice Waveform**: Real-time microphone amplitude detector that visualizes voice activity directly within the pill ear.

### 📥 NotchDrop File Shelf
- **Drag-and-Drop Staging**: Drag files or folders from Desktop, Explorer, or browsers directly to your screen's top bezel.
- **Batch Actions**: One-click **Zip All** archive creation, direct file reveals in File Explorer, and quick shelf clearing.
- **Native OLE COM Registration**: Deep Windows shell integration with safe file path filtering.

### 🔊 Seamless Hardware HUDs
- **Minimalist Overlays**: Smoothly replaces clunky system popups for volume changes and display brightness adjustments.
- **Golden Ratio Geometry**: Maintains visual consistency with the standby notch dimensions.

### 📋 Smart Clipboard Preview
- **Color Swatch Detection**: Automatically identifies hex color codes (`#FF5733`, `#6C5CE7`) copied to your clipboard and renders an instant color preview toast.
- **Snappy Dismissal**: Glides in and out without interrupting your workflow.

### 🤖 Local AI Agent & CLI Alert Socket (`ping-island`)
- Built-in lightweight IPC socket listening on `127.0.0.1:44755`.
- Trigger interactive notifications and island expansions from any terminal command, Python script, build pipeline, or AI agent (Antigravity, Claude, Cursor, LangChain).

---

## ⚡ Architecture & Performance

Peninsula is built from the ground up for high-performance Windows computing:

| Requirement | Implementation Detail | Result |
|---|---|---|
| **Zero Focus Theft** | `WS_EX_NOACTIVATE` + `WS_EX_TOOLWINDOW` + `WM_MOUSEACTIVATE -> MA_NOACTIVATE` Win32 subclassing. | **Zero characters lost.** You can click and interact with Peninsula while actively typing in IDEs, games, or word processors. |
| **DPI-Perfect Anchoring** | Physical pixel calculations (`dip * dpi / 96`) anchored at monitor top-center bezel with `WM_DPICHANGED` / `WM_DISPLAYCHANGE` listening. | Pixel-perfect alignment across 100%, 125%, 150%, 200% scaling factors. |
| **Fullscreen Guard** | Distinguishes between standard maximized windows (`WS_CAPTION`) and true exclusive fullscreen / borderless games. | Never obstructs your view during games or F11 videos. |
| **Resource Footprint** | Pure event-driven Rust background threads (500ms active / 2000ms idle polling). | **0.0% CPU and 0.0% GPU** at idle in Task Manager. |

---

## 🚀 Quick Download

### Windows 10 / 11 (64-bit)

1. Head over to the **[Latest Releases](https://github.com/Avneesh-Pratihast/Peninsula/releases/latest)**.
2. Download `Peninsula.exe`.
3. Double-click to run! Peninsula will smoothly dock at the top center of your primary monitor.

> **Note**: Peninsula is fully portable — no heavy installation required. Simply place it in your preferred folder or add a shortcut to your `shell:startup` folder for launch on boot.

---

## ⌨️ Controls & Shortcuts

| Action | Gesture / Shortcut |
|---|---|
| **Expand Media Deck** | Click on the pill while media is playing |
| **Instant Collapse** | Press <kbd>Escape</kbd> anywhere or click the `✕` close button |
| **Glanceable Controls** | Hover cursor over the island |
| **Stage Files** | Drag any file or folder to the top center bezel |
| **Adjust Volume** | Use your physical keyboard media keys |

---

## 🤖 AI Agent & CLI Integration

Integrate Peninsula into your local developer scripts, build systems, or autonomous AI agents!

Send a quick JSON payload to the local Peninsula socket:

### PowerShell
```powershell
$body = @{
    title   = "Build Succeeded"
    message = "Cargo release compiled in 4.2s"
    source  = "Rust CI"
} | ConvertTo-Json

$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 44755)
$stream = $client.GetStream()
$writer = New-Object System.IO.StreamWriter($stream)
$writer.WriteLine($body)
$writer.Flush()
$client.Close()
```

### Python
```python
import socket, json

payload = {
    "title": "Autonomous Agent",
    "message": "Task verification completed successfully.",
    "source": "Antigravity"
}

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.connect(("127.0.0.1", 44755))
    s.sendall(json.dumps(payload).encode("utf-8") + b"\n")
```

---

## 🛠️ Building from Source

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (1.77.2 or newer)
- [Node.js](https://nodejs.org/) (v18+) & `npm`
- Visual Studio C++ Build Tools with Windows 10/11 SDK

### Step-by-Step
```bash
# 1. Clone the repository
git clone https://github.com/Avneesh-Pratihast/Peninsula.git
cd Peninsula

# 2. Install dependencies
npm install

# 3. Run in development mode
npx tauri dev

# 4. Build standalone production binary
npx tauri build
```

The optimized executable will be generated at:
```
src-tauri/target/release/Peninsula.exe
```

---

## 📄 License

Distributed under the **MIT License**. See `LICENSE` for details.

---

<div align="center">
  <b>Engineered with precision for Windows.</b><br />
  Star ⭐ this repo if you love modern desktop aesthetics!
</div>
