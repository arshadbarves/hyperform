import type { DecisionModelStatus } from "../../commands/settings";
import type { DecisionRuntime } from "../../protocol/config/DecisionRuntime";
import type { ChoiceOption } from "./options";

/** The Decision model card's runtime choice and native model row. */

export const RUNTIME_OPTIONS: readonly ChoiceOption<DecisionRuntime>[] = [
  {
    value: "sidecar",
    label: "Sidecar",
    description: "laya-serve answers over this machine's network, started by the command below or already running.",
  },
  {
    value: "native",
    label: "In the app",
    description:
      "The app runs the model itself from the files downloaded below; without them, the features keep today's behavior.",
  },
];

export type NativeModelTone = "quiet" | "ok" | "warn" | "danger" | "working";
export type NativeModelAction = "download" | "cancel" | "remove" | null;

export interface NativeModelView {
  tone: NativeModelTone;
  text: string;
  /** Downloaded share, 0 to 1, while a download runs. */
  progress: number | null;
  action: NativeModelAction;
  actionLabel: string;
}

/** `1234567` as `1.2 MB`: decimal units, as download sizes are quoted. */
export function formatBytes(bytes: number): string {
  const units = ["bytes", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) return `${bytes} bytes`;
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

function share(status: DecisionModelStatus): number {
  return status.totalBytes > 0 ? Math.min(1, status.downloadedBytes / status.totalBytes) : 0;
}

/** What the native model row says and offers for `status`. */
export function nativeModelView(status: DecisionModelStatus): NativeModelView {
  const total = formatBytes(status.totalBytes);
  const done = formatBytes(status.downloadedBytes);
  if (!status.available) {
    const text = `No model to download: ${status.error ?? `unknown checkpoint "${status.checkpoint}"`}.`;
    return { tone: "danger", text, progress: null, action: null, actionLabel: "" };
  }
  if (status.downloading) {
    const percent = Math.floor(share(status) * 100);
    const text = `Downloading ${done} of ${total} (${percent}%).`;
    return { tone: "working", text, progress: share(status), action: "cancel", actionLabel: "Cancel" };
  }
  if (status.installed) {
    const revision = (status.revision ?? "").slice(0, 7);
    const built = status.nativeBuilt
      ? ""
      : " This app was built without the native runtime, so it cannot run it; use the sidecar.";
    const tone = status.nativeBuilt ? "ok" : "warn";
    const text = `Downloaded: ${status.checkpoint} at ${revision} (${total}).${built}`;
    return { tone, text, progress: null, action: "remove", actionLabel: "Remove" };
  }
  const partial = status.downloadedBytes > 0;
  const actionLabel = partial ? "Resume" : "Download";
  if (status.error) {
    const reason = status.error.charAt(0).toUpperCase() + status.error.slice(1);
    const resume = partial ? ` ${done} of ${total} are kept for the next try.` : "";
    return { tone: "warn", text: `${reason}.${resume}`, progress: null, action: "download", actionLabel };
  }
  const text = partial
    ? `Paused at ${done} of ${total}; resuming continues from there.`
    : `Not downloaded. ${total} from ${status.repo ?? "Hugging Face"}, checked against pinned checksums.`;
  return { tone: "quiet", text, progress: null, action: "download", actionLabel };
}
