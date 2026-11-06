<script lang="ts">
  /**
   * The app logo: the pearl pet glowing on a midnight squircle. Draws the
   * same art as the desktop icon (`MASTER_SVG` in scripts/generate_icons.py).
   */
  type Props = { size?: number; class?: string };
  let { size = 18, class: className }: Props = $props();

  const uid = $props.id();
  const BODY = "M16 8.6C22.4 8.6 26.6 13.2 26.6 19C26.6 24.2 22.2 27.2 16 27.2C9.8 27.2 5.4 24.2 5.4 19C5.4 13.2 9.6 8.6 16 8.6Z";
  const PET = "translate(16 -28) scale(15)";
  const id = (name: string) => `${uid}-lm-${name}`;
  const url = (name: string) => `url(#${id(name)})`;
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 512 512"
  fill="none"
  aria-hidden="true"
>
  <defs>
    <linearGradient id={id("bg")} x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#202024" />
      <stop offset="50%" stop-color="#151518" />
      <stop offset="100%" stop-color="#0C0C0E" />
    </linearGradient>
    <radialGradient id={id("sky")} cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#4F86FF" stop-opacity="0.5" />
      <stop offset="45%" stop-color="#4F86FF" stop-opacity="0.18" />
      <stop offset="100%" stop-color="#4F86FF" stop-opacity="0" />
    </radialGradient>
    <radialGradient id={id("lilac")} cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#7F5CFF" stop-opacity="0.5" />
      <stop offset="45%" stop-color="#7F5CFF" stop-opacity="0.18" />
      <stop offset="100%" stop-color="#7F5CFF" stop-opacity="0" />
    </radialGradient>
    <radialGradient id={id("pool")} cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#9A84FF" stop-opacity="0.32" />
      <stop offset="60%" stop-color="#9A84FF" stop-opacity="0.08" />
      <stop offset="100%" stop-color="#9A84FF" stop-opacity="0" />
    </radialGradient>
    <radialGradient id={id("contact")} cx="50%" cy="50%" r="50%">
      <stop offset="0%" stop-color="#000000" stop-opacity="0.6" />
      <stop offset="60%" stop-color="#000000" stop-opacity="0.2" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0" />
    </radialGradient>
    <linearGradient id={id("gloss")} x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.08" />
      <stop offset="38%" stop-color="#FFFFFF" stop-opacity="0" />
      <stop offset="70%" stop-color="#000000" stop-opacity="0" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0.4" />
    </linearGradient>
    <linearGradient id={id("bevel")} x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.32" />
      <stop offset="14%" stop-color="#FFFFFF" stop-opacity="0.08" />
      <stop offset="60%" stop-color="#FFFFFF" stop-opacity="0" />
      <stop offset="100%" stop-color="#000000" stop-opacity="0.5" />
    </linearGradient>
    <clipPath id={id("clip")}><rect width="512" height="512" rx="116" /></clipPath>
    <radialGradient id={id("body")} cx="36%" cy="28%" r="82%">
      <stop offset="0%" stop-color="#FFFFFF" />
      <stop offset="42%" stop-color="#F2F4F9" />
      <stop offset="74%" stop-color="#D9DDE7" />
      <stop offset="100%" stop-color="#B3B8C8" />
    </radialGradient>
    <linearGradient id={id("sheen")} x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#bcd9ff" stop-opacity="0.5" />
      <stop offset="45%" stop-color="#bcd9ff" stop-opacity="0" />
      <stop offset="60%" stop-color="#d9c8ff" stop-opacity="0" />
      <stop offset="100%" stop-color="#d9c8ff" stop-opacity="0.65" />
    </linearGradient>
    <radialGradient id={id("bloom")} cx="50%" cy="64%" r="46%">
      <stop offset="0%" stop-color="#FFFFFF" stop-opacity="0.35" />
      <stop offset="100%" stop-color="#FFFFFF" stop-opacity="0" />
    </radialGradient>
    <linearGradient id={id("foot")} x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#E1E4EC" />
      <stop offset="100%" stop-color="#B2B7C7" />
    </linearGradient>
    <filter id={id("aura")} x="-60%" y="-60%" width="220%" height="220%">
      <feGaussianBlur stdDeviation="3" />
    </filter>
    <filter id={id("shadow")} x="-30%" y="-30%" width="160%" height="170%">
      <feDropShadow dx="0" dy="12" stdDeviation="14" flood-color="#000000" flood-opacity="0.55" />
      <feDropShadow dx="0" dy="3" stdDeviation="4" flood-color="#000000" flood-opacity="0.35" />
    </filter>
  </defs>

  <rect width="512" height="512" rx="116" fill={url("bg")} />
  <g clip-path={url("clip")}>
    <circle cx="200" cy="200" r="220" fill={url("sky")} />
    <circle cx="318" cy="318" r="220" fill={url("lilac")} />
    <ellipse cx="256" cy="404" rx="180" ry="36" fill={url("pool")} />
  </g>
  <ellipse cx="256" cy="402" rx="120" ry="16" fill={url("contact")} />

  <g transform={PET}>
    <path d={BODY} fill="#9DB8FF" opacity="0.55" filter={url("aura")} />
  </g>

  <g filter={url("shadow")}>
    <g transform={PET}>
      <ellipse cx="11.8" cy="27.2" rx="2.6" ry="1.5" fill={url("foot")} />
      <ellipse cx="20.2" cy="27.2" rx="2.6" ry="1.5" fill={url("foot")} />
      <path d={BODY} fill={url("body")} />
      <path d={BODY} fill={url("bloom")} />
      <path d={BODY} fill={url("sheen")} />
      <path d={BODY} fill="none" stroke="#FFFFFF" stroke-opacity="0.6" stroke-width="0.4" />
      <ellipse cx="11.8" cy="12.9" rx="3.4" ry="1.9" transform="rotate(-28 11.8 12.9)" fill="#FFFFFF" fill-opacity="0.92" />
      <ellipse cx="9.3" cy="21.7" rx="1.9" ry="1.1" fill="#ff8fa3" fill-opacity="0.45" />
      <ellipse cx="22.7" cy="21.7" rx="1.9" ry="1.1" fill="#ff8fa3" fill-opacity="0.45" />
      <ellipse cx="12.4" cy="18.4" rx="1.35" ry="2" fill="#14151a" />
      <ellipse cx="19.6" cy="18.4" rx="1.35" ry="2" fill="#14151a" />
      <path d="M14.6 22.5q1.4 1.3 2.8 0" stroke="#14151a" stroke-width="0.9" stroke-linecap="round" />
    </g>
  </g>

  <rect width="512" height="512" rx="116" fill={url("gloss")} />
  <rect x="1.5" y="1.5" width="509" height="509" rx="114.5" fill="none" stroke={url("bevel")} stroke-width="3" />
  <rect x="0.5" y="0.5" width="511" height="511" rx="115.5" fill="none" stroke="#FFFFFF" stroke-opacity="0.12" />
</svg>
