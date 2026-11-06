import { MODES, PLAN, togglePlanning } from "./domain/modes";
import { SETTING_ENTRIES } from "./domain/settings/searchIndex";
import { settingWrite } from "./domain/settings/tomlValue";
import { canPickPlan, planPicker } from "./domain/work/picker";
import type { PaletteItem } from "./paletteTypes";
import { modLabel } from "./platform";
import {
  activeProjectRoot,
  compact,
  exportTranscript,
  projectPlans,
  sessionList,
  sessions,
  setMode,
  setPlanning,
} from "./runtime";
import { goHome, openSettings, showInbox } from "./stores/app-actions";
import { pickPlan, showPlanPicker } from "./stores/plan-actions";
import { petUi } from "./stores/pet.svelte";
import { settingsStore } from "./stores/settings.svelte";
import { ui } from "./stores/ui.svelte";
import { runUiCommand } from "./stores/uiCommands";
import {
  Bot,
  Brain,
  Copy,
  Eye,
  FolderPlus,
  GitBranch,
  GitCompare,
  HelpCircle,
  Home,
  Inbox,
  ListChecks,
  PanelLeft,
  Plus,
  Settings,
  Shield,
  Smile,
  SquareTerminal,
  Workflow,
} from "./ui/icons";

/** What you can ask of the pet, while there is one. */
function petActions(): PaletteItem[] {
  const level = settingsStore.settings?.ui.companion ?? "lively";
  if (level === "off") return [];
  const name = petUi.name;
  const pet = (item: Omit<PaletteItem, "group" | "icon">): PaletteItem => ({ ...item, group: "Pet", icon: Smile });
  const roam = petUi.roam;
  const setRoam = (next: boolean) => {
    settingsStore.scope = "user";
    void settingsStore.apply([settingWrite(["ui", "pet", "roam"], next)]);
  };
  return [
    pet({ label: `Rename ${name}`, keywords: "pet companion name call", run: () => openSettings("pet", "ui.pet.name") }),
    pet({ label: `Change ${name}'s look`, keywords: "pet companion color skin look", run: () => openSettings("pet", "ui.pet.look") }),
    pet({
      label: `Show ${name}'s card`,
      hint: "Level, streak and what it wears",
      keywords: "pet companion level xp card unlocks",
      run: () => petUi.openCard(),
    }),
    ...(level === "lively"
      ? [
          pet({
            label: roam ? `Stop ${name} roaming` : `Let ${name} roam`,
            keywords: "pet companion walk wander roam stay",
            run: () => setRoam(!roam),
          }),
          pet({ label: `Call ${name} back`, hint: "To the title bar", keywords: "pet companion home island return", run: () => petUi.callBack() }),
        ]
      : []),
  ];
}

/** With work plans on: the project's open plans, to continue in this chat, and the full list. */
function workPlanItems(): PaletteItem[] {
  const root = activeProjectRoot();
  const current = sessions.active?.work?.plan.planId ?? null;
  const plans = projectPlans.of(root);
  if (!canPickPlan(plans, current)) return [];
  const picker = planPicker(plans, { currentPlanId: current, sessionId: sessions.activeId, chats: sessionList.items });
  const plan = (item: Omit<PaletteItem, "group" | "icon">): PaletteItem => ({ ...item, group: "Plans", icon: Workflow });
  return [
    ...picker.open.map((row) =>
      plan({
        label: `Continue plan: ${row.plan.goal}`,
        hint: [row.status, row.progress, row.needs].filter(Boolean).join(" · "),
        keywords: "plan continue resume attach work task",
        run: () => void pickPlan(row),
      }),
    ),
    plan({ label: "Continue a plan…", hint: "Every plan in this project", keywords: "plan continue attach switch work", run: () => void showPlanPicker() }),
  ];
}

