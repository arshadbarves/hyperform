<script lang="ts">
  import type { ToolCallState } from "$lib/domain/timeline/toolState";
  import { todoProgress } from "$lib/domain/todos";
  import { asObject, list } from "$lib/domain/tools/toolInput";
  import type { TodoItem } from "$lib/protocol/TodoItem";
  import { ListChecks } from "$lib/ui/icons";
  import TodoChecklist from "../../planning/TodoChecklist.svelte";
  import ToolFrame from "./ToolFrame.svelte";

  type Props = { call: ToolCallState };
  let { call }: Props = $props();

  const STATUSES = new Set(["pending", "in_progress", "completed"]);

  const todos = $derived(
    list(call.input, "todos").flatMap((raw): TodoItem[] => {
      const item = asObject(raw);
      if (!item || typeof item.content !== "string") return [];
      const status = typeof item.status === "string" && STATUSES.has(item.status) ? item.status : "pending";
      const activeForm = typeof item.activeForm === "string" ? item.activeForm : "";
      return [{ content: item.content, activeForm, status: status as TodoItem["status"] }];
    }),
  );
  const progress = $derived(todoProgress(todos));
  let open = $state(false);
</script>

<ToolFrame
  {call}
  icon={ListChecks}
  label="Todos"
  subject={progress.total ? `${progress.done}/${progress.total} done` : "cleared"}
  extra={progress.currentLabel || undefined}
  expandable={todos.length > 0}
  bind:open
>
  <TodoChecklist {todos} />
</ToolFrame>
