<script lang="ts">
  import { revealPath } from "$lib/commands";
  import { attemptLine, integrationLine, resultView } from "$lib/domain/work/report";
  import { WEB_HINT, blockKind } from "$lib/domain/work/status";
  import type { Task } from "$lib/protocol/Task";
  import { editWork, errorText, pushToast } from "$lib/runtime";
  import { revealLabel } from "$lib/platform";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { Button } from "$lib/ui";

  /**
   * A task's Result: the summary, what was observed (changed files from
   * the diff, the checks that ran) before what the helper says, then what
   * it learned. Under it, how the helper's change came back, with Retry
   * serially and the worktree after a conflict, and which attempt this is.
   */
  type Props = { planId: string; task: Task; readOnly: boolean };
  let { planId, task, readOnly }: Props = $props();

  const result = $derived(resultView(task));
  const reported = $derived(task.run.report !== null);
  const integration = $derived(task.run.integration);
  const came = $derived(integrationLine(integration));
  const attempt = $derived(attemptLine(task, settingsStore.settings?.work.max_attempts ?? null));
  const conflicted = $derived(task.status.type === "blocked" && blockKind(task.status.reason) === "conflict");
  const worktree = $derived(integration?.type === "conflict" ? integration.worktree : null);

  async function reveal(path: string) {
    try {
      await revealPath(path);
    } catch (e) {
      pushToast(`Could not show the worktree · ${errorText(e)}`, "warn");
    }
  }
</script>

{#if result || came || attempt}
  <section class="task-section task-result" aria-label="Result">
    <h3 class="plan-view-heading">Result</h3>
    {#if result?.unreported}
      <p class="task-detail-why tone-attention">The helper ended without a report; what it changed is below.</p>
    {/if}
    {#if result?.summary}<p class="task-result-summary">{result.summary}</p>{/if}

    {#if result && reported}
      <dl class="task-result-facts">
        <dt>Changed</dt>
        <dd>
          {#if result.files === null}
            <span class="task-note">Not known; the diff could not be read.</span>
          {:else if result.files.length === 0}
            <span class="task-note">No files</span>
          {:else}
            <span class="task-chips">{#each result.files as file (file)}<span class="task-chip">{file}</span>{/each}</span>
          {/if}
        </dd>
        {#if result.tests.length}
          <dt>Checks</dt>
          <dd>
            <ul class="task-plain-list">
              {#each result.tests as check, i (i)}
                <li class:tone-ok={check.passed === true} class:tone-danger={check.passed === false}>
                  {check.label}{check.passed === true ? " · passed" : check.passed === false ? " · failed" : ""}
                </li>
              {/each}
            </ul>
          </dd>
        {/if}
      </dl>
    {/if}

    {#if result?.claims.length || result?.advice.length || result?.confidence}
      <div class="task-result-group">
        <p class="task-result-label">The helper says (not checked)</p>
        <dl class="notebook-entry-facts">
          {#each result.claims as claim (claim.label)}<dt>{claim.label}</dt><dd>{claim.items.join(", ")}</dd>{/each}
          {#each result.advice as part (part.label)}
            <dt>{part.label}</dt>
            <dd><ul class="task-plain-list">{#each part.items as item, i (i)}<li>{item}</li>{/each}</ul></dd>
          {/each}
          {#if result.confidence}<dt>Confidence</dt><dd>{result.confidence}</dd>{/if}
        </dl>
      </div>
    {/if}

    {#if result?.learned.length}
      <div class="task-result-group">
        <p class="task-result-label">
          Learned
          {#if result.fromWeb}<span class="task-row-badge" title={WEB_HINT}>from web · unconfirmed</span>{/if}
        </p>
        <dl class="notebook-entry-facts">
          {#each result.learned as part (part.label)}
            <dt>{part.label}</dt>
            <dd><ul class="task-plain-list">{#each part.items as item, i (i)}<li>{item}</li>{/each}</ul></dd>
          {/each}
        </dl>
      </div>
    {/if}

    {#if came}<p class={`task-result-line tone-${came.tone}`}>{came.text}</p>{/if}
    {#if integration?.type === "pendingApply"}
      <button type="button" class="task-link" onclick={() => ui.openPanel("agents")}>Review the change</button>
    {/if}
    {#if !readOnly && (conflicted || worktree)}
      <div class="task-result-actions">
        {#if conflicted}
          <Button size="s" variant="secondary" onclick={() => void editWork(planId, { type: "retrySerially", taskId: task.id })}>
            Retry serially
          </Button>
        {/if}
        {#if worktree}
          {@const path = worktree}
          <Button size="s" title={revealLabel()} onclick={() => void reveal(path)}>Show worktree</Button>
        {/if}
      </div>
    {/if}
    {#if attempt}<p class="task-detail-meta">{attempt}</p>{/if}
  </section>
{/if}
