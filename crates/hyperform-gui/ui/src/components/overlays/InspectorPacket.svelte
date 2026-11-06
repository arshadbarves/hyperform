<script lang="ts">
  import { untrack } from "svelte";
  import { MAIN_AGENT } from "$lib/domain/sessionView";
  import { latestPackets, packetSections, packetSize, sentReason } from "$lib/domain/work/packet";
  import type { TaskPacket } from "$lib/protocol/TaskPacket";
  import { sessions, taskPacket } from "$lib/runtime";
  import { ui } from "$lib/stores/ui.svelte";
  import { Disclosure } from "$lib/ui";
  import { fmtTokens, relTime } from "$lib/util";

  /**
   * With a work plan attached: the task packet an agent was given, section
   * by section, with why each notebook entry made it in and what was left
   * out for the budget. Follows the main agent's latest packet unless a
   * task's detail asked for its own.
   */
  const view = $derived(sessions.active);
  const work = $derived(view?.work ?? null);
  const latest = $derived(view ? latestPackets(view.packets) : []);
  const asked = $derived(ui.plan.packetTask && work?.tasks[ui.plan.packetTask] ? ui.plan.packetTask : null);
  const taskId = $derived(asked ?? latest[0]?.taskId ?? null);
  const sent = $derived(latest.find((p) => p.taskId === taskId) ?? null);
  const title = $derived(taskId ? (work?.tasks[taskId]?.title ?? taskId) : "");

  type State = { kind: "loading" } | { kind: "ready"; packet: TaskPacket } | { kind: "error"; error: string };
  let open = $state(untrack(() => ui.plan.packetTask !== null));
  let load = $state<State>({ kind: "loading" });
  const sections = $derived(load.kind === "ready" && work ? packetSections(load.packet, work.tasks) : []);

  $effect(() => {
    if (ui.plan.packetTask) open = true;
  });

  // Fresh when it opens, for another task, and after each new packet goes out.
  $effect(() => {
    const sessionId = view?.sessionId;
    const id = taskId;
    void sent?.at;
    if (!open || !sessionId || !id) return;
    let live = true;
    load = { kind: "loading" };
    void taskPacket(sessionId, id).then((r) => {
      if (live) load = r.ok ? { kind: "ready", packet: r.value } : { kind: "error", error: r.error };
    });
    return () => {
      live = false;
    };
  });

  function agentName(agentId: string): string {
    if (agentId === MAIN_AGENT) return "Main agent";
    return view?.agents[agentId]?.agentType ?? "Helper";
  }
</script>

{#if work && taskId}
  <Disclosure bind:open class="inspector-insights inspector-packet" summaryClass="inspector-insights-summary">
    {#snippet summary()}
      <span>Task packet</span>
      <span class="inspector-insights-hint">{title}</span>
    {/snippet}
    {#if latest.length > 1}
      <div class="packet-agents" role="group" aria-label="Packets by agent">
        {#each latest as p (p.agentId)}
          <button
            type="button"
            class="packet-agent"
            class:is-active={p.taskId === taskId}
            aria-pressed={p.taskId === taskId}
            onclick={() => (ui.plan.packetTask = p.taskId)}
          >
            {agentName(p.agentId)} · {work.tasks[p.taskId]?.title ?? p.taskId}
          </button>
        {/each}
      </div>
    {/if}
    <p class="packet-note">
      {sent ? `Sent ${sentReason(sent.reason)} · ${relTime(sent.at)}` : "Not sent yet: what the next packet for this task holds."}
    </p>
    {#if load.kind === "loading"}
      <p class="packet-note">Building the packet…</p>
    {:else if load.kind === "error"}
      <p class="packet-note">Not available · {load.error}</p>
    {:else}
      <p class="packet-note">{packetSize(load.packet)}</p>
      {#each sections as section (section.title)}
        <section class="packet-section">
          <h4>{section.title}</h4>
          <ul>
            {#each section.lines as line (line.key)}
              <li>
                <span class="packet-line-text">{line.text}</span>
                {#if line.why}<span class="packet-line-why">{line.why}</span>{/if}
                {#if line.tokens !== null}<span class="packet-line-tokens">{fmtTokens(line.tokens)}</span>{/if}
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    {/if}
  </Disclosure>
{/if}
