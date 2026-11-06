<script lang="ts">
  import { plural } from "$lib/domain/format";
  import type { AgentInfo } from "$lib/protocol/AgentInfo";
  import { applyAgentChanges, discardAgentChanges } from "$lib/runtime";
  import { Button } from "$lib/ui";
  import Icon, { GitBranch, GitMerge } from "$lib/ui/icons";
  import PetSprite from "../pet/PetSprite.svelte";

  /** A subagent's isolated work, waiting for you to bring it in or throw it away. */
  type Props = { agent: AgentInfo; onOpen: (agentId: string) => void };
  let { agent, onOpen }: Props = $props();

  let deciding = $state(false);
  const worktree = $derived(agent.worktree);

  async function decide(apply: boolean) {
    deciding = true;
    await (apply ? applyAgentChanges(agent.agentId) : discardAgentChanges(agent.agentId));
    deciding = false;
  }
</script>

{#if worktree}
  <div class={`apply-card worktree-${worktree.state}`}>
    <button type="button" class="apply-card-main" onclick={() => onOpen(agent.agentId)} title="Open transcript">
      <span class="apply-card-title"><PetSprite status={agent.status} />{agent.agentType} · {agent.description}</span>
      <span class="apply-card-meta" title={worktree.path}>
        <Icon icon={GitBranch} size={11} />
        {worktree.branch} · {plural(worktree.filesChanged, "file")}{worktree.diffstat ? ` · ${worktree.diffstat}` : ""}
      </span>
    </button>
    {#if worktree.state === "conflicted"}
      <p class="apply-card-note">Applying hit a conflict, so your files were left as they were.</p>
    {/if}
    <div class="apply-card-actions">
      <Button variant="accent" size="s" disabled={deciding} onclick={() => void decide(true)}>
        <Icon icon={GitMerge} size={12} />
        Apply
      </Button>
      <Button size="s" disabled={deciding} onclick={() => void decide(false)}>Discard</Button>
    </div>
  </div>
{/if}
