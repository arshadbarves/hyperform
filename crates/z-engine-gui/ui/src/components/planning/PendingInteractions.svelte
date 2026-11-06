<script lang="ts" module>
  import type { SessionView } from "$lib/domain/sessionView";
  import { MAIN_AGENT } from "$lib/domain/sessionView";

  /** Who is asking, for requests that come from a subagent; null for the main agent. */
  export function agentLabel(view: SessionView, agentId: string): string | null {
    if (agentId === MAIN_AGENT) return null;
    const info = view.agents[agentId];
    return info ? `${info.agentType} · ${info.description}` : agentId;
  }
</script>

<script lang="ts">
  import ApprovalCard from "../chat/ApprovalCard.svelte";
  import QuestionCard from "./QuestionCard.svelte";

  /**
   * What waits on you, docked in the composer in place of the text box: the
   * oldest approval, else the oldest question. The rest follow one by one.
   */
  type Props = { view: SessionView };
  let { view }: Props = $props();

  const approval = $derived(Object.values(view.approvals)[0] ?? null);
  const question = $derived(Object.values(view.questions)[0] ?? null);
  const waiting = $derived(Object.keys(view.approvals).length + Object.keys(view.questions).length);
</script>

{#if approval}
  {#key approval.requestId}
    <ApprovalCard request={approval} agentLabel={agentLabel(view, approval.agentId)} autoFocus />
  {/key}
{:else if question}
  {#key question.requestId}
    <QuestionCard pending={question} agentLabel={agentLabel(view, question.agentId)} />
  {/key}
{/if}
{#if waiting > 1}
  <p class="dock-more">{waiting - 1} more waiting after this</p>
{/if}
