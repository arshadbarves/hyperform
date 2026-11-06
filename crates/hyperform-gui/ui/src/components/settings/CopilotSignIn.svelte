<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { openReleaseUrl } from "$lib/commands";
  import { COPILOT_HOST_OPTIONS, type CopilotHost } from "$lib/domain/settings/copilotLogin";
  import type { KeyStatus } from "$lib/protocol/config/KeyStatus";
  import { copilotBaseUrl, copilotEnterpriseDomain } from "$lib/providers";
  import { CopilotLogin } from "$lib/stores/copilotLogin.svelte";
  import { Button, copyFeedback, SegmentedChoice } from "$lib/ui";
  import Icon, { Check, Copy, ExternalLink, LoaderCircle } from "$lib/ui/icons";

  /**
   * Copilot's credential in place of an API key field: GitHub.com or an
   * Enterprise domain, then GitHub's device sign-in with the code to enter.
   */
  type Props = {
    baseUrl: string;
    /** The token saved for `baseUrl`'s bucket. */
    saved: KeyStatus;
    onBaseUrl: (url: string) => void;
  };

  let { baseUrl, saved, onBaseUrl }: Props = $props();

  const initialDomain = untrack(() => copilotEnterpriseDomain(baseUrl));
  let host = $state<CopilotHost>(initialDomain === null ? "github" : "enterprise");
  let domain = $state(initialDomain ?? "");
  const login = new CopilotLogin();
  const copier = copyFeedback();
  onDestroy(() => login.cancel());

  const flow = $derived(login.state);
  const signedIn = $derived(Boolean(baseUrl) && (saved.hasKey || flow.phase === "done"));

  /** A device code belongs to one GitHub host, so any change stops the sign-in under way. */
  function retarget(nextHost: CopilotHost, nextDomain: string) {
    host = nextHost;
    domain = nextDomain;
    login.cancel();
    onBaseUrl(copilotBaseUrl(nextHost === "github" ? null : nextDomain));
  }
</script>

<div class="provider-field-group copilot-sign-in">
  <div class="provider-field-label-row">
    <span class="provider-field-label">GitHub account</span>
    {#if signedIn && flow.phase !== "waiting"}<span class="provider-saved-hint">Signed in</span>{/if}
  </div>
  <SegmentedChoice label="GitHub host" options={COPILOT_HOST_OPTIONS} value={host} onSelect={(next) => retarget(next, domain)} />
  {#if host === "enterprise"}
    <input
      value={domain}
      oninput={(event) => retarget("enterprise", event.currentTarget.value)}
      placeholder="github.example.com"
      aria-label="GitHub Enterprise domain"
      spellcheck={false}
      autocomplete="off"
    />
  {/if}

  {#if flow.phase === "waiting"}
    {@const code = flow}
    <div class="copilot-code">
      <span class="provider-field-hint">Enter this code on GitHub and approve the sign-in.</span>
      <div class="copilot-code-row">
        <code class="copilot-user-code">{code.userCode}</code>
        <Button
          variant="icon-mini"
          title={copier.copied ? "Copied" : "Copy code"}
          aria-label="Copy code"
          onclick={() => void copier.copy(code.userCode)}
        >
          <Icon icon={copier.copied ? Check : Copy} size={13} />
        </Button>
        <Button variant="secondary" size="s" onclick={() => void openReleaseUrl(code.verificationUri)}>
          <span>Open GitHub</span>
          <Icon icon={ExternalLink} size={11} />
        </Button>
      </div>
      <div class="copilot-waiting">
        <Icon icon={LoaderCircle} size={13} class="spin" />
        <span>Waiting for you to approve…</span>
        <Button size="s" onclick={() => login.cancel()}>Cancel</Button>
      </div>
    </div>
  {:else}
    <div class="copilot-sign-in-row">
      <Button
        variant={signedIn ? "secondary" : "primary"}
        size="s"
        disabled={!baseUrl || flow.phase === "starting"}
        onclick={() => void login.start(baseUrl)}
      >
        {#if flow.phase === "starting"}
          <Icon icon={LoaderCircle} size={13} class="spin" />
          <span>Asking GitHub…</span>
        {:else}
          <span>{signedIn ? "Sign in again" : "Sign in with GitHub"}</span>
        {/if}
      </Button>
    </div>
  {/if}

  {#if flow.phase === "failed"}
    <p class="setting-error" role="alert">{flow.reason}</p>
  {/if}
  <small class="provider-field-hint">
    Uses the models your Copilot plan includes. The GitHub token is saved in auth.json for this host only.
  </small>
</div>