/** Everything the palette can do, in plain words; settings appear once you type. */
export function paletteActions(opts: {
  newTask: () => void;
  addWorkspace: () => void;
  openWorktree: () => void;
  openSettings: () => void;
  toggleSidebar: () => void;
}): PaletteItem[] {
  const mode = sessions.active?.mode ?? "default";
  const planning = sessions.active?.planning ?? false;
  const mod = modLabel();
  const go = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "Go to" });
  const act = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "Actions" });
  const chat = (item: Omit<PaletteItem, "group">): PaletteItem => ({ ...item, group: "This chat" });
  return [
    act({ label: "New chat", keywords: "new task session create start", icon: Plus, shortcut: `${mod}N`, run: opts.newTask }),
    act({ label: "New chat in a worktree…", hint: "On its own branch", keywords: "worktree branch isolate parallel", icon: GitBranch, run: opts.openWorktree }),
    act({ label: "Add a project…", hint: "Choose a folder", keywords: "add open folder workspace project", icon: FolderPlus, run: opts.addWorkspace }),
    go({ label: "Home", keywords: "home start projects", icon: Home, run: () => goHome() }),
    go({ label: "Inbox", hint: "Approvals, finished chats, notices", keywords: "inbox activity notifications approvals", icon: Inbox, run: () => showInbox() }),
    go({ label: "Settings", keywords: "settings preferences config", icon: Settings, shortcut: `${mod},`, run: opts.openSettings }),
    go({ label: "Show or hide the sidebar", keywords: "toggle sidebar", icon: PanelLeft, shortcut: `${mod}B`, run: opts.toggleSidebar }),
    chat({ label: "Review changes", hint: "What this chat changed", keywords: "diff review changes files git", icon: GitCompare, shortcut: `${mod}D`, run: () => ui.openPanel("changes") }),
    chat({ label: "Plan", hint: "The plan and its checklist", keywords: "plan todo checklist review approve", icon: ListChecks, run: () => ui.openPanel("plan") }),
    chat({ label: "Agents", hint: "Helpers and work to apply", keywords: "agents subagents helpers worktree usage cost", icon: Bot, run: () => ui.openPanel("agents") }),
    chat({ label: "Background jobs", keywords: "jobs background shell stop output", icon: SquareTerminal, run: () => ui.openJobs() }),
    chat({ label: "Inspect the prompt", hint: "The last request, part by part", keywords: "inspect prompt request context tokens system tools", icon: Eye, run: () => ui.openPanel("context") }),
    chat({ label: "Context usage", keywords: "context tokens window breakdown /context", icon: Brain, run: () => void runUiCommand("context", "") }),
    chat({ label: "Compact the conversation", hint: "Summarize older messages", keywords: "compact summarize free tokens /compact", icon: Brain, run: () => void compact() }),
    chat({ label: "Copy the chat as Markdown", keywords: "export transcript markdown share", icon: Copy, run: () => void exportTranscript("markdown") }),
    chat({ label: "Copy the chat as JSON", keywords: "export transcript json", icon: Copy, run: () => void exportTranscript("json") }),
    ...petActions(),
    ...workPlanItems(),
    {
      label: planning ? PLAN.off : PLAN.on,
      hint: PLAN.description,
      keywords: "plan mode research propose read-only build",
      group: "Permission mode",
      icon: ListChecks,
      shortcut: "⇧Tab",
      run: () => void setPlanning(togglePlanning(planning)),
    },
    ...MODES.filter((m) => m.id !== mode && !m.warning).map((m) => ({
      label: `Switch to ${m.label}`,
      hint: m.description,
      keywords: `mode permission ${m.id}`,
      group: "Permission mode",
      icon: Shield,
      run: () => void setMode(m.id),
    })),
    {
      label: "Commands and shortcuts",
      keywords: "help commands keys shortcuts /help",
      group: "Help",
      icon: HelpCircle,
      run: () => void runUiCommand("help", ""),
    },
  ];
}

/** Single settings, found by name or by the words people use; they open at their row. */
export function paletteSettings(): PaletteItem[] {
  return SETTING_ENTRIES.map((entry) => ({
    label: entry.title,
    keywords: `setting ${entry.words}`,
    group: "Settings",
    icon: Settings,
    run: () => openSettings(entry.tab, entry.key),
  }));
}
