<script lang="ts">
  /** A ring filled clockwise from the top; `value` runs from 0 to 1. */
  type Props = {
    value: number;
    size?: number;
    stroke?: number;
    tone?: "quiet" | "ok" | "warn" | "danger" | "working";
    label?: string;
    class?: string;
  };

  let { value, size = 18, stroke = 2, tone = "quiet", label, class: className = "" }: Props = $props();

  const center = $derived(size / 2);
  const radius = $derived((size - stroke) / 2);
  const length = $derived(2 * Math.PI * radius);
  const filled = $derived(Math.min(1, Math.max(0, Number.isFinite(value) ? value : 0)));
</script>

<svg
  class={`ring tone-${tone} ${className}`}
  width={size}
  height={size}
  viewBox={`0 0 ${size} ${size}`}
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
>
  <circle class="ring-track" cx={center} cy={center} r={radius} stroke-width={stroke} />
  <circle
    class="ring-arc"
    cx={center}
    cy={center}
    r={radius}
    stroke-width={stroke}
    stroke-dasharray={length}
    stroke-dashoffset={length * (1 - filled)}
    transform={`rotate(-90 ${center} ${center})`}
  />
</svg>
