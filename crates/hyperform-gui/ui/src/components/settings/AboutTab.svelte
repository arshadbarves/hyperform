<script lang="ts">
  import { openReleaseUrl } from "$lib/commands";
  import { joinPath } from "$lib/domain/settings/instructions";
  import { settingsStore } from "$lib/stores/settings.svelte";
  import { bindStore } from "$lib/svelte/bind.svelte";
  import { updateStore } from "$lib/updateStore";
  import { Button, Pill } from "$lib/ui";
  import Icon, {
    ArrowRight,
    Check,
    CheckCircle2,
    Copy,
    Download,
    ExternalLink,
    FileText,
    Folder,
    KeyRound,
    LoaderCircle,
    RefreshCw,
    Shield,
    Sparkles,
  } from "$lib/ui/icons";
  import LogoMark from "./LogoMark.svelte";
  import SettingsCard from "./SettingsCard.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";

  const CHANGELOG_URL = "https://github.com/arshadbarves/hyperform/blob/release/CHANGELOG.md";

  const update = bindStore(updateStore);
  const info = $derived(update.current.info);
  const checking = $derived(update.current.checking);
  const installing = $derived(update.current.installing);
  const progress = $derived(update.current.progress);
  const pct = $derived(progress?.percentage != null ? Math.round(progress.percentage) : null);
  const isDownloading = $derived(installing && progress?.phase === "downloading");
  const isInstalling = $derived(installing && (progress?.phase === "installing" || progress?.phase === "ready"));
  const app = $derived(settingsStore.info);
  const displayVersion = $derived(app?.version || info?.current || "");

  let copiedPath = $state<string | null>(null);

  async function copyToClipboard(text: string, id: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedPath = id;
      setTimeout(() => {
        if (copiedPath === id) copiedPath = null;
      }, 1600);
    } catch {
      // The clipboard can be unavailable in the webview; the path stays visible to copy by hand.
    }
  }

  const paths = $derived.by(() => {
    const layerPath = (scope: string) => settingsStore.loaded?.layers.find((l) => l.scope === scope)?.path ?? null;
    const configDir = app?.configDir;
    const inConfig = (name: string) => (configDir ? joinPath(configDir, name) : `~/.config/hyperform/${name}`);
    return [
      { id: "user", label: "User settings", path: layerPath("user") ?? inConfig("settings.toml"), desc: "Your defaults for every project", icon: FileText },
      { id: "auth", label: "API keys", path: inConfig("auth.json"), desc: "Stored API keys, kept out of settings files", icon: KeyRound },
      { id: "trust", label: "Trusted workspaces", path: inConfig("trust.json"), desc: "Projects whose own settings apply in full", icon: Shield },
      { id: "project", label: "Project settings", path: layerPath("project") ?? ".hyperform/settings.toml", desc: "Shared with the team", icon: FileText },
      { id: "local", label: "Personal project settings", path: layerPath("projectLocal") ?? ".hyperform/settings.local.toml", desc: "Git-ignored overrides", icon: FileText },
      { id: "data", label: "Sessions & checkpoints", path: app ? joinPath(app.dataDir, "sessions") : "sessions", desc: "Chat history and shadow checkpoints", icon: Folder },
      { id: "pet", label: "Your pet", path: app ? joinPath(app.dataDir, "pet.json") : "pet.json", desc: "Its level, streak and what it wears", icon: Sparkles },
    ];
  });
</script>

