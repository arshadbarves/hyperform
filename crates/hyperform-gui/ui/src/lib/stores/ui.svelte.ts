import { clampPanelWidth, PANEL_DEFAULT_W, type PanelTab } from "../domain/sidePanel";
import type { StageView } from "../domain/stage";
import { emptyPlanView, stepBack, type PlanViewState } from "../domain/work/panel";

export type SettingsTab =
  | "models"
  | "providers"
  | "permissions"
  | "hooks"
  | "extensions"
  | "mcp"
  | "verification"
  | "memory"
  | "advanced"
  | "experimental"
  | "appearance"
  | "pet"
  | "about";
export type WorkTab = "agents" | "jobs";
export type DiffLayout = "unified" | "split";
/** This chat's edits, or every uncommitted change in the project. */
export type DiffScope = "session" | "git";

/** The right-hand side panel. `tab` is kept while it is closed, so it reopens where it was. */
export interface PanelState {
  tab: PanelTab;
  open: boolean;
  /** Covers the whole stage instead of docking beside the chat. */
  expanded: boolean;
  /** Docked width in pixels, clamped by `clampPanelWidth`. */
  width: number;
}

const DIFF_LAYOUT_KEY = "hyperform.diffLayout";
const PANEL_WIDTH_KEY = "hyperform.panelWidth";

