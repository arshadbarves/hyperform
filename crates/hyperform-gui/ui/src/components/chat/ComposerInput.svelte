<script lang="ts">
  import { untrack } from "svelte";
  import { replaceAtToken } from "$lib/atFile";
  import type { SlashCommandInfo } from "$lib/commands";
  import { createComposerHistory } from "$lib/domain/composerHistory";
  import { composerIntent } from "$lib/domain/composerKeys";
  import { activePopover, agentMention, filterAgents, wrapIndex, type MentionItem } from "$lib/domain/composerPopover";
  import { planSubmission } from "$lib/domain/composerSubmit";
  import { togglePlanning } from "$lib/domain/modes";
  import { REMEMBER_TARGETS, type RememberScope } from "$lib/domain/remember";
  import { filterCommands, menuOrder } from "$lib/domain/slashCommands";
  import { cancelTurn, catalogs, searchFiles, sessions, setPlanning } from "$lib/runtime";
  import { hideShell } from "$lib/shellStore";
  import { composer } from "$lib/stores/composer.svelte";
  import { executePlan, rememberNote } from "$lib/stores/composerSubmit";
  import { userSignals } from "$lib/stores/userSignals.svelte";
  import MentionPopover from "./MentionPopover.svelte";
  import RememberPopover from "./RememberPopover.svelte";
  import SlashPopover from "./SlashPopover.svelte";

  /**
   * The draft: the textarea, its `/` `@` `#` pop-ups, history and keys.
   * The parent calls `submit` (the Send button) and `insert` (the + menu).
   */
  type Props = {
    root: string | null;
    busy: boolean;
    hero: boolean;
    shellVisible: boolean;
    onPasteFiles: (e: ClipboardEvent) => void;
  };
  let { root, busy, hero, shellVisible, onPasteFiles }: Props = $props();

  const history = createComposerHistory();
  let ta: HTMLTextAreaElement | undefined = $state();
  let caret = $state(0);
  let dismissed = $state(false);
  let selected = $state(0);
  let files = $state<{ q: string; list: string[] }>({ q: "\u0000", list: [] });

  const view = $derived(sessions.active);
  const commands = $derived(catalogs.commandsFor(root));
  const text = $derived(composer.draft);
  const shellMode = $derived(text.startsWith("!"));
  const popover = $derived(dismissed ? null : activePopover(text, caret));
  const slashItems = $derived(popover?.kind === "slash" ? menuOrder(filterCommands(commands, popover.query)) : []);
  const mentionQuery = $derived(popover?.kind === "mention" ? popover.query : null);
  const mentionItems: MentionItem[] = $derived(
    mentionQuery === null
      ? []
      : [
          ...filterAgents(catalogs.agentsFor(root), mentionQuery).map((agent) => ({ kind: "agent" as const, agent })),
          ...(files.q === mentionQuery ? files.list : []).map((path) => ({ kind: "file" as const, path })),
        ],
  );
  const itemCount = $derived(
    popover?.kind === "slash" ? slashItems.length : popover?.kind === "mention" ? mentionItems.length : popover?.kind === "remember" ? REMEMBER_TARGETS.length : 0,
  );
  const placeholder = $derived(
    shellMode
      ? "A shell command, run in the project (e.g. !git status)"
      : busy
        ? "Add to what the agent is doing…"
        : hero
          ? "Describe a change, ask a question, or type / for commands"
          : "Ask anything · @ to mention files · / for commands",
  );

  $effect(() => {
    void catalogs.ensure(root);
  });

  $effect(() => {
    const q = mentionQuery;
    if (q === null) return;
    let alive = true;
    const timer = setTimeout(() => {
      void searchFiles(root, q).then((list) => {
        if (alive) files = { q, list };
      });
    }, 120);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  });

  $effect(() => {
    const el = ta;
    if (composer.focusTick === 0 || !el) return;
    untrack(() => {
      const end = composer.draft.length;
      el.focus();
      el.setSelectionRange(end, end);
      caret = end;
    });
  });

  function syncCaret() {
    if (ta) caret = ta.selectionStart;
  }

  function setText(next: string, nextCaret = next.length) {
    composer.setDraft(next, false);
    caret = nextCaret;
    selected = 0;
    requestAnimationFrame(() => {
      ta?.focus();
      ta?.setSelectionRange(nextCaret, nextCaret);
    });
  }

  /** Put a prefix (`@`, `/`, `# `, `!`) at the caret, as if typed; `/`, `#` and `!` start the draft. */
  export function insert(prefix: string) {
    dismissed = false;
    if (prefix === "@") {
      const at = caret || text.length;
      const before = text.slice(0, at);
      const glue = before && !/\s$/.test(before) ? " " : "";
      setText(`${before}${glue}@${text.slice(at)}`, at + glue.length + 1);
      return;
    }
    setText(`${prefix}${text.replace(/^[/#!]\s?/, "")}`, prefix.length);
  }

  function onInput(e: Event & { currentTarget: HTMLTextAreaElement }) {
    composer.setDraft(e.currentTarget.value, false);
    userSignals.typed();
    caret = e.currentTarget.selectionStart;
    dismissed = false;
    selected = 0;
    history.reset();
  }

  function pickSlash(command: SlashCommandInfo, run: boolean) {
    if (command.argumentHint || !run) setText(`/${command.name} `);
    else void submit(false, `/${command.name}`);
  }

  function pickMention(item: MentionItem) {
    const inserted = item.kind === "agent" ? agentMention(item.agent) : `@${item.path} `;
    if (item.kind === "file") composer.addFile(item.path);
    const next = replaceAtToken(text, caret, inserted);
    setText(next.text, next.caret);
  }

  async function pickRemember(scope: RememberScope) {
    const note = text.trim().replace(/^#/, "").trim();
    if (note && (await rememberNote(scope, note))) caret = 0;
  }

  function pickSelected(run: boolean) {
    if (popover?.kind === "slash" && slashItems[selected]) pickSlash(slashItems[selected], run);
    else if (popover?.kind === "mention" && mentionItems[selected]) pickMention(mentionItems[selected]);
    else if (popover?.kind === "remember") void pickRemember(REMEMBER_TARGETS[selected]?.scope ?? "project");
  }

  export async function submit(interrupt: boolean, override?: string) {
    const raw = override ?? text;
    let plan = planSubmission({ text: raw, attachments: composer.attachments, busy, interrupt, commands });
    if (plan.kind === "remember") {
      if (!dismissed) return pickRemember(REMEMBER_TARGETS[selected]?.scope ?? "project");
      plan = busy ? { kind: "steer", text: raw.trim() } : { kind: "submit", text: raw.trim(), attachments: composer.attachments };
    }
    if (plan.kind === "none") return;
    history.push(raw.trim());
    dismissed = false;
    caret = 0;
    await executePlan(plan);
  }

  function onKeyDown(e: KeyboardEvent) {
    syncCaret();
    if (popover && itemCount > 0 && !e.isComposing) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        e.preventDefault();
        selected = wrapIndex(selected, e.key === "ArrowDown" ? 1 : -1, itemCount);
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey && !e.metaKey && !e.ctrlKey)) {
        e.preventDefault();
        pickSelected(e.key === "Enter");
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        dismissed = true;
        return;
      }
    }
    if (e.key === "Escape" && !busy && shellVisible) {
      e.preventDefault();
      hideShell();
      return;
    }
    switch (composerIntent(e, { busy, text, caret })) {
      case "send":
        e.preventDefault();
        void submit(false);
        break;
      case "interrupt":
        e.preventDefault();
        void submit(true);
        break;
      case "cancel":
        e.preventDefault();
        void cancelTurn();
        break;
      case "clear":
        e.preventDefault();
        setText("");
        break;
      case "historyPrev":
      case "historyNext": {
        const next = e.key === "ArrowUp" ? history.prev(text) : history.next();
        if (next === null) break;
        e.preventDefault();
        setText(next);
        break;
      }
      case "togglePlan":
        e.preventDefault();
        void setPlanning(togglePlanning(view?.planning ?? false));
        break;
    }
  }
</script>

{#if popover?.kind === "slash"}
  <SlashPopover items={slashItems} {selected} onPick={(c) => pickSlash(c, true)} onHover={(i) => (selected = i)} />
{:else if popover?.kind === "mention"}
  <MentionPopover items={mentionItems} loading={files.q !== mentionQuery} {selected} onPick={pickMention} onHover={(i) => (selected = i)} />
{:else if popover?.kind === "remember"}
  <RememberPopover {selected} onPick={(scope) => void pickRemember(scope)} onHover={(i) => (selected = i)} />
{/if}
<div class={`composer-input-area${shellMode ? " shell-active" : ""}`}>
  <textarea
    bind:this={ta}
    rows={hero ? 3 : 2}
    class={shellMode ? "shell-textarea" : ""}
    aria-label="Message"
    {placeholder}
    value={text}
    oninput={onInput}
    onselect={syncCaret}
    onclick={syncCaret}
    onkeyup={syncCaret}
    onkeydown={onKeyDown}
    onpaste={onPasteFiles}
  ></textarea>
</div>
