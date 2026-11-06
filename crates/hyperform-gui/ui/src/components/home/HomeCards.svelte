<script lang="ts">
  import { setupChecklist } from "$lib/domain/onboarding";
  import { sameWorkspacePath } from "$lib/domain/paths";
  import { startersFor } from "$lib/domain/starters";
  import { continuePlan } from "$lib/domain/work/homeCard";
  import { projectPlans, projects, sessionList } from "$lib/runtime";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { ticker } from "$lib/ui/ticker.svelte";
  import { detectProviderId, presetById, requiresApiKey } from "$lib/providers";
  import ChangesCard from "./ChangesCard.svelte";
  import ContinueCard from "./ContinueCard.svelte";
  import SetupChecklist from "./SetupChecklist.svelte";
  import StarterChips from "./StarterChips.svelte";

  /** Below the hero composer: starting points, then where the project stands. Only what applies is shown. */
  type Props = { root: string };
  let { root }: Props = $props();

  const clock = ticker(() => true, 30_000);
  const summary = $derived(projects.summary(root));
  const details = $derived(projects.details(root));
  const chats = $derived(sessionList.items.filter((s) => sameWorkspacePath(s.projectRoot, root)).slice(0, 3));
  const plan = $derived(continuePlan(projectPlans.of(root)));
  const starters = $derived(startersFor({ changed: summary?.changed ?? 0, hasInstructions: details?.instructions ?? null }));
  const provider = $derived(settingsStore.activeProvider);
  const preset = $derived(presetById(detectProviderId(provider?.baseUrl)));
  const trust = $derived(sameWorkspacePath(settingsStore.root, root) ? settingsStore.trust : null);
  const checklist = $derived(
    setupChecklist({
      modelReady: !preset || !provider || provider.hasKey || !requiresApiKey(preset),
      trusted: trust ? trust.trusted : null,
      hasInstructions: details?.instructions ?? null,
    }),
  );

  $effect(() => {
    void summary?.changed;
    void projects.loadDetails(root);
  });

  $effect(() => {
    if (sameWorkspacePath(settingsStore.root, root)) void settingsStore.loadTrust();
  });

  $effect(() => {
    if (projectPlans.enabled) void projectPlans.refresh(root);
  });
</script>

<div class="home-below">
  <StarterChips top={starters.top} more={starters.more} />

  <div class="home-grid">
    {#if chats.length || plan}<ContinueCard {chats} {plan} now={clock.now} />{/if}
    {#if details?.files.length}<ChangesCard files={details.files} />{/if}
    {#if !checklist.complete}<SetupChecklist list={checklist} {root} />{/if}
  </div>
</div>
