import { listen } from "@tauri-apps/api/event";
import {
  checkForUpdate,
  installUpdate,
  openReleaseUrl,
  type UpdateInfo,
  type UpdateProgress,
} from "./commands";
import { pushToast } from "./runtime/toasts";

export type UpdateSnapshot = {
  info: UpdateInfo | null;
  checking: boolean;
  installing: boolean;
  progress: UpdateProgress | null;
  popoverOpen: boolean;
};

let info: UpdateInfo | null = null;
let checking = false;
let installing = false;
let progress: UpdateProgress | null = null;
let popoverOpen = false;
let toastShown = false;
let listenerAttached = false;
let isMockActive = false;
const subs = new Set<() => void>();
let snapshot: UpdateSnapshot = { info, checking, installing, progress, popoverOpen };

function emit() {
  snapshot = { info, checking, installing, progress, popoverOpen };
  for (const l of subs) l();
}

function ensureListener() {
  if (listenerAttached || typeof window === "undefined") return;
  listenerAttached = true;
  listen<UpdateProgress>("update-progress", (e) => {
    progress = e.payload;
    if (e.payload.phase === "ready" || e.payload.phase === "installing") {
      installing = true;
    }
    emit();
  }).catch((err) => console.warn("Failed to listen to update-progress", err));
}

export const updateStore = {
  subscribe(l: () => void) {
    ensureListener();
    subs.add(l);
    return () => {
      subs.delete(l);
    };
  },
  getSnapshot(): UpdateSnapshot {
    return snapshot;
  },
  setPopoverOpen(open: boolean) {
    popoverOpen = open;
    emit();
  },
  triggerMock(enable = true) {
    if (!import.meta.env.DEV) return;
    if (!enable || (isMockActive && info?.available)) {
      info = null;
      installing = false;
      progress = null;
      popoverOpen = false;
      isMockActive = false;
      emit();
      return;
    }
    isMockActive = true;
    info = {
      available: true,
      current: "1.3.0",
      latest: "1.4.0",
      url: "https://github.com/arshadbarves/hyperform/releases",
      releaseNotes:
        "### What's New in v1.4.0\n" +
        "- Complete UI/UX & cognitive ergonomics redesign\n" +
        "- Apple-grade fluid animations across sidebar, diff, and modals\n" +
        "- Live download progress tracking with byte metrics\n" +
        "- Real-time memory headroom & cache analytics\n" +
        "- Enhanced syntax-highlighted code blocks & line counters\n" +
        "- Ultra-minimalist branding and zero color-banding finish",
    };
    popoverOpen = true;
    emit();
  },
  async check(force = false) {
    if (checking || installing || (import.meta.env.DEV && isMockActive)) return;
    ensureListener();
    checking = true;
    emit();
    try {
      info = await checkForUpdate(force);
      emit();
      if (info.available && !toastShown) {
        toastShown = true;
        pushToast(`Update v${info.latest} available`, "info");
      }
    } catch (e) {
      console.error(e);
    } finally {
      checking = false;
      emit();
    }
  },
  async install() {
    if (checking || installing || !info?.available) return;
    ensureListener();

    if (import.meta.env.DEV && isMockActive) {
      installing = true;
      const total = 58.4 * 1024 * 1024;
      for (let p = 10; p <= 100; p += 15) {
        progress = {
          phase: p < 95 ? "downloading" : "installing",
          downloadedBytes: Math.round((p / 100) * total),
          totalBytes: total,
          percentage: p,
        };
        emit();
        await new Promise((r) => setTimeout(r, 450));
      }
      installing = false;
      progress = null;
      popoverOpen = false;
      pushToast("Mock update download finished successfully!", "ok");
      emit();
      return;
    }

    installing = true;
    progress = { phase: "downloading", downloadedBytes: 0 };
    emit();
    pushToast(`Downloading v${info.latest}…`, "info");
    try {
      await installUpdate();
      // request_restart() ends the process; no return expected.
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.warn("updater install failed", e);
      installing = false;
      progress = null;

      // Already on latest (stale banner) — clear and re-check; do not open browser.
      if (/no update available/i.test(msg)) {
        info = info ? { ...info, available: false } : null;
        popoverOpen = false;
        pushToast("You're already on the latest version", "ok");
        emit();
        void updateStore.check(true);
        return;
      }

      pushToast(`Update failed: ${msg}`, "warn");
      emit();
    } finally {
      installing = false;
      progress = null;
      emit();
    }
  },
  openRelease() {
    if (info?.url) void openReleaseUrl(info.url);
  },
};

if (import.meta.env.DEV && typeof window !== "undefined") {
  (window as unknown as { __previewUpdate?: (show?: boolean) => void }).__previewUpdate = (
    show = true,
  ) => updateStore.triggerMock(show);
}
