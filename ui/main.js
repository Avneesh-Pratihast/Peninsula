// Isle Dynamic Island Client Logic (Vanilla JS, zero bundler dependency)

const container = document.getElementById("island-container");
const decks = {
  Idle: document.getElementById("deck-idle"),
  DragHover: document.getElementById("deck-drag-hover"),
  MediaCompact: document.getElementById("deck-media-compact"),
  MediaHover: document.getElementById("deck-media-hover"),
  MediaExpanded: document.getElementById("deck-media-expanded"),
  VolumeHud: document.getElementById("deck-volume"),
  BrightnessHud: document.getElementById("deck-brightness"),
  Clipboard: document.getElementById("deck-clipboard"),
  FileShelf: document.getElementById("deck-file-shelf"),
  AgentAlert: document.getElementById("deck-agent"),
};

let currentMode = "Idle";
let stagedFilesList = [];
let hasTrackSession = false;
let isPlayingState = false;
let currentDurationMs = 0;
let currentPositionMs = 0;
let isScrubbing = false;
let isSeekPending = false;
let currentVolumeLevel = 0.5;
let currentBrightnessLevel = 1.0;
let isVolDragging = false;
let isBrightDragging = false;
let playbackTickerInterval = null;
let privacyCameraActive = false;
let privacyMicActive = false;
let isVoiceSpeaking = false;
let voicePeakLevel = 0.0;
let volumeDismissTimer = null;
let brightnessDismissTimer = null;
let clipboardDismissTimer = null;
let agentDismissTimer = null;
let pauseCollapseTimer = null;
let musicPausedTimestamp = null;
const PAUSE_COLLAPSE_DELAY_MS = 20000; // 20-second grace delay before returning to time-only idle

let previousIslandMode = "Idle";

function isTransientHud(mode) {
  return mode === "VolumeHud" || mode === "BrightnessHud" || mode === "Clipboard" || mode === "AgentAlert" || mode === "DragHover";
}

function getTargetMediaMode() {
  if (isPlayingState) return "MediaCompact";
  if (hasTrackSession && musicPausedTimestamp && (Date.now() - musicPausedTimestamp < PAUSE_COLLAPSE_DELAY_MS)) {
    return "MediaCompact";
  }
  return "Idle";
}

function getTargetReturnMode() {
  if (previousIslandMode === "MediaExpanded") {
    if (hasTrackSession) {
      if (!isPlayingState && musicPausedTimestamp && (Date.now() - musicPausedTimestamp >= PAUSE_COLLAPSE_DELAY_MS)) {
        return { type: "Idle" };
      }
      return { type: "MediaExpanded", pinned: false };
    }
  }
  if (previousIslandMode === "FileShelf" && stagedFilesList.length > 0) {
    return { type: "FileShelf" };
  }
  return { type: getTargetMediaMode() };
}

function returnToPreviousMode() {
  const targetObj = getTargetReturnMode();
  invokeTauri("request_island_mode", { mode: targetObj, timeoutMs: null, force: true });
}

// Track Announcement & Hover State
let currentTrackTitle = "";
let currentTrackArtist = "";
let announceTimer = null;
let hoverDelayTimer = null;
let hoverLeaveTimer = null;
let isMaximizedWindow = false;

// 1-Minute Cadence Clock Engine (No seconds, drift-free, centered tabular nums)
function updateClock() {
  const now = new Date();
  const hrs = String(now.getHours()).padStart(2, "0");
  const min = String(now.getMinutes()).padStart(2, "0");
  const timeStr = `${hrs}:${min}`;

  const clockIdle = document.getElementById("idle-clock");
  if (clockIdle) clockIdle.textContent = timeStr;

  const clockCompact = document.getElementById("compact-clock");
  if (clockCompact) clockCompact.textContent = timeStr;
}
updateClock();

function startClockEngine() {
  updateClock();
  const now = new Date();
  const msToNextMinute = (60 - now.getSeconds()) * 1000 - now.getMilliseconds();
  setTimeout(() => {
    updateClock();
    setInterval(updateClock, 60000);
  }, Math.max(0, msToNextMinute));
}

function updatePrivacyDot() {
  const dots = [
    { dot: document.getElementById("privacy-dot"), container: document.getElementById("privacy-container-idle") },
    { dot: document.getElementById("compact-privacy-dot"), container: document.getElementById("privacy-container-compact") },
  ];

  dots.forEach(({ dot, container }) => {
    if (!dot) return;
    dot.className = "privacy-dot";
    if (container) {
      if (privacyCameraActive) container.classList.add("camera-on");
      else container.classList.remove("camera-on");
    }

    // ONLY turn RED when camera is on! Otherwise pristine glowing white!
    if (privacyCameraActive) {
      dot.classList.add("camera");
    }
    dot.removeAttribute("title");
  });

  updateVoiceEqualizerUI();
}

// Satisfying Voice Equalizer: Morphs the dot into a bouncy 4-bar sound wave
function updateVoiceEqualizerUI() {
  const visualizers = [
    { container: document.getElementById("privacy-container-idle"), bars: document.getElementById("voice-equalizer-ring") },
    { container: document.getElementById("privacy-container-compact"), bars: document.getElementById("compact-voice-ring") },
  ];

  const activeSpeaking = privacyMicActive && isVoiceSpeaking;

  visualizers.forEach(({ container, bars }) => {
    if (!container || !bars) return;

    if (activeSpeaking) {
      container.classList.add("speaking");

      // Satisfying bell-curve dynamic wave scaling
      const boost = voicePeakLevel * 900.0;
      const h1 = Math.min(11, Math.max(3.5, 3.5 + boost * 0.6));
      const h2 = Math.min(15, Math.max(5.0, 5.0 + boost * 1.3));
      const h3 = Math.min(16, Math.max(5.5, 5.5 + boost * 1.4));
      const h4 = Math.min(11, Math.max(3.5, 3.5 + boost * 0.7));

      const b1 = bars.querySelector(".v1");
      const b2 = bars.querySelector(".v2");
      const b3 = bars.querySelector(".v3");
      const b4 = bars.querySelector(".v4");

      if (b1) b1.style.height = `${h1}px`;
      if (b2) b2.style.height = `${h2}px`;
      if (b3) b3.style.height = `${h3}px`;
      if (b4) b4.style.height = `${h4}px`;
    } else {
      container.classList.remove("speaking");
      const allBars = bars.querySelectorAll(".v-bar");
      allBars.forEach((bar) => {
        bar.style.height = "3px";
      });
    }
  });
}

// Procedural Harmonic Equalizer Engine (Refined Option B with DSP Attack/Release & Zero-CPU Sleep)
class ProceduralEqualizerEngine {
  constructor() {
    this.compactBars = [];
    this.expandedBars = [];
    this.compactWaveEl = null;
    this.expandedMiniWaveEl = null;

    this.isPlaying = false;
    this.animFrameId = null;

    // Bar parameters: [baseFreq1, baseFreq2, maxAmp, minAmp, phase]
    // Calibrated to 13.0px peak (0.62 / 1.00 / 0.78 / 0.48 asymmetric kick curve)
    this.params = [
      { f1: 1.25, f2: 2.85, maxH: 8.0,  minH: 2.5, phi: 0.2 }, // Low Bass (~62% peak)
      { f1: 1.95, f2: 3.45, maxH: 13.0, minH: 2.5, phi: 1.1 }, // Kick/Punch (100% peak)
      { f1: 2.70, f2: 4.10, maxH: 10.0, minH: 2.5, phi: 2.3 }, // Mid (~78% peak)
      { f1: 3.55, f2: 5.20, maxH: 6.2,  minH: 2.5, phi: 0.7 }  // Treble (~48% peak)
    ];

    this.currentHeights = [2.5, 2.5, 2.5, 2.5];
    this.restingHeights = [2.5, 2.5, 2.5, 2.5];
    this.tick = this.tick.bind(this);
  }

