<script lang="ts">
  import { CATEGORY_ORDER } from "$lib/domain/inspectOutline";
  import { categoryMeta, type ContextCategory } from "$lib/promptInspectView";
  import { fmtTokens } from "$lib/util";

  /**
   * What the request is made of, as a ring of the context window: one arc
   * per kind of part, the rest of the ring is room left. A kind in the legend
   * filters the outline; picking it again clears the filter.
   */
  type Props = {
    totals: Record<ContextCategory, number>;
    used: number;
    limit: number;
    active: ContextCategory | null;
    onPick: (category: ContextCategory | null) => void;
  };
  let { totals, used, limit, active, onPick }: Props = $props();

  const R = 50;
  const C = 2 * Math.PI * R;
  const GAP = 2.2;

  const pct = $derived(limit > 0 ? Math.min(100, Math.round((used / limit) * 100)) : 0);
  const level = $derived(pct >= 85 ? "danger" : pct >= 65 ? "warn" : "ok");
  const parts = $derived(CATEGORY_ORDER.filter((c) => totals[c] > 0));
  const sum = $derived(parts.reduce((n, c) => n + totals[c], 0));
  const arcs = $derived.by(() => {
    const filled = C * Math.min(1, limit > 0 ? used / limit : 0);
    let start = 0;
    return parts.map((c) => {
      const length = sum > 0 ? (totals[c] / sum) * filled : 0;
      const arc = { c, start, length: Math.max(0.6, length - (parts.length > 1 ? GAP : 0)) };
      start += length;
      return arc;
    });
  });
</script>

<section class="inspector-map" aria-label="What fills the context window">
  <div class="inspector-ring">
    <svg class="inspector-ring-svg" viewBox="0 0 120 120" aria-hidden="true">
      <circle class="inspector-ring-track" cx="60" cy="60" r={R} />
      {#each arcs as arc (arc.c)}
        <circle
          class="inspector-ring-arc"
          class:is-dim={active !== null && active !== arc.c}
          cx="60"
          cy="60"
          r={R}
          stroke={categoryMeta(arc.c).color}
          stroke-dasharray={`${arc.length} ${C}`}
          stroke-dashoffset={-arc.start}
        />
      {/each}
    </svg>
    <div class="inspector-ring-center">
      <span class={`inspector-map-pct level-${level}`}>{pct}%</span>
      <span class="inspector-ring-sub">of the window</span>
    </div>
  </div>
  <p class="inspector-map-line">{fmtTokens(used)} of {fmtTokens(limit)} tokens</p>
  <div class="inspector-map-keys" role="group" aria-label="Show one kind of part">
    {#each parts as c (c)}
      <button type="button" class="inspector-map-key" aria-pressed={active === c} onclick={() => onPick(active === c ? null : c)}>
        <span class="inspector-swatch" style:background={categoryMeta(c).color}></span>
        {categoryMeta(c).label}
        <span class="inspector-map-tokens">{fmtTokens(totals[c])}</span>
      </button>
    {/each}
  </div>
</section>
