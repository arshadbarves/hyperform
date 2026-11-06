<script lang="ts">
  import type { TrustRequestView } from "$lib/domain/sessionView/types";
  import { answerTrust } from "$lib/runtime";
  import { Button } from "$lib/ui";
  import Icon, { ShieldAlert } from "$lib/ui/icons";

  type Props = { request: TrustRequestView };
  let { request }: Props = $props();

  let sending = $state(false);

  async function answer(trusted: boolean) {
    if (sending) return;
    sending = true;
    if (!(await answerTrust(trusted))) sending = false;
  }
</script>

<div class="interaction-card trust-banner" role="region" aria-label="Workspace trust" data-pending-card>
  <div class="interaction-kicker">
    <Icon icon={ShieldAlert} size={13} />
    <span>Trust this workspace?</span>
  </div>
  <p class="trust-banner-text">
    <code>{request.projectRoot}</code> defines {request.defines.join(", ")}. They stay off until you trust it,
    because they run commands on your machine.
  </p>
  <div class="interaction-actions">
    <Button variant="accent" disabled={sending} onclick={() => void answer(true)}>Trust this workspace</Button>
    <Button disabled={sending} onclick={() => void answer(false)}>Not now</Button>
  </div>
</div>