<div class="tab-body about-tab">
  <div class="about-hero">
    <div class="about-hero-logo"><LogoMark size={52} /></div>
    <div class="about-hero-text">
      <div class="about-hero-title-row">
        <h3>HyperForm AI</h3>
        {#if displayVersion}<Pill>v{displayVersion}</Pill>{/if}
      </div>
      <p class="about-hero-sub">Think Beyond. Build Beyond.</p>
    </div>
  </div>

  <SettingsGroup title="Software updates" description="HyperForm AI checks for new releases on launch.">
    {#if info?.available}
      <div class="settings-card about-update-card has-update" role="status">
        <div class="about-update-row">
          <div class="about-update-icon-wrap"><Icon icon={Sparkles} size={16} /></div>
          <div class="about-update-detail">
            <div class="about-update-headline">
              <span class="about-update-label">Update available</span>
              <Pill tone="info">v{info.current} <Icon icon={ArrowRight} size={9} /> v{info.latest}</Pill>
            </div>
            {#if info.releaseNotes}<p class="about-update-summary">{info.releaseNotes}</p>{/if}
          </div>
        </div>

        {#if installing && pct != null}
          <div class="about-update-progress">
            <div class="about-progress-track" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100} role="progressbar">
              <div class="about-progress-fill" style:scale={`${pct / 100} 1`}></div>
            </div>
            <span class="about-progress-pct">{pct}%</span>
          </div>
        {/if}

        <div class="about-update-actions">
          <Button variant="accent" disabled={installing} onclick={() => void updateStore.install()}>
            {#if isInstalling}
              <Icon icon={LoaderCircle} size={13} class="spin" /><span>Installing…</span>
            {:else if isDownloading}
              <Icon icon={LoaderCircle} size={13} class="spin" /><span>Downloading…</span>
            {:else}
              <Icon icon={Download} size={13} /><span>Update & Restart</span>
            {/if}
          </Button>
          <Button
            variant="icon"
            title="Recheck latest release"
            aria-label="Recheck latest release"
            spinning={checking}
            disabled={checking || installing}
            onclick={() => void updateStore.check(true)}
          >
            <Icon icon={RefreshCw} size={13} />
          </Button>
          {#if info.url}
            <Button variant="icon" title="View release on GitHub" aria-label="View release on GitHub" onclick={() => updateStore.openRelease()}>
              <Icon icon={ExternalLink} size={13} />
            </Button>
          {/if}
        </div>
      </div>
    {:else}
      <div class="settings-card about-update-card" role="status">
        <div class="about-update-row">
          <div class="about-update-icon-wrap is-ok"><Icon icon={CheckCircle2} size={16} /></div>
          <div class="about-update-detail">
            <span class="about-update-label">Up to date</span>
            <span class="about-uptodate-sub">{displayVersion ? `Version ${displayVersion} is the latest release` : "No newer release found"}</span>
          </div>
          <Button variant="secondary" disabled={checking} onclick={() => void updateStore.check(true)}>
            <Icon icon={RefreshCw} size={12} class={checking ? "spin" : undefined} />
            <span>{checking ? "Checking…" : "Check for updates"}</span>
          </Button>
        </div>
      </div>
    {/if}
  </SettingsGroup>

  <SettingsGroup title="Files & storage" description="Where HyperForm keeps settings, keys, history and your pet.">
    <SettingsCard>
      {#each paths as p (p.id)}
        <div class="path-item-row">
          <div class="path-item-icon"><Icon icon={p.icon} size={14} /></div>
          <div class="path-item-copy">
            <div class="path-item-title-row">
              <span class="path-item-name">{p.label}</span>
              <span class="path-item-desc">{p.desc}</span>
            </div>
            <code class="path-item-code">{p.path}</code>
          </div>
          <Button size="s" variant="secondary" title={`Copy ${p.path}`} onclick={() => void copyToClipboard(p.path, p.id)}>
            {#if copiedPath === p.id}
              <Icon icon={Check} size={12} /><span>Copied</span>
            {:else}
              <Icon icon={Copy} size={12} /><span>Copy</span>
            {/if}
          </Button>
        </div>
      {/each}
    </SettingsCard>
  </SettingsGroup>

  <div class="about-links-row">
    <Button variant="secondary" onclick={() => void openReleaseUrl(CHANGELOG_URL)}>
      <Icon icon={FileText} size={13} />
      <span>View release notes</span>
      <Icon icon={ExternalLink} size={10} class="about-link-ext" />
    </Button>
    {#if info?.url}
      <Button variant="secondary" onclick={() => updateStore.openRelease()}>
        <Icon icon={Sparkles} size={13} />
        <span>GitHub release</span>
        <Icon icon={ExternalLink} size={10} class="about-link-ext" />
      </Button>
    {/if}
  </div>
</div>
