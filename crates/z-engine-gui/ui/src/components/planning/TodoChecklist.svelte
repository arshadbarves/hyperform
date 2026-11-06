<script lang="ts">
  import type { TodoItem } from "$lib/protocol/TodoItem";
  import Icon, { CheckSquare, Circle, LoaderCircle } from "$lib/ui/icons";

  type Props = { todos: TodoItem[] };
  let { todos }: Props = $props();
</script>

<ul class="todo-checklist">
  {#each todos as todo, i (i)}
    <li class={`todo-item todo-${todo.status}`}>
      <span class="todo-glyph" aria-hidden="true">
        {#if todo.status === "completed"}
          <Icon icon={CheckSquare} size={12} />
        {:else if todo.status === "in_progress"}
          <Icon icon={LoaderCircle} size={12} class="spin" />
        {:else}
          <Icon icon={Circle} size={12} />
        {/if}
      </span>
      <span class="todo-text">
        {todo.status === "in_progress" && todo.activeForm ? todo.activeForm : todo.content}
      </span>
    </li>
  {/each}
</ul>
