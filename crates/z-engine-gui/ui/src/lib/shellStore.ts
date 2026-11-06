type Listener = () => void;

export interface ShellEntry {
  id: number;
  cmd: string;
  lines: string[];
}

export interface ShellState {
  visible: boolean;
  entries: ShellEntry[];
}

const listeners = new Set<Listener>();
let nextId = 1;
let state: ShellState = { visible: false, entries: [] };

function emit() {
  for (const l of listeners) l();
}

export const shellStore = {
  subscribe(l: Listener) {
    listeners.add(l);
    return () => {
      listeners.delete(l);
    };
  },
  getSnapshot(): ShellState {
    return state;
  },
};

/** Open the overlay and start a new `!` command block. */
export function startShell(cmd: string) {
  state = {
    visible: true,
    entries: [...state.entries.slice(-19), { id: nextId++, cmd, lines: [] }],
  };
  emit();
}

/** Output of a `!` command (a `commandOutput` named `shell`); a code fence is unwrapped. */
export function appendShellOutput(raw: string) {
  const text = raw.replace(/^```[^\n]*\n([\s\S]*?)\n?```\s*$/, "$1").replace(/\n$/, "");
  const lines = text ? text.split("\n") : [];
  const entries = state.entries.slice();
  const last = entries[entries.length - 1];
  if (!last) entries.push({ id: nextId++, cmd: "", lines });
  else entries[entries.length - 1] = { ...last, lines: [...last.lines, ...lines] };
  state = { visible: true, entries };
  emit();
}

export function hideShell() {
  if (!state.visible) return;
  state = { ...state, visible: false };
  emit();
}

export function showShell() {
  if (state.visible || state.entries.length === 0) return;
  state = { ...state, visible: true };
  emit();
}

export function clearShell() {
  state = { ...state, entries: [] };
  emit();
}

export function resetShell() {
  nextId = 1;
  state = { visible: false, entries: [] };
  emit();
}
