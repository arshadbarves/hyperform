import { Color, SRGBColorSpace } from "three";
import type { LiveTone } from "../domain/liveStatus";
import type { PetLook } from "../domain/pet/looks";

/** The pet's own colors, named after their `--pet-*` tokens. */
export type PetColorName =
  | "light"
  | "shade"
  | "rim"
  | "eye"
  | "blush"
  | "leaf"
  | "leafLight"
  | "stem"
  | "scarf"
  | "scarfShade"
  | "gold"
  | "heart"
  | "sweat";

/** What the pet holds is painted in these (`--prop-*` in pet-props.css). */
export type PropColorName = "wood" | "metal" | "glass" | "paper" | "board" | "straw" | "screen";

type Token = readonly [name: `--${string}`, fallback: string];

/** Each color's token, and its value in `tokens.css` for where no stylesheet is loaded. */
const PET: Record<PetColorName, Token> = {
  light: ["--pet-light", "#ffffff"],
  shade: ["--pet-shade", "#7b8193"],
  rim: ["--pet-rim", "rgba(255, 255, 255, 0.42)"],
  eye: ["--pet-eye", "#14151a"],
  blush: ["--pet-blush", "#ff8fa3"],
  leaf: ["--pet-leaf", "#7ad694"],
  leafLight: ["--pet-leaf-light", "#9ae3ad"],
  stem: ["--pet-stem", "#5fbf7a"],
  scarf: ["--pet-scarf", "#ff6b6b"],
  scarfShade: ["--pet-scarf-shade", "#e85a5a"],
  gold: ["--pet-gold", "#ffd76a"],
  // Set on `.pet` in pet-face.css rather than on the root, so these read their fallback.
  heart: ["--pet-heart", "#ff5f7a"],
  sweat: ["--pet-sweat", "#9fd8ff"],
};

// Also set on `.pet`, in pet-props.css.
const PROPS: Record<PropColorName, Token> = {
  wood: ["--prop-wood", "#c98b5a"],
  metal: ["--prop-metal", "#c9ced8"],
  glass: ["--prop-glass", "rgba(188, 217, 255, 0.28)"],
  paper: ["--prop-paper", "#f7f4ec"],
  board: ["--prop-board", "#b07a4f"],
  straw: ["--prop-straw", "#f2c46b"],
  screen: ["--prop-screen", "#3a3f4d"],
};

const LOOKS: Record<PetLook, Token> = {
  pearl: ["--pet-pearl", "#eceef4"],
  mint: ["--pet-mint", "#b8f0d8"],
  sky: ["--pet-sky", "#bcd9ff"],
  lilac: ["--pet-lilac", "#d9c8ff"],
  peach: ["--pet-peach", "#ffd2b8"],
  graphite: ["--pet-graphite", "#8e929e"],
};

/** The aura's tint per status tone, as in `pet-body.css`; a quiet pet's aura is its body color. */
const TONES: Record<Exclude<LiveTone, "quiet">, Token> = {
  working: ["--blue", "#0a84ff"],
  attention: ["--amber", "#ff9f0a"],
  ok: ["--green", "#30d158"],
  danger: ["--red", "#ff453a"],
};

interface Swatch {
  color: Color;
  alpha: number;
}

const cache = new Map<string, Swatch>();

function srgb(r: number, g: number, b: number, alpha: number): Swatch {
  return { color: new Color().setRGB(r / 255, g / 255, b / 255, SRGBColorSpace), alpha };
}

function alphaOf(value: string | undefined): number {
  if (value === undefined) return 1;
  const amount = value.endsWith("%") ? Number(value.slice(0, -1)) / 100 : Number(value);
  return Math.min(1, Math.max(0, amount));
}

/** A CSS color as tokens are written: `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()` or `rgba()`. Null for anything else. */
function parse(value: string): Swatch | null {
  const text = value.trim().toLowerCase();
  const hex = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/.exec(text);
  if (hex) {
    const digits = hex[1].length === 3 ? [...hex[1]].map((d) => d + d).join("") : hex[1];
    const byte = (i: number) => parseInt(digits.slice(i, i + 2), 16);
    return srgb(byte(0), byte(2), byte(4), digits.length === 8 ? byte(6) / 255 : 1);
  }
  const rgb = /^rgba?\(\s*([\d.]+)[\s,]+([\d.]+)[\s,]+([\d.]+)\s*(?:[,/]\s*([\d.]+%?)\s*)?\)$/.exec(text);
  if (rgb) return srgb(Number(rgb[1]), Number(rgb[2]), Number(rgb[3]), alphaOf(rgb[4]));
  return null;
}

function tokenValue(name: string): string {
  if (typeof document === "undefined" || typeof getComputedStyle !== "function") return "";
  return getComputedStyle(document.documentElement).getPropertyValue(name);
}

function read([name, fallback]: Token): Swatch {
  let swatch = cache.get(name);
  if (!swatch) {
    swatch = parse(tokenValue(name)) ?? parse(fallback) ?? srgb(255, 255, 255, 1);
    cache.set(name, swatch);
  }
  return swatch;
}

/** One of the pet's colors, as a new `Color` (linear, ready for a material). */
export function petColor(name: PetColorName): Color {
  return read(PET[name]).color.clone();
}

/** The token's own opacity, 0..1: the rim is translucent, the rest are opaque. */
export function petAlpha(name: PetColorName): number {
  return read(PET[name]).alpha;
}

/** A prop's color, as a new `Color`. */
export function propColor(name: PropColorName): Color {
  return read(PROPS[name]).color.clone();
}

/** A prop color's own opacity (the glass is see-through). */
export function propAlpha(name: PropColorName): number {
  return read(PROPS[name]).alpha;
}

/** The body color for a look. */
export function bodyColor(look: PetLook): Color {
  return read(LOOKS[look]).color.clone();
}

/** The aura's color for a status tone; quiet takes the body color of `look`. */
export function toneColor(tone: LiveTone, look: PetLook): Color {
  return tone === "quiet" ? bodyColor(look) : read(TONES[tone]).color.clone();
}

/** `color-mix(in srgb, a share, b)`: mixed as the stylesheets mix, in sRGB rather than linear. */
export function mixColors(a: Color, b: Color, share: number): Color {
  const from = a.getRGB({ r: 0, g: 0, b: 0 }, SRGBColorSpace);
  const to = b.getRGB({ r: 0, g: 0, b: 0 }, SRGBColorSpace);
  const mix = (x: number, y: number) => x * share + y * (1 - share);
  return new Color().setRGB(mix(from.r, to.r), mix(from.g, to.g), mix(from.b, to.b), SRGBColorSpace);
}

/** Forgets the colors read so far, so the next ask reads the tokens again. */
export function refreshPalette() {
  cache.clear();
}