  ensureElements() {
    if (!this.compactBars.length) {
      this.compactWaveEl = document.getElementById("compact-wave");
      this.compactBars = Array.from(document.querySelectorAll("#compact-wave .bar"));
    }
    if (!this.expandedBars.length) {
      this.expandedMiniWaveEl = document.getElementById("expanded-mini-wave");
      this.expandedBars = Array.from(document.querySelectorAll("#expanded-mini-wave .m-bar"));
    }
  }

  setPlaying(playing) {
    this.ensureElements();
    this.isPlaying = !!playing;

    if (this.compactWaveEl) {
      this.compactWaveEl.classList.toggle("playing", this.isPlaying);
      this.compactWaveEl.classList.toggle("paused", !this.isPlaying);
    }
    if (this.expandedMiniWaveEl) {
      this.expandedMiniWaveEl.classList.toggle("playing", this.isPlaying);
      this.expandedMiniWaveEl.classList.toggle("paused", !this.isPlaying);
    }

    if (!this.animFrameId) {
      this.animFrameId = requestAnimationFrame(this.tick);
    }
  }

  tick(timestamp) {
    this.ensureElements();
    const t = timestamp * 0.001;
    let allSettled = true;

    // Energetic breathing envelope (~15-second musical swell, high dynamic floor)
    const envelope = 0.82 + 0.18 * Math.sin(t * 0.42);

    for (let i = 0; i < 4; i++) {
      const p = this.params[i];
      let targetH = this.restingHeights[i];

      if (this.isPlaying) {
        // Multi-harmonic aperiodic oscillator
        const s1 = Math.sin(2 * Math.PI * p.f1 * t + p.phi);
        const s2 = Math.cos(2 * Math.PI * p.f2 * t + p.phi * 1.7);
        const raw = ((s1 * 0.60 + s2 * 0.40 + 1.0) / 2.0) * envelope;
        targetH = p.minH + (p.maxH - p.minH) * Math.max(0, Math.min(1, raw));
      }

      // DSP Envelope Follower: Snappy Attack (0.46), Smooth Acoustic Release (0.18)
      const diff = targetH - this.currentHeights[i];
      const lerp = diff > 0 ? 0.46 : 0.18;
      this.currentHeights[i] += diff * lerp;

      const hStr = `${this.currentHeights[i].toFixed(1)}px`;
      if (this.compactBars[i]) this.compactBars[i].style.height = hStr;
      if (this.expandedBars[i]) this.expandedBars[i].style.height = hStr;

      if (Math.abs(diff) > 0.08) {
        allSettled = false;
      }
    }

    // Zero-CPU Dormancy when paused and settled
    if (!this.isPlaying && allSettled) {
      for (let i = 0; i < 4; i++) {
        this.currentHeights[i] = this.restingHeights[i];
        const hStr = `${this.restingHeights[i]}px`;
        if (this.compactBars[i]) this.compactBars[i].style.height = hStr;
        if (this.expandedBars[i]) this.expandedBars[i].style.height = hStr;
      }
      this.animFrameId = null;
      return; // Stop rAF loop
    }

    this.animFrameId = requestAnimationFrame(this.tick);
  }
}

const proceduralEqualizer = new ProceduralEqualizerEngine();

function updateBatteryDisplay(data) {
  if (!data) return;
  const badges = [
    { badge: document.getElementById("battery-badge"), icon: document.getElementById("battery-icon"), text: document.getElementById("battery-text") },
    { badge: document.getElementById("compact-battery-badge"), icon: document.getElementById("compact-battery-icon"), text: document.getElementById("compact-battery-text") },
  ];

  const pct = Math.max(6, Math.min(100, data.percent));
  let barColor = "#34c759"; // Good: current green

  if (data.is_charging) {
    barColor = "#34c759"; // Charging: current green
  } else if (data.percent <= 20) {
    barColor = "#ff453a"; // Low: red
  } else if (data.percent <= 50) {
    barColor = "#ff9f0a"; // Medium: orange
  }

  const batteryBarHtml = `
    <div class="battery-bar-container">
      <div class="battery-bar-shell">
        <div class="battery-bar-fill" style="width: ${pct}%; background-color: ${barColor};"></div>
        <svg class="battery-bolt-svg" viewBox="0 0 24 24"><path d="M13 2L4 14h7v8l9-12h-7z"/></svg>
      </div>
      <div class="battery-bar-tip"></div>
    </div>
  `;

  badges.forEach(({ badge, icon, text }) => {
    if (text) text.textContent = `${data.percent}%`;
    if (icon) icon.innerHTML = batteryBarHtml;

    if (badge) {
      badge.className = "battery-badge";
      if (data.is_charging) {
        badge.classList.add("charging");
      }
    }
  });
}

function setMode(modeName) {
  if (!isTransientHud(modeName)) {
    previousIslandMode = modeName;
  }
  currentMode = modeName;

  Object.keys(decks).forEach((key) => {
    if (decks[key]) {
      decks[key].classList.remove("active");
    }
  });

  container.className = "island-pill";

  switch (modeName) {
    case "Hover":
      container.classList.add("mode-hover");
      if (decks.Idle) decks.Idle.classList.add("active");
      break;
    case "Peek":
      container.classList.add("mode-peek");
      if (decks.Idle) decks.Idle.classList.add("active");
      break;
    case "DragHover":
      container.classList.add("mode-drag-hover");
      if (decks.DragHover) decks.DragHover.classList.add("active");
      break;
    case "MediaCompact":
      container.classList.add("mode-media-compact");
      if (decks.MediaCompact) decks.MediaCompact.classList.add("active");
      break;
    case "MediaAnnounce":
      container.classList.add("mode-media-compact", "mode-media-announce");
      if (decks.MediaCompact) decks.MediaCompact.classList.add("active");
      break;
    case "MediaHover":
      container.classList.add("mode-media-hover");
      if (decks.MediaHover) decks.MediaHover.classList.add("active");
      break;
    case "MediaExpanded":
      container.classList.add("mode-media-expanded");
      if (decks.MediaExpanded) decks.MediaExpanded.classList.add("active");
      break;
    case "VolumeHud":
      container.classList.add("mode-volume");
      if (decks.VolumeHud) decks.VolumeHud.classList.add("active");
      break;
    case "BrightnessHud":
      container.classList.add("mode-brightness");
      if (decks.BrightnessHud) decks.BrightnessHud.classList.add("active");
      break;
    case "Clipboard":
      container.classList.add("mode-clipboard");
      if (decks.Clipboard) decks.Clipboard.classList.add("active");
      break;
    case "FileShelf":
      container.classList.add("mode-file-shelf");
      if (decks.FileShelf) decks.FileShelf.classList.add("active");
      break;
    case "AgentAlert":
      container.classList.add("mode-agent");
      if (decks.AgentAlert) decks.AgentAlert.classList.add("active");
      break;
    default:
      container.classList.add("mode-idle");
      if (decks.Idle) decks.Idle.classList.add("active");
      break;
  }

  updatePrivacyDot();
}

// Global Tauri IPC Helper
function invokeTauri(cmd, args = {}) {
  if (window.__TAURI__ && window.__TAURI__.core && typeof window.__TAURI__.core.invoke === "function") {
    return window.__TAURI__.core.invoke(cmd, args);
  }
  console.log(`[IPC Stub] ${cmd}`, args);
  return Promise.resolve(null);
}

function listenTauri(event, callback) {
  if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === "function") {
    return window.__TAURI__.event.listen(event, callback);
  }
  return Promise.resolve(() => {});
}

