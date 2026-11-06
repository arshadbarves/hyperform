/** macOS vs Windows/Linux modifier labels for shortcuts. */
export function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mac|iPhone|iPad/.test(navigator.platform) || /Mac OS/.test(navigator.userAgent);
}

export function isWinPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Win/.test(navigator.platform);
}

export function modLabel(): string {
  return isMacPlatform() ? "⌘" : "Ctrl+";
}

/** What the system file manager's "show this file" is called here. */
export function revealLabel(): string {
  if (isMacPlatform()) return "Reveal in Finder";
  return isWinPlatform() ? "Show in Explorer" : "Show in file manager";
}

/** Surfaces start solid; `applyWindowMaterial` lets the native material through once it is confirmed. */
export function applyPlatformClass(): void {
  const root = document.documentElement;
  const mac = isMacPlatform();
  const win = isWinPlatform();
  root.classList.toggle("plat-mac", mac);
  root.classList.toggle("plat-win", win);
  root.classList.toggle("plat-linux", !mac && !win);
  applyWindowMaterial(false);
}

export function applyWindowMaterial(translucent: boolean): void {
  const root = document.documentElement;
  root.classList.toggle("native-glass", translucent);
  root.classList.toggle("solid-surfaces", !translucent);
}
