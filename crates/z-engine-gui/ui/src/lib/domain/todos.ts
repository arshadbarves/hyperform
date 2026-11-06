import type { TodoItem } from "../protocol/TodoItem";

export interface TodoProgress {
  done: number;
  total: number;
  /** The item in progress, else the next pending one. */
  current: TodoItem | null;
  /** Label for the strip: the in-progress `activeForm`, falling back to `content`. */
  currentLabel: string;
  allDone: boolean;
}

export function todoProgress(todos: TodoItem[] | undefined): TodoProgress {
  const list = todos ?? [];
  const done = list.filter((t) => t.status === "completed").length;
  const active = list.find((t) => t.status === "in_progress") ?? null;
  const next = active ?? list.find((t) => t.status === "pending") ?? null;
  const currentLabel = active ? active.activeForm || active.content : next?.content ?? "";
  return {
    done,
    total: list.length,
    current: next,
    currentLabel,
    allDone: list.length > 0 && done === list.length,
  };
}