let lastThumbnailUri = null;
function extractThumbnailAccent(dataUri) {
  if (!dataUri) {
    document.documentElement.style.setProperty("--album-glow-color", "rgba(167, 139, 250, 0.25)");
    document.documentElement.style.setProperty("--album-accent-color", "rgba(167, 139, 250, 0.35)");
    document.documentElement.style.setProperty("--album-secondary-color", "rgba(99, 102, 241, 0.2)");
    document.documentElement.style.setProperty("--wave-accent-color", "#ffffff");
    lastThumbnailUri = null;
    return;
  }
  if (dataUri === lastThumbnailUri) return;
  lastThumbnailUri = dataUri;

  const img = new Image();
  img.crossOrigin = "anonymous";
  img.onload = () => {
    try {
      const cvs = document.createElement("canvas");
      cvs.width = 24;
      cvs.height = 24;
      const ctx = cvs.getContext("2d");
      ctx.drawImage(img, 0, 0, 24, 24);
      const px = ctx.getImageData(0, 0, 24, 24).data;
      
      let r1 = 0, g1 = 0, b1 = 0, count1 = 0;
      let r2 = 0, g2 = 0, b2 = 0, count2 = 0;

      for (let i = 0; i < px.length; i += 4) {
        const cr = px[i], cg = px[i + 1], cb = px[i + 2];
        const max = Math.max(cr, cg, cb), min = Math.min(cr, cg, cb);
        const sat = max - min;
        
        // Primary vibrant filter
        if (sat > 25 && max > 50 && min < 220) {
          r1 += cr; g1 += cg; b1 += cb; count1++;
        }
        // Secondary mood filter (deeper mid-tones)
        if (max > 30 && max < 190) {
          r2 += cr; g2 += cg; b2 += cb; count2++;
        }
      }

      if (count1 > 0) {
        r1 = Math.round(r1 / count1);
        g1 = Math.round(g1 / count1);
        b1 = Math.round(b1 / count1);
      } else {
        r1 = 180; g1 = 170; b1 = 220;
      }

      if (count2 > 0) {
        r2 = Math.round(r2 / count2);
        g2 = Math.round(g2 / count2);
        b2 = Math.round(b2 / count2);
      } else {
        r2 = Math.max(0, r1 - 40);
        g2 = Math.max(0, g1 - 30);
        b2 = Math.min(255, b1 + 30);
      }

      document.documentElement.style.setProperty("--album-glow-color", `rgba(${r1}, ${g1}, ${b1}, 0.35)`);
      document.documentElement.style.setProperty("--album-accent-color", `rgba(${r1}, ${g1}, ${b1}, 0.40)`);
      document.documentElement.style.setProperty("--album-secondary-color", `rgba(${r2}, ${g2}, ${b2}, 0.28)`);
      document.documentElement.style.setProperty("--wave-accent-color", `rgb(${Math.min(r1 + 35, 255)}, ${Math.min(g1 + 35, 255)}, ${Math.min(b1 + 35, 255)})`);
    } catch (e) {
      console.warn("Palette extraction fallback:", e);
    }
  };
  img.src = dataUri;
}

function updatePlaybackProgress() {
  if (currentDurationMs > 0) {
    const progress = Math.min(Math.max((currentPositionMs / currentDurationMs) * 100, 0), 100);
    const fillEl = document.getElementById("media-progress-fill");
    if (fillEl) fillEl.style.width = `${progress}%`;

    const orbitalRing = document.getElementById("orbital-progress-ring");
    if (orbitalRing) {
      orbitalRing.style.setProperty("--progress", progress.toFixed(1));
      orbitalRing.style.setProperty("--ring-fill-alpha", progress > 0.5 ? "1" : "0");
    }

    const curTimeEl = document.getElementById("media-time-current");
    if (curTimeEl) curTimeEl.textContent = formatTime(currentPositionMs);

    const durTimeEl = document.getElementById("media-time-duration");
    if (durTimeEl) durTimeEl.textContent = formatTime(currentDurationMs);
  }
}

function startPlaybackTicker() {
  if (playbackTickerInterval) clearInterval(playbackTickerInterval);
  playbackTickerInterval = setInterval(() => {
    if (isPlayingState && currentDurationMs > 0 && currentPositionMs < currentDurationMs) {
      if (isScrubbing || isSeekPending) return;
      currentPositionMs += 1000;
      updatePlaybackProgress();
    }
  }, 1000);
}