function stored(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function remember(key: string, value: string) {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch {
    /* private mode: remember for this run only */
  }
}

/** The Plan tab's place in each plan: the open task, rows opened by hand, the notebook tab and scroll. */
class PlanUi {
  #views = $state<Record<string, PlanViewState>>({});
  /** Scroll offsets by plan and place; not reactive, since every scroll writes one. */
  #scroll = new Map<string, number>();
  /** The task whose packet the Context tab shows; null follows the main agent's latest. */
  packetTask = $state<string | null>(null);
  /** The chat whose Plan tab lists the project's plans in place of the one attached. */
  pickingFor = $state<string | null>(null);

  of(planId: string): PlanViewState {
    return this.#views[planId] ?? emptyPlanView();
  }

  scrollOf(planId: string, place: string): number {
    return this.#scroll.get(`${planId}\n${place}`) ?? 0;
  }

  keepScroll(planId: string, place: string, top: number): void {
    this.#scroll.set(`${planId}\n${place}`, top);
  }

  /** List the project's plans in `sessionId`'s Plan tab, or (null) go back to its plan. */
  pick(sessionId: string | null): void {
    this.pickingFor = sessionId;
  }

  set(planId: string, patch: Partial<PlanViewState>): void {
    this.#views[planId] = { ...this.of(planId), ...patch };
  }

  select(planId: string, taskId: string | null): void {
    this.set(planId, { selected: taskId, view: "tasks" });
  }

  setExpanded(planId: string, taskId: string, open: boolean): void {
    this.set(planId, { expanded: { ...this.of(planId).expanded, [taskId]: open } });
  }

  /** Esc out of a task's detail, then the findings; false when neither was open. */
  back(planId: string): boolean {
    const next = stepBack(this.of(planId));
    if (next) this.#views[planId] = next;
    return next !== null;
  }
}

/** App chrome state (overlays, the side panel, sidebar, stage page). UI-only; no engine I/O. */
class UiStore {
  sidebarOpen = $state(true);
  /** The page the main stage is asked for; `stageFor` resolves what it shows. */
  view = $state<StageView>("home");
  paletteOpen = $state(false);
  /** Palette opened by `/resume`: chats only. */
  paletteSessionsOnly = $state(false);
  settingsOpen = $state(false);
  settingsTab = $state<SettingsTab>("providers");
  /** A setting to scroll to and highlight when Settings opens (deep search). */
  settingsFocus = $state<string | null>(null);
  worktreeOpen = $state(false);

  panel = $state<PanelState>({
    tab: "changes",
    open: false,
    expanded: false,
    width: clampPanelWidth(Number(stored(PANEL_WIDTH_KEY) ?? PANEL_DEFAULT_W)),
  });
  /** Tabs with news the user has not looked at yet (a helper started). */
  panelBadges = $state<PanelTab[]>([]);
  /** A file the Changes tab should select when it opens, and the scope to show it in. */
  diffFocus = $state<string | null>(null);
  diffScope = $state<DiffScope | null>(null);
  /** Unified or side-by-side; remembered on this machine. */
  diffLayout = $state<DiffLayout>(stored(DIFF_LAYOUT_KEY) === "split" ? "split" : "unified");
  /** Helpers or background jobs, inside the Agents tab. */
  workTab = $state<WorkTab>("agents");
  /** Agent whose transcript the Agents tab shows. */
  agentTranscript = $state<string | null>(null);
  plan = new PlanUi();

  togglePalette() {
    this.paletteSessionsOnly = false;
    this.paletteOpen = !this.paletteOpen;
  }

  openPalette(sessionsOnly = false) {
    this.paletteSessionsOnly = sessionsOnly;
    this.paletteOpen = true;
  }

  /** Without a page, Settings opens where it was last left. */
  openSettings(tab: SettingsTab | null = null, focus: string | null = null) {
    if (tab) this.settingsTab = tab;
    this.settingsFocus = focus;
    this.settingsOpen = true;
  }

  openWorktree() {
    this.worktreeOpen = true;
  }

  setDiffLayout(layout: DiffLayout) {
    this.diffLayout = layout;
    remember(DIFF_LAYOUT_KEY, layout);
  }

  /** The side panel tab showing, or null when the panel is closed. */
  get panelTab(): PanelTab | null {
    return this.panel.open ? this.panel.tab : null;
  }

  /** The open panel covers the stage. */
  get panelExpanded(): boolean {
    return this.panel.open && this.panel.expanded;
  }

  /**
   * Show `tab`. `target` is the file to select (changes) or the agent to
   * follow (agents); `scope` picks which changes the file is shown in. A
   * closed panel opens docked.
   */
  openPanel(tab: PanelTab, target: string | null = null, scope: DiffScope | null = null): void {
    if (!this.panel.open) this.panel.expanded = false;
    if (tab === "changes") {
      this.diffFocus = target;
      this.diffScope = scope;
    } else if (tab === "agents") {
      this.worktreeOpen = false;
      this.workTab = "agents";
      this.agentTranscript = target;
    }
    this.showPanelTab(tab);
  }

  /** Switch tabs from the panel's own tab strip; each tab stays as it was left. */
  showPanelTab(tab: PanelTab): void {
    this.panel.tab = tab;
    this.panel.open = true;
    this.panelBadges = this.panelBadges.filter((t) => t !== tab);
  }

  /** The Plan tab, on a task or a notebook entry when `focus` says so. */
  openPlan(planId: string, focus: Partial<PlanViewState> = {}): void {
    this.plan.set(planId, focus);
    this.plan.pick(null);
    this.openPanel("plan");
  }

  /** The Plan tab listing the project's plans to continue in `sessionId`. */
  openPlanPicker(sessionId: string): void {
    this.plan.pick(sessionId);
    this.openPanel("plan");
  }

  /** The Context tab on the packet a task's agent was given. */
  openPacket(taskId: string): void {
    this.plan.packetTask = taskId;
    this.openPanel("context");
  }

  /** The Agents tab on its background jobs. */
  openJobs(): void {
    this.openPanel("agents");
    this.workTab = "jobs";
  }

  /** `expanded` is left as it was, so the panel leaves in the shape it had. */
  closePanel(): void {
    this.panel.open = false;
    this.agentTranscript = null;
  }

  /** With a tab: show it, or close when it is already showing. Without one: open where it was, or close. */
  togglePanel(tab?: PanelTab): void {
    if (tab ? this.panelTab === tab : this.panel.open) this.closePanel();
    else this.openPanel(tab ?? this.panel.tab);
  }

  setPanelExpanded(expanded: boolean): void {
    this.panel.expanded = expanded && this.panel.open;
  }

  /** Resize the docked panel within `room` (the stage and panel together); `save` remembers it on this machine. */
  setPanelWidth(width: number, room?: number, save = false): void {
    this.panel.width = clampPanelWidth(width, room);
    if (save) remember(PANEL_WIDTH_KEY, String(this.panel.width));
  }

  /** Mark tabs as having news, except the one showing. */
  badgePanel(tabs: readonly PanelTab[]): void {
    const add = tabs.filter((t) => t !== this.panelTab && !this.panelBadges.includes(t));
    if (add.length) this.panelBadges = [...this.panelBadges, ...add];
  }
}

export const ui = new UiStore();
