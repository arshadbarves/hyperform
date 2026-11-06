/** Presentation helpers for `[shell.sandbox]`. */

export type SandboxPlatform = "mac" | "linux" | "windows";

export function sandboxPlatform(isMac: boolean, isWindows: boolean): SandboxPlatform {
  if (isMac) return "mac";
  return isWindows ? "windows" : "linux";
}

/** Which OS sandbox runs commands here, or why none does. */
export function sandboxPlatformNote(platform: SandboxPlatform): string {
  switch (platform) {
    case "mac":
      return "Uses the built-in macOS sandbox (sandbox-exec).";
    case "linux":
      return "Needs bubblewrap (bwrap) installed; without it commands run unsandboxed with a notice.";
    case "windows":
      return "Not available on Windows yet; commands run unsandboxed with a notice.";
  }
}

/** Entries that would make most of the disk writable are refused. */
export function writableDirError(entry: string): string | null {
  const trimmed = entry.trim().replace(/[/\\]+$/, "");
  if (/[\r\n]/.test(entry)) return "Enter one directory per entry.";
  if (trimmed === "" || trimmed === "~") return `${entry} would make most of the disk writable.`;
  return null;
}
