<script lang="ts">
  import type { AgentStatus } from "$lib/protocol/AgentStatus";
  import type { JobStatus } from "$lib/protocol/JobStatus";
  import { Pill, type PillTone } from "$lib/ui";

  /** A helper's or job's status: a dot and one word; running pulses. */
  type Props = { status: AgentStatus | JobStatus };
  let { status }: Props = $props();

  const LOOK: Record<AgentStatus | JobStatus, [string, PillTone]> = {
    running: ["Running", "working"],
    waiting: ["Waiting", "attention"],
    completed: ["Done", "neutral"],
    failed: ["Failed", "danger"],
    cancelled: ["Cancelled", "neutral"],
    killed: ["Killed", "danger"],
  };
  const look = $derived(LOOK[status]);
</script>

<Pill tone={look[1]} dot plain live={status === "running"}>{look[0]}</Pill>
