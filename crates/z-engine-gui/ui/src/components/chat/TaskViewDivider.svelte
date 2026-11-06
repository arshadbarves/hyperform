<script lang="ts">
  import { taskViewDivider } from "$lib/domain/timeline/taskViewDivider";
  import type { TaskViewInfo } from "$lib/protocol/TaskViewInfo";
  import { includeFullHistory } from "$lib/runtime";
  import { Button } from "$lib/ui";

  /** Where a new task set earlier exchanges aside (`decisions_task_view`), with the way back. */
  type Props = { view: TaskViewInfo; latest: boolean };
  let { view, latest }: Props = $props();

  const text = $derived(taskViewDivider(view, latest));
  let sending = $state(false);

  async function restore() {
    sending = true;
    await includeFullHistory();
    sending = false;
  }
</script>

<div class="task-view-divider" role="note">
  <span class="task-view-rule" aria-hidden="true"></span>
  <span class="task-view-label">{text.label}</span>
  {#if text.canRestore}
    <Button variant="ghost" size="s" disabled={sending} onclick={() => void restore()}>Include full history</Button>
  {/if}
  <span class="task-view-rule" aria-hidden="true"></span>
</div>
