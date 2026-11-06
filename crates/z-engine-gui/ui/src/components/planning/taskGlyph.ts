import type { TaskState, TaskTone } from "$lib/domain/work/status";
import type { PillTone } from "$lib/ui";
import {
  AlertOctagon,
  AlertTriangle,
  CheckCircle2,
  Circle,
  Eye,
  Hourglass,
  LoaderCircle,
  MinusCircle,
  Pause,
  Repeat,
  type IconSvgElement,
} from "$lib/ui/icons";

/** One glyph per task state, colored by its tone in `plan-tree.css`. */
export const TASK_GLYPH: Record<TaskState, IconSvgElement> = {
  proposed: Circle,
  pending: Circle,
  ready: Circle,
  waiting: Hourglass,
  held: Pause,
  active: LoaderCircle,
  verifying: Eye,
  done: CheckCircle2,
  blocked: AlertTriangle,
  failed: AlertOctagon,
  cancelled: MinusCircle,
  superseded: Repeat,
};

/** A status tone as a pill's color. */
export const PILL_TONE: Record<TaskTone, PillTone> = {
  quiet: "neutral",
  neutral: "neutral",
  working: "working",
  ok: "ok",
  attention: "attention",
  danger: "danger",
};