function applyMediaData(data) {
  if (!data) return;
  hasTrackSession = !!(data.title || data.artist);
  invokeTauri("notify_media_active", { hasSession: hasTrackSession, isPlaying: data.is_playing });

  if (!hasTrackSession) {
    proceduralEqualizer.setPlaying(false);
    if (currentMode !== "Paused" && currentMode !== "DragHover" && currentMode !== "FileShelf" && currentMode !== "Idle") {
      invokeTauri("request_island_mode", { mode: { type: "Idle" }, timeoutMs: null, force: true });
    }
    return;
  }

  const title = data.title || "";
  const artist = data.artist || (data.album_title ? data.album_title : "");
  const isTrackChanged = (title && title !== currentTrackTitle) || (artist && artist !== currentTrackArtist);
  const wasEmpty = !currentTrackTitle;
  currentTrackTitle = title;
  currentTrackArtist = artist;

  const wasPlaying = isPlayingState;
  isPlayingState = data.is_playing;
  currentDurationMs = data.duration_ms || 0;
  if (!isScrubbing && !isSeekPending) {
    currentPositionMs = data.position_ms || 0;
  }

  // 20-Second Pause Delay Window Management:
  // When music pauses, remain in MediaCompact for 20 seconds, then transition to time-only Idle
  if (data.is_playing) {
    musicPausedTimestamp = null;
    if (pauseCollapseTimer) {
      clearTimeout(pauseCollapseTimer);
      pauseCollapseTimer = null;
    }
  } else {
    if (!musicPausedTimestamp) {
      musicPausedTimestamp = Date.now();
    }
    if (!pauseCollapseTimer) {
      pauseCollapseTimer = setTimeout(() => {
        if (!isPlayingState && hasTrackSession) {
          musicPausedTimestamp = null;
          pauseCollapseTimer = null;
          if (currentMode === "MediaCompact" || currentMode === "MediaHover" || currentMode === "MediaAnnounce") {
            invokeTauri("request_island_mode", { mode: { type: "Idle" }, timeoutMs: null, force: true });
          }
        }
      }, PAUSE_COLLAPSE_DELAY_MS);
    }
  }

  // Track Announcement: 3.2s Horizontal Peek on Track Change
  if (isTrackChanged && isPlayingState && title && !wasEmpty) {
    const announceEl = document.getElementById("compact-announce-text");
    if (announceEl) {
      announceEl.textContent = artist ? `${title} — ${artist}` : title;
    }
    if (currentMode === "MediaCompact" || currentMode === "MediaAnnounce") {
      invokeTauri("request_island_mode", { mode: { type: "MediaAnnounce" }, timeoutMs: null, force: true });
      if (announceTimer) clearTimeout(announceTimer);
      announceTimer = setTimeout(() => {
        if (currentMode === "MediaAnnounce") {
          invokeTauri("request_island_mode", { mode: { type: "MediaCompact" }, timeoutMs: null, force: true });
        }
      }, 3000);
    }
  }

  // Procedural Harmonic Equalizer State (Drives compact and expanded mini-wave concurrently)
  proceduralEqualizer.setPlaying(isPlayingState);

  // 20×20 Squircle Album Art & Dynamic Ambient Color Glow
  const compactThumbImg = document.getElementById("compact-thumb-img");
  const compactArtIcon = document.getElementById("compact-art-icon");
  extractThumbnailAccent(data.thumbnail_data);
  if (compactThumbImg && compactArtIcon) {
    if (data.thumbnail_data) {
      compactThumbImg.src = data.thumbnail_data;
      compactThumbImg.style.display = "block";
      compactArtIcon.style.display = "none";
    } else {
      compactThumbImg.style.display = "none";
      compactArtIcon.style.display = "block";
      compactArtIcon.textContent = title && title.toLowerCase().includes("video") ? "🎬" : "🎵";
    }
  }

  // visionOS Squircle Hover Icon (Play ▶ vs Pause ⏸)
  const squircleHoverIcon = document.getElementById("squircle-hover-icon");
  if (squircleHoverIcon) {
    squircleHoverIcon.innerHTML = isPlayingState
      ? `<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>`
      : `<path d="M8 5v14l11-7z"/>`;
  }

  // Hover Card Deck Update (300 × 80 DIP)
  const hoverTitleEl = document.getElementById("hover-title");
  if (hoverTitleEl) hoverTitleEl.textContent = title || "Nothing Playing";
  const hoverArtistEl = document.getElementById("hover-artist");
  if (hoverArtistEl) hoverArtistEl.textContent = artist || "System Media";

  const hoverArtImg = document.getElementById("hover-art");
  const hoverArtPh = document.getElementById("hover-art-ph");
  if (hoverArtImg && hoverArtPh) {
    if (data.thumbnail_data) {
      hoverArtImg.src = data.thumbnail_data;
      hoverArtImg.style.display = "block";
      hoverArtPh.style.display = "none";
    } else {
      hoverArtImg.style.display = "none";
      hoverArtPh.style.display = "flex";
    }
  }

  const hoverPlaySvg = document.getElementById("hover-play-svg");
  if (hoverPlaySvg) {
    hoverPlaySvg.innerHTML = isPlayingState
      ? `<path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/>`
      : `<path d="M8 5v14l11-7z"/>`;
  }

  // Rich Player Deck Update (360 × 140 DIP)
  const mediaTitleEl = document.getElementById("media-title");
  if (mediaTitleEl) mediaTitleEl.textContent = title || "Unknown Title";

  const mediaArtistEl = document.getElementById("media-artist");
  if (mediaArtistEl) mediaArtistEl.textContent = artist || "System Media";

  const expandedPlaySvg = document.getElementById("expanded-play-svg");
  if (expandedPlaySvg) {
    expandedPlaySvg.innerHTML = isPlayingState
      ? `<path d="M7.5 5h1a1.5 1.5 0 0 1 1.5 1.5v11a1.5 1.5 0 0 1-1.5 1.5h-1a1.5 1.5 0 0 1-1.5-1.5v-11A1.5 1.5 0 0 1 7.5 5zm8 0h1a1.5 1.5 0 0 1 1.5 1.5v11a1.5 1.5 0 0 1-1.5 1.5h-1a1.5 1.5 0 0 1-1.5-1.5v-11a1.5 1.5 0 0 1 1.5-1.5z"/>`
      : `<path d="M9.5 5.5a1 1 0 0 0-1.5.86v11.28a1 1 0 0 0 1.5.86l10-5.64a1 1 0 0 0 0-1.72l-10-5.64z"/>`;
  }


  const artImg = document.getElementById("media-art");
  const artPh = document.getElementById("media-art-ph");
  if (artImg && artPh) {
    if (data.thumbnail_data) {
      artImg.src = data.thumbnail_data;
      artImg.style.display = "block";
      artPh.style.display = "none";
    } else {
      artImg.style.display = "none";
      artPh.style.display = "flex";
    }
  }

  updatePlaybackProgress();
  updatePrivacyDot();
}

async function renderStagedFiles(paths) {
  if (!paths || paths.length === 0) return;
  stagedFilesList = paths;
  const stagedInfo = await invokeTauri("stage_dropped_paths", { paths });
  
  const listEl = document.getElementById("shelf-files");
  if (listEl) {
    listEl.innerHTML = "";
    const countEl = document.getElementById("shelf-count");
    if (countEl) countEl.textContent = `${stagedInfo ? stagedInfo.length : 0} file${stagedInfo && stagedInfo.length > 1 ? "s" : ""}`;

    if (stagedInfo && stagedInfo.length > 0) {
      stagedInfo.forEach((file) => {
        const row = document.createElement("div");
        row.className = "shelf-file-item";
        
        let thumbHtml = `<span class="shelf-icon">📄</span>`;
        if (file.is_image && file.thumbnail_data) {
          thumbHtml = `<img class="shelf-thumb" src="${file.thumbnail_data}" alt="thumb" />`;
        } else if (["zip", "rar", "7z", "tar", "gz"].includes(file.extension)) {
          thumbHtml = `<span class="shelf-icon">📦</span>`;
        } else if (["mp3", "flac", "wav", "m4a"].includes(file.extension)) {
          thumbHtml = `<span class="shelf-icon">🎵</span>`;
        } else if (["mp4", "mkv", "mov", "avi"].includes(file.extension)) {
          thumbHtml = `<span class="shelf-icon">🎬</span>`;
        }

        row.innerHTML = `
          ${thumbHtml}
          <span class="shelf-name" title="${file.original_path}">${file.file_name}</span>
          <span class="shelf-size">${file.formatted_size}</span>
        `;
        listEl.appendChild(row);
      });
    }
  }
}

