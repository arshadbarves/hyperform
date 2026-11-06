<script lang="ts">
  import { plural } from "$lib/domain/format";
  import type { InterruptedHelper } from "$lib/domain/work/interrupted";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import type { Task } from "$lib/protocol/Task";
  import { discardAgentChanges } from "$lib/runtime";
  import { confirmStore } from "$lib/stores/confirm.svelte";
  import { Button } from "$lib/ui";
  import Icon, { GitBranch, ListChecks } from "$lib/ui/icons";
  import PetSprite from "../pet/PetSprite.svelte";

  /**
   * A task helper the plan interrupted: its worktree is kept for its task,
   * which picks up from it, so there is no Apply here. Discard throws the
   * work away once you confirm; the task then starts over. The task opens
   * in the Plan tab, where Show worktree stays.
   */
  type Props = {
    agent: AgentInfo;
    kept: InterruptedHelper;
    onOpen: (agentId: string) => void;
    onTask: (task: Task) => void;
  };
  let { agent, kept, onOpen, onTask }: Props = $props();

  let deciding = $state(false);
  const worktree = $derived(agent.worktree);

  async function discard() {
    const ok = await confirmStore.ask({
      title: "Discard this helper's work?",
      description: `“${kept.task.title}” no longer picks up from it and starts over when it runs again.`,
      confirmLabel: "Discard",
      tone: "danger",
    });
    if (!ok) return;
    deciding = true;
    await discardAgentChanges(agent.agentId);
    deciding = false;
  }
</script>

{#if worktree}
  <div class="apply-card">
    <button type="button" class="apply-card-main" onclick={() => onOpen(agent.agentId)} title="Open transcript">
      <span class="apply-card-title"><PetSprite status={agent.status} />{agent.agentType} · {agent.description}</span>
      <span class="apply-card-meta" title={worktree.path}>
        <Icon icon={GitBranch} size={11} />
        {worktree.branch} · {plural(worktree.filesChanged, "file")}{worktree.diffstat ? ` · ${worktree.diffstat}` : ""}
      </span>
    </button>
    <button type="button" class="apply-card-task" title="Open the task in the Plan tab" onclick={() => onTask(kept.task)}>
      <Icon icon={ListChecks} size={11} />
      <span>{kept.line}</span>
    </button>
    {#if kept.note}<p class="apply-card-hint">{kept.note}</p>{/if}
    <p class="apply-card-hint">{kept.hint}</p>
    <div class="apply-card-actions">
      <Button size="s" disabled={deciding} onclick={() => void discard()}>Discard</Button>
    </div>
  </div>
{/if}