async function initTauriEvents() {
  startClockEngine();
  startPlaybackTicker();
  proceduralEqualizer.setPlaying(false);

  // 1. INSTANT STARTUP PROBE: Check everything immediately upon opening!
  invokeTauri("get_battery_info").then((data) => {
    if (data) updateBatteryDisplay(data);
  });

  invokeTauri("get_privacy_info").then((data) => {
    if (data) {
      privacyCameraActive = data.camera_active;
      privacyMicActive = data.microphone_active;
      updatePrivacyDot();
    }
  });

  invokeTauri("get_system_volume").then((vol) => {
    if (vol) {
      currentVolumeLevel = vol[0];
    }
  });

  invokeTauri("get_system_brightness").then((b) => {
    if (typeof b === "number") {
      currentBrightnessLevel = b;
      const fillEl = document.getElementById("bright-bar-fill");
      if (fillEl) fillEl.style.width = `${Math.round(b * 100)}%`;
      const pctEl = document.getElementById("bright-percent");
      if (pctEl) pctEl.textContent = `${Math.round(b * 100)}%`;
    }
  });

  invokeTauri("get_media_info").then((track) => {
    if (track && (track.title || track.artist)) {
      applyMediaData(track);
      if (track.is_playing) {
        invokeTauri("request_island_mode", { mode: { type: "MediaCompact" }, timeoutMs: null, force: true });
      }
    }
  });

  // 2. Dynamic morph timing driven by Rust backend
  await listenTauri("isle://morph_start", (event) => {
    const payload = event.payload;
    if (payload && payload.duration_ms) {
      container.style.transitionDuration = `${payload.duration_ms}ms`;
    }
  });

  // 3. Morph Mode Changed from Rust
  await listenTauri("isle://mode_changed", (event) => {
    const payload = event.payload;
    const modeType = typeof payload.mode === "object" ? payload.mode.type : payload.mode;
    setMode(modeType);
  });

  // 4. Battery & Power Status Updates
  await listenTauri("isle://battery_update", (event) => {
    updateBatteryDisplay(event.payload);
  });

  // 5. Camera & Microphone Privacy Updates
  await listenTauri("isle://privacy_update", (event) => {
    const data = event.payload;
    if (!data) return;
    privacyCameraActive = data.camera_active;
    privacyMicActive = data.microphone_active;
    updatePrivacyDot();
  });

  // 6. Voice Meter Detection: Red Equalizer reacts to intensity of sound
  await listenTauri("isle://voice_meter", (event) => {
    const data = event.payload;
    if (!data) return;
    isVoiceSpeaking = data.speaking;
    voicePeakLevel = data.level || 0.0;
    updatePrivacyDot();
  });

  // 7. Real GSMTC Media Update (Live HUD)
  await listenTauri("isle://media_update", (event) => {
    applyMediaData(event.payload);
  });

  // 8. CoreAudio Volume Update (Live HUD)
  await listenTauri("isle://volume_changed", (event) => {
    const { level, muted } = event.payload;
    currentVolumeLevel = level;
    if (isVolDragging) return;
    const percent = Math.round(level * 100);
    
    const fillEl = document.getElementById("vol-bar-fill");
    if (fillEl) fillEl.style.width = `${percent}%`;

    const percentEl = document.getElementById("vol-percent");
    if (percentEl) percentEl.textContent = `${percent}%`;

    const iconEl = document.getElementById("vol-icon");
    if (iconEl) iconEl.textContent = muted || percent === 0 ? "🔇" : percent > 50 ? "🔊" : "🔉";

    if (!isTransientHud(currentMode)) {
      previousIslandMode = currentMode;
    }
    invokeTauri("request_island_mode", { mode: { type: "VolumeHud" }, timeoutMs: null, force: false });

    if (volumeDismissTimer) clearTimeout(volumeDismissTimer);
    volumeDismissTimer = setTimeout(() => {
      returnToPreviousMode();
    }, 1500);
  });

  // 8b. Hardware Brightness Update (Live HUD)
  await listenTauri("isle://brightness_changed", (event) => {
    const { level } = event.payload;
    currentBrightnessLevel = level;
    if (isBrightDragging) return;
    const percent = Math.round(level * 100);

    const fillEl = document.getElementById("bright-bar-fill");
    if (fillEl) fillEl.style.width = `${percent}%`;

    const percentEl = document.getElementById("bright-percent");
    if (percentEl) percentEl.textContent = `${percent}%`;

    const iconEl = document.getElementById("bright-icon");
    if (iconEl) iconEl.textContent = percent > 60 ? "🔆" : percent > 25 ? "☀️" : "🔅";

    if (!isTransientHud(currentMode)) {
      previousIslandMode = currentMode;
    }
    invokeTauri("request_island_mode", { mode: { type: "BrightnessHud" }, timeoutMs: null, force: false });

    if (brightnessDismissTimer) clearTimeout(brightnessDismissTimer);
    brightnessDismissTimer = setTimeout(() => {
      returnToPreviousMode();
    }, 1500);
  });

  // 9. Clipboard Update
  await listenTauri("isle://clipboard_update", (event) => {
    const item = event.payload;
    const textEl = document.getElementById("clip-text");
    if (textEl) textEl.textContent = item.text;

    const swatchEl = document.getElementById("clip-swatch");
    if (swatchEl) {
      if (item.color_swatch) {
        swatchEl.style.background = item.color_swatch;
        swatchEl.style.display = "block";
      } else {
        swatchEl.style.display = "none";
      }
    }

    invokeTauri("request_island_mode", { mode: { type: "Clipboard" }, timeoutMs: null, force: false });

    if (clipboardDismissTimer) clearTimeout(clipboardDismissTimer);
    clipboardDismissTimer = setTimeout(() => {
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    }, 2500);
  });

  // 10. TRIPLE-REDUNDANT FILE DROP INGESTION (NotchDrop)
  await listenTauri("isle://drag_enter", () => {
    invokeTauri("request_island_mode", { mode: { type: "DragHover" }, timeoutMs: null, force: true });
  });

  await listenTauri("isle://drag_leave", () => {
    const target = getTargetMediaMode();
    invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
  });

  await listenTauri("isle://drop", async (event) => {
    const { files } = event.payload;
    if (files && files.length > 0) {
      await renderStagedFiles(files);
      invokeTauri("request_island_mode", { mode: { type: "FileShelf" }, timeoutMs: null, force: true });
    }
  });

  await listenTauri("tauri://drag-drop", async (event) => {
    const paths = event.payload && event.payload.paths ? event.payload.paths : [];
    if (paths.length > 0) {
      await renderStagedFiles(paths);
      invokeTauri("request_island_mode", { mode: { type: "FileShelf" }, timeoutMs: null, force: true });
    }
  });

  window.addEventListener("dragenter", (e) => {
    e.preventDefault();
    invokeTauri("request_island_mode", { mode: { type: "DragHover" }, timeoutMs: null, force: true });
  });

  window.addEventListener("dragover", (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "copy";
    }
  });

  window.addEventListener("dragleave", (e) => {
    if (e.clientX <= 0 || e.clientY <= 0 || e.clientX >= window.innerWidth || e.clientY >= window.innerHeight) {
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    }
  });

  window.addEventListener("drop", async (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.dataTransfer && e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const paths = [];
      for (let i = 0; i < e.dataTransfer.files.length; i++) {
        const f = e.dataTransfer.files[i];
        if (f.path) paths.push(f.path);
        else if (f.name) paths.push(f.name);
      }
      if (paths.length > 0) {
        await renderStagedFiles(paths);
        invokeTauri("request_island_mode", { mode: { type: "FileShelf" }, timeoutMs: null, force: true });
      }
    }
  });

  // 11. Agent & CLI Live Notifications (ping-island)
  await listenTauri("isle://agent_notify", (event) => {
    const payload = event.payload;
    if (payload) {
      const titleEl = document.getElementById("agent-title");
      const subEl = document.getElementById("agent-subtitle");
      const srcEl = document.getElementById("agent-source");
      const orbEl = document.getElementById("agent-orb");

      if (titleEl) titleEl.textContent = payload.title || "Agent Task";
      if (subEl) subEl.textContent = payload.subtitle || "Completed";
      if (srcEl) srcEl.textContent = payload.source || "CLI";
      
      if (orbEl) {
        if (payload.status === "error") orbEl.textContent = "🔴";
        else if (payload.status === "warning") orbEl.textContent = "🟡";
        else if (payload.status === "success") orbEl.textContent = "✅";
        else orbEl.textContent = "🤖";
      }

      invokeTauri("request_island_mode", { mode: { type: "AgentAlert" }, timeoutMs: null, force: false });

      if (agentDismissTimer) clearTimeout(agentDismissTimer);
      const dur = (payload && payload.duration_ms) ? payload.duration_ms : 3500;
      agentDismissTimer = setTimeout(() => {
        const target = getTargetMediaMode();
        invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
      }, dur);
    }
  });

  // Compact Deck Controls: Prev, Next, Vinyl Toggle, Wave Expand
  const btnNotchPrev = document.getElementById("btn-notch-prev");
  if (btnNotchPrev) {
    btnNotchPrev.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "prev", seekMs: null });
    };
  }

  const btnNotchNext = document.getElementById("btn-notch-next");
  if (btnNotchNext) {
    btnNotchNext.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "next", seekMs: null });
    };
  }

  const artOrbitalCluster = document.getElementById("art-orbital-cluster");
  if (artOrbitalCluster) {
    artOrbitalCluster.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "toggle", seekMs: null });
    };
  }

  const compactWaveEl = document.getElementById("compact-wave");
  if (compactWaveEl) {
    compactWaveEl.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("request_island_mode", { mode: { type: "MediaExpanded", pinned: false }, timeoutMs: null, force: true });
    };
    // Equalizer Hover Pop Up: 600ms intentional deliberate hover delay (eliminates accidental pop-ups)
    compactWaveEl.addEventListener("mouseenter", () => {
      if (hoverLeaveTimer) clearTimeout(hoverLeaveTimer);
      hoverDelayTimer = setTimeout(() => {
        if (currentMode === "MediaCompact" || currentMode === "MediaAnnounce") {
          invokeTauri("request_island_mode", { mode: { type: "MediaHover" }, timeoutMs: null, force: true });
        }
      }, 600);
    });
    compactWaveEl.addEventListener("mouseleave", () => {
      if (hoverDelayTimer) clearTimeout(hoverDelayTimer);
    });
  }

  const compactDeck = document.getElementById("deck-media-compact");
  if (compactDeck) {
    compactDeck.addEventListener("click", (e) => {
      if (e.target.closest("button") || e.target.closest("#art-orbital-cluster") || e.target.closest("#compact-wave")) return;
      e.stopPropagation();
      if (hasTrackSession) {
        invokeTauri("request_island_mode", { mode: { type: "MediaExpanded", pinned: false }, timeoutMs: null, force: true });
      }
    });
    // Announcement hover delay: pause while hovering, 1200ms grace period on leave
    compactDeck.addEventListener("mouseenter", () => {
      if (currentMode === "MediaAnnounce" && announceTimer) {
        clearTimeout(announceTimer);
      }
    });
    compactDeck.addEventListener("mouseleave", () => {
      if (currentMode === "MediaAnnounce") {
        if (announceTimer) clearTimeout(announceTimer);
        announceTimer = setTimeout(() => {
          if (currentMode === "MediaAnnounce") {
            invokeTauri("request_island_mode", { mode: { type: "MediaCompact" }, timeoutMs: null, force: true });
          }
        }, 1200);
      }
    });
  }

  // Hover Card Deck Controls & Smooth Retraction Delay
  const hoverDeck = document.getElementById("deck-media-hover");
  if (hoverDeck) {
    hoverDeck.addEventListener("mouseenter", () => {
      if (hoverLeaveTimer) clearTimeout(hoverLeaveTimer);
    });

    hoverDeck.addEventListener("mouseleave", () => {
      if (currentMode === "MediaHover") {
        hoverLeaveTimer = setTimeout(() => {
          const target = getTargetMediaMode();
          invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
        }, 450);
      }
    });

    hoverDeck.addEventListener("click", (e) => {
      if (e.target.tagName === "BUTTON" || e.target.closest("button")) return;
      invokeTauri("request_island_mode", { mode: { type: "MediaExpanded", pinned: false }, timeoutMs: null, force: true });
    });
  }

  const hoverPrevBtn = document.getElementById("btn-hover-prev");
  if (hoverPrevBtn) {
    hoverPrevBtn.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "prev", seekMs: null });
    };
  }

  const hoverToggleBtn = document.getElementById("btn-hover-toggle");
  if (hoverToggleBtn) {
    hoverToggleBtn.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "toggle", seekMs: null });
    };
  }

  const hoverNextBtn = document.getElementById("btn-hover-next");
  if (hoverNextBtn) {
    hoverNextBtn.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "next", seekMs: null });
    };
  }

  // Expanded Deck Media Controls
  const toggleBtn = document.getElementById("btn-toggle");
  if (toggleBtn) {
    toggleBtn.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "toggle", seekMs: null });
    };
  }

  const prevBtn = document.getElementById("btn-prev");
  if (prevBtn) {
    prevBtn.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "prev", seekMs: null });
    };
  }

  const nextBtn = document.getElementById("btn-next");
  if (nextBtn) {
    nextBtn.onclick = (e) => {
      e.stopPropagation();
      invokeTauri("media_control", { cmd: "next", seekMs: null });
    };
  }

  // Collapse buttons
  const collapseMediaBtn = document.getElementById("btn-collapse-media");
  if (collapseMediaBtn) {
    collapseMediaBtn.onclick = (e) => {
      e.stopPropagation();
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    };
  }

  // Escape key dismiss for expanded decks
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    }
  });

  const collapseShelfBtn = document.getElementById("btn-collapse-shelf");
  if (collapseShelfBtn) {
    collapseShelfBtn.onclick = (e) => {
      e.stopPropagation();
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    };
  }

  const collapseClipBtn = document.getElementById("btn-collapse-clip");
  if (collapseClipBtn) {
    collapseClipBtn.onclick = (e) => {
      e.stopPropagation();
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    };
  }

  const collapseAgentBtn = document.getElementById("btn-collapse-agent");
  if (collapseAgentBtn) {
    collapseAgentBtn.onclick = (e) => {
      e.stopPropagation();
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    };
  }

  // Maximized Window State (Clock Standby stays visible by default)
  await listenTauri("isle://maximized_state", (event) => {
    const { is_maximized, cursor_at_top } = event.payload || {};
    isMaximizedWindow = is_maximized;
    if (cursor_at_top) {
      container.classList.remove("mode-autohide");
    }
  });

  const islandWrapper = document.getElementById("island-wrapper");
  if (islandWrapper) {
    islandWrapper.addEventListener("mouseenter", () => {
      container.classList.remove("mode-autohide");
    });
  }

  // Interactive Scrub Bar: High-Performance, Zero-Latency visionOS Draggable Scrubber
  const progressBar = document.getElementById("media-progress-bar");
  const scrubTooltip = document.getElementById("scrub-tooltip");
  const progressFill = document.getElementById("media-progress-fill");
  const timeCurrentEl = document.getElementById("media-time-current");

  let scrubTargetMs = 0;
  let seekSuppressionTimeout = null;

  function getScrubMetrics(e) {
    const rect = progressBar.getBoundingClientRect();
    const clientX = e.clientX ?? (e.touches && e.touches[0] ? e.touches[0].clientX : 0);
    const clickX = Math.max(0, Math.min(clientX - rect.left, rect.width));
    const pct = rect.width > 0 ? clickX / rect.width : 0;
    return { clickX, pct, width: rect.width };
  }

  function applyScrubFrame(clickX, pct) {
    if (progressFill) {
      progressFill.style.width = `${(pct * 100).toFixed(2)}%`;
    }
    if (currentDurationMs > 0) {
      scrubTargetMs = Math.round(pct * currentDurationMs);
      if (timeCurrentEl) {
        timeCurrentEl.textContent = formatTime(scrubTargetMs);
      }
      if (scrubTooltip) {
        const clampedX = Math.max(16, Math.min(clickX, progressBar.offsetWidth - 16));
        scrubTooltip.style.left = `${clampedX}px`;
        scrubTooltip.textContent = formatTime(scrubTargetMs);
      }
    }
  }

  if (progressBar) {
    // Passive Hover Tooltip Follow
    progressBar.addEventListener("pointermove", (e) => {
      if (isScrubbing) return;
      const { clickX, pct } = getScrubMetrics(e);
      if (scrubTooltip && currentDurationMs > 0) {
        const clampedX = Math.max(16, Math.min(clickX, progressBar.offsetWidth - 16));
        scrubTooltip.style.left = `${clampedX}px`;
        const hoverMs = Math.round(pct * currentDurationMs);
        scrubTooltip.textContent = formatTime(hoverMs);
      }
    });

    // Pointerdown: Grab playhead and immediately position with 0ms lag
    progressBar.addEventListener("pointerdown", (e) => {
      if (currentDurationMs <= 0) return;
      e.stopPropagation();
      e.preventDefault();
      isScrubbing = true;
      progressBar.classList.add("scrubbing");

      try {
        progressBar.setPointerCapture(e.pointerId);
      } catch (_) {}

      const { clickX, pct } = getScrubMetrics(e);
      applyScrubFrame(clickX, pct);
    });

    // Pointermove: Instantaneous 60/120fps dragging without transition latency
    progressBar.addEventListener("pointermove", (e) => {
      if (!isScrubbing) return;
      e.stopPropagation();
      e.preventDefault();
      const { clickX, pct } = getScrubMetrics(e);
      applyScrubFrame(clickX, pct);
    });

    // Pointerup / Pointercancel: Release, commit seek, and smoothly resume
    const finishScrub = (e) => {
      if (!isScrubbing) return;
      e.stopPropagation();
      e.preventDefault();
      isScrubbing = false;
      progressBar.classList.remove("scrubbing");

      try {
        if (progressBar.hasPointerCapture(e.pointerId)) {
          progressBar.releasePointerCapture(e.pointerId);
        }
      } catch (_) {}

      if (currentDurationMs > 0) {
        const { pct } = getScrubMetrics(e);
        const finalMs = Math.round(pct * currentDurationMs);
        currentPositionMs = finalMs;
        updatePlaybackProgress();

        // Commit seek command to Windows GSMTC
        invokeTauri("media_control", { cmd: "seek", seekMs: finalMs });

        // Temporarily freeze incoming position updates to eliminate rubber-banding
        isSeekPending = true;
        if (seekSuppressionTimeout) clearTimeout(seekSuppressionTimeout);
        seekSuppressionTimeout = setTimeout(() => {
          isSeekPending = false;
        }, 750);
      }
    };

    progressBar.addEventListener("pointerup", finishScrub);
    progressBar.addEventListener("pointercancel", finishScrub);
  }

  // ---------------------------------------------------------
  // Unified visionOS Draggable Volume & Brightness Controllers
  // ---------------------------------------------------------

  // Helper visual functions
  function applyVolumeVisuals(pct) {
    const percent = Math.round(pct * 100);
    const fillEl = document.getElementById("vol-bar-fill");
    if (fillEl) fillEl.style.width = `${percent}%`;
    const pctEl = document.getElementById("vol-percent");
    if (pctEl) pctEl.textContent = `${percent}%`;
    const iconEl = document.getElementById("vol-icon");
    if (iconEl) iconEl.textContent = percent === 0 ? "🔇" : percent > 50 ? "🔊" : "🔉";
  }

  function applyBrightnessVisuals(pct) {
    const percent = Math.round(pct * 100);
    const fillEl = document.getElementById("bright-bar-fill");
    if (fillEl) fillEl.style.width = `${percent}%`;
    const pctEl = document.getElementById("bright-percent");
    if (pctEl) pctEl.textContent = `${percent}%`;
    const iconEl = document.getElementById("bright-icon");
    if (iconEl) iconEl.textContent = percent > 60 ? "🔆" : percent > 25 ? "☀️" : "🔅";
  }

  // 1. Interactive Zero-Lag Draggable Volume Controller
  const volBarBg = document.getElementById("vol-bar-bg");
  if (volBarBg) {
    const getVolMetrics = (e) => {
      const rect = volBarBg.getBoundingClientRect();
      const clientX = e.clientX ?? (e.touches && e.touches[0] ? e.touches[0].clientX : 0);
      const clickX = Math.max(0, Math.min(clientX - rect.left, rect.width));
      return rect.width > 0 ? clickX / rect.width : 0;
    };

    volBarBg.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (!isTransientHud(currentMode)) {
        previousIslandMode = currentMode;
      }
      isVolDragging = true;
      volBarBg.classList.add("dragging");
      try { volBarBg.setPointerCapture(e.pointerId); } catch (_) {}
      if (volumeDismissTimer) clearTimeout(volumeDismissTimer);

      const pct = getVolMetrics(e);
      currentVolumeLevel = pct;
      applyVolumeVisuals(pct);
      invokeTauri("set_system_volume", { level: pct });
    });

    volBarBg.addEventListener("pointermove", (e) => {
      if (!isVolDragging) return;
      e.stopPropagation();
      e.preventDefault();
      const pct = getVolMetrics(e);
      currentVolumeLevel = pct;
      applyVolumeVisuals(pct);
      invokeTauri("set_system_volume", { level: pct });
    });

    const finishVolDrag = (e) => {
      if (!isVolDragging) return;
      e.stopPropagation();
      e.preventDefault();
      isVolDragging = false;
      volBarBg.classList.remove("dragging");
      try {
        if (volBarBg.hasPointerCapture(e.pointerId)) {
          volBarBg.releasePointerCapture(e.pointerId);
        }
      } catch (_) {}

      const pct = getVolMetrics(e);
      currentVolumeLevel = pct;
      applyVolumeVisuals(pct);
      invokeTauri("set_system_volume", { level: pct });

      if (volumeDismissTimer) clearTimeout(volumeDismissTimer);
      volumeDismissTimer = setTimeout(() => {
        returnToPreviousMode();
      }, 1500);
    };

    volBarBg.addEventListener("pointerup", finishVolDrag);
    volBarBg.addEventListener("pointercancel", finishVolDrag);
  }

  // Volume HUD hover delay integration: pause dismissal while hovering, 1200ms grace on leave
  const volDeck = document.getElementById("deck-volume");
  if (volDeck) {
    volDeck.addEventListener("mouseenter", () => {
      if (volumeDismissTimer) clearTimeout(volumeDismissTimer);
    });
    volDeck.addEventListener("mouseleave", () => {
      if (!isVolDragging && currentMode === "VolumeHud") {
        if (volumeDismissTimer) clearTimeout(volumeDismissTimer);
        volumeDismissTimer = setTimeout(() => {
          returnToPreviousMode();
        }, 1200);
      }
    });
  }

  // Volume icon mute toggle
  const volIcon = document.getElementById("vol-icon");
  if (volIcon) {
    volIcon.onclick = (e) => {
      e.stopPropagation();
      currentVolumeLevel = currentVolumeLevel > 0 ? 0 : 0.5;
      applyVolumeVisuals(currentVolumeLevel);
      invokeTauri("set_system_volume", { level: currentVolumeLevel });
    };
  }

  // 2. Interactive Zero-Lag Draggable Brightness Controller
  const brightBarBg = document.getElementById("bright-bar-bg");
  if (brightBarBg) {
    const getBrightMetrics = (e) => {
      const rect = brightBarBg.getBoundingClientRect();
      const clientX = e.clientX ?? (e.touches && e.touches[0] ? e.touches[0].clientX : 0);
      const clickX = Math.max(0, Math.min(clientX - rect.left, rect.width));
      return rect.width > 0 ? clickX / rect.width : 0;
    };

    brightBarBg.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      e.preventDefault();
      if (!isTransientHud(currentMode)) {
        previousIslandMode = currentMode;
      }
      isBrightDragging = true;
      brightBarBg.classList.add("dragging");
      try { brightBarBg.setPointerCapture(e.pointerId); } catch (_) {}
      if (brightnessDismissTimer) clearTimeout(brightnessDismissTimer);

      const pct = getBrightMetrics(e);
      currentBrightnessLevel = pct;
      applyBrightnessVisuals(pct);
      invokeTauri("set_system_brightness", { level: pct });
    });

    brightBarBg.addEventListener("pointermove", (e) => {
      if (!isBrightDragging) return;
      e.stopPropagation();
      e.preventDefault();
      const pct = getBrightMetrics(e);
      currentBrightnessLevel = pct;
      applyBrightnessVisuals(pct);
      invokeTauri("set_system_brightness", { level: pct });
    });

    const finishBrightDrag = (e) => {
      if (!isBrightDragging) return;
      e.stopPropagation();
      e.preventDefault();
      isBrightDragging = false;
      brightBarBg.classList.remove("dragging");
      try {
        if (brightBarBg.hasPointerCapture(e.pointerId)) {
          brightBarBg.releasePointerCapture(e.pointerId);
        }
      } catch (_) {}

      const pct = getBrightMetrics(e);
      currentBrightnessLevel = pct;
      applyBrightnessVisuals(pct);
      invokeTauri("set_system_brightness", { level: pct });

      if (brightnessDismissTimer) clearTimeout(brightnessDismissTimer);
      brightnessDismissTimer = setTimeout(() => {
        returnToPreviousMode();
      }, 1500);
    };

    brightBarBg.addEventListener("pointerup", finishBrightDrag);
    brightBarBg.addEventListener("pointercancel", finishBrightDrag);
  }

  // Brightness HUD hover delay integration: pause dismissal while hovering, 1200ms grace on leave
  const brightDeck = document.getElementById("deck-brightness");
  if (brightDeck) {
    brightDeck.addEventListener("mouseenter", () => {
      if (brightnessDismissTimer) clearTimeout(brightnessDismissTimer);
    });
    brightDeck.addEventListener("mouseleave", () => {
      if (!isBrightDragging && currentMode === "BrightnessHud") {
        if (brightnessDismissTimer) clearTimeout(brightnessDismissTimer);
        brightnessDismissTimer = setTimeout(() => {
          returnToPreviousMode();
        }, 1200);
      }
    });
  }

  // Brightness icon step toggle
  const brightIcon = document.getElementById("bright-icon");
  if (brightIcon) {
    brightIcon.onclick = (e) => {
      e.stopPropagation();
      // Cycle through 25% -> 50% -> 75% -> 100%
      let nextPct = currentBrightnessLevel + 0.25;
      if (nextPct > 1.05) nextPct = 0.25;
      currentBrightnessLevel = Math.min(1, Math.max(0.1, nextPct));
      applyBrightnessVisuals(currentBrightnessLevel);
      invokeTauri("set_system_brightness", { level: currentBrightnessLevel });
    };
  }

  // 3. Mouse Wheel on the Island Pill:
  // - Shift + Wheel (or when in Brightness HUD): Adjusts Brightness
  // - Wheel (standard): Adjusts Volume
  container.addEventListener("wheel", (e) => {
    e.preventDefault();
    const step = 0.02;
    const dir = e.deltaY > 0 ? step : (e.deltaY < 0 ? -step : 0);
    if (dir === 0) return;

    if (e.shiftKey || currentMode === "BrightnessHud") {
      if (!isTransientHud(currentMode)) {
        previousIslandMode = currentMode;
      }
      currentBrightnessLevel = Math.max(0.05, Math.min(1, currentBrightnessLevel + dir));
      applyBrightnessVisuals(currentBrightnessLevel);
      invokeTauri("set_system_brightness", { level: currentBrightnessLevel });
      invokeTauri("request_island_mode", { mode: { type: "BrightnessHud" }, timeoutMs: null, force: false });

      if (brightnessDismissTimer) clearTimeout(brightnessDismissTimer);
      brightnessDismissTimer = setTimeout(() => {
        returnToPreviousMode();
      }, 1500);
    } else {
      if (!isTransientHud(currentMode)) {
        previousIslandMode = currentMode;
      }
      currentVolumeLevel = Math.max(0, Math.min(1, currentVolumeLevel + dir));
      applyVolumeVisuals(currentVolumeLevel);
      invokeTauri("set_system_volume", { level: currentVolumeLevel });
      invokeTauri("request_island_mode", { mode: { type: "VolumeHud" }, timeoutMs: null, force: false });

      if (volumeDismissTimer) clearTimeout(volumeDismissTimer);
      volumeDismissTimer = setTimeout(() => {
        returnToPreviousMode();
      }, 1500);
    }
  }, { passive: false });

  // Clipboard HUD hover delay integration
  const clipDeck = document.getElementById("deck-clipboard");
  if (clipDeck) {
    clipDeck.addEventListener("mouseenter", () => {
      if (clipboardDismissTimer) clearTimeout(clipboardDismissTimer);
    });
    clipDeck.addEventListener("mouseleave", () => {
      if (currentMode === "Clipboard") {
        if (clipboardDismissTimer) clearTimeout(clipboardDismissTimer);
        clipboardDismissTimer = setTimeout(() => {
          returnToPreviousMode();
        }, 1200);
      }
    });
  }

  // Agent Alert HUD hover delay integration: pause dismissal while hovering, 1200ms grace on leave, click dismiss
  const agentDeck = document.getElementById("deck-agent");
  if (agentDeck) {
    agentDeck.addEventListener("mouseenter", () => {
      if (agentDismissTimer) clearTimeout(agentDismissTimer);
    });
    agentDeck.addEventListener("mouseleave", () => {
      if (currentMode === "AgentAlert") {
        if (agentDismissTimer) clearTimeout(agentDismissTimer);
        agentDismissTimer = setTimeout(() => {
          returnToPreviousMode();
        }, 1200);
      }
    });
    agentDeck.addEventListener("click", (e) => {
      e.stopPropagation();
      if (agentDismissTimer) clearTimeout(agentDismissTimer);
      returnToPreviousMode();
    });
  }

  // File shelf actions
  const zipBtn = document.getElementById("btn-zip-files");
  if (zipBtn) {
    zipBtn.onclick = async (e) => {
      e.stopPropagation();
      if (stagedFilesList.length > 0) {
        try {
          zipBtn.textContent = "⏳ Zipping...";
          const zipPath = await invokeTauri("zip_staged_files", { paths: stagedFilesList });
          zipBtn.textContent = "✅ Zipped!";
          setTimeout(() => { zipBtn.textContent = "📦 Zip All"; }, 2000);
          if (zipPath) {
            await renderStagedFiles([zipPath]);
            invokeTauri("reveal_file_in_explorer", { path: zipPath });
          }
        } catch (err) {
          zipBtn.textContent = "❌ Error";
          console.error("Zip failed:", err);
          setTimeout(() => { zipBtn.textContent = "📦 Zip All"; }, 2000);
        }
      }
    };
  }

  const revealBtn = document.getElementById("btn-reveal-files");
  if (revealBtn) {
    revealBtn.onclick = (e) => {
      e.stopPropagation();
      if (stagedFilesList.length > 0) {
        invokeTauri("reveal_file_in_explorer", { path: stagedFilesList[0] });
      }
    };
  }

  const clearBtn = document.getElementById("btn-clear-shelf");
  if (clearBtn) {
    clearBtn.onclick = (e) => {
      e.stopPropagation();
      stagedFilesList = [];
      previousIslandMode = getTargetMediaMode();
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    };
  }

  // Island click behavior:
  // When in expanded media, click outside buttons collapses back to compact media / idle.
  // Note: Compact media does NOT expand on arbitrary click — ONLY clicking the equalizer wave expands it!
  container.onclick = (e) => {
    if (e.target.tagName === "BUTTON" || e.target.closest("button") || e.target.closest(".deck-scrub") || e.target.closest(".scrub-bar") || e.target.closest(".deck-volume") || e.target.closest(".vol-bar-bg") || e.target.closest(".deck-brightness") || e.target.closest(".bright-bar-bg") || e.target.classList.contains("btn-icon-close")) return;
    if (currentMode === "Idle" && hasTrackSession) {
      invokeTauri("request_island_mode", { mode: { type: "MediaCompact" }, timeoutMs: null, force: true });
    } else if (currentMode === "MediaExpanded") {
      previousIslandMode = getTargetMediaMode();
      const target = getTargetMediaMode();
      invokeTauri("request_island_mode", { mode: { type: target }, timeoutMs: null, force: true });
    }
  };
}

function formatTime(ms) {
  if (!ms || isNaN(ms) || ms < 0) return "0:00";
  const totalSec = Math.floor(ms / 1000);
  const hrs = Math.floor(totalSec / 3600);
  const min = Math.floor((totalSec % 3600) / 60);
  const sec = totalSec % 60;
  if (hrs > 0) {
    return `${hrs}:${min < 10 ? "0" : ""}${min}:${sec < 10 ? "0" : ""}${sec}`;
  }
  return `${min}:${sec < 10 ? "0" : ""}${sec}`;
}

if (document.readyState === "loading") {
  window.addEventListener("DOMContentLoaded", () => {
    initTauriEvents();
  });
} else {
  initTauriEvents();
}
