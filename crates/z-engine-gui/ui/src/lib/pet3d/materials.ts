import { DataTexture, LinearFilter, MeshBasicMaterial, MeshPhysicalMaterial, type Color } from "three";

/**
 * The few surfaces the pet is made of: its pearl skin, glossy plastic and
 * ink (eyes, headphones, props), soft satin (paper, wood, cloth), glass,
 * and unlit highlights. All share the scene's reflection map.
 */

/** The pet's skin: a soft pearl with a clear coat, and a sheen that brightens its rim. */
export function pearl(color: Color): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({
    color,
    roughness: 0.46,
    clearcoat: 0.85,
    clearcoatRoughness: 0.22,
    sheen: 0.5,
    sheenRoughness: 0.45,
    sheenColor: 0xffffff,
  });
}

/** Hard glossy plastic or wet ink: the eyes, the headphones, the laptop. */
export function glossy(color: Color, roughness = 0.2): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({ color, roughness, clearcoat: 1, clearcoatRoughness: 0.06 });
}

/** A soft, barely shiny surface: paper, wood, straw, cloth, leaves. */
export function satin(color: Color, roughness = 0.62): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({ color, roughness, sheen: 0.25, sheenRoughness: 0.6, sheenColor: 0xffffff });
}

/** Tinted glass that shows the pet through it. */
export function glass(color: Color, alpha: number): MeshPhysicalMaterial {
  return new MeshPhysicalMaterial({
    color,
    roughness: 0.08,
    transparent: true,
    opacity: Math.max(alpha, 0.18),
    depthWrite: false,
    clearcoat: 1,
  });
}

/** A light that ignores the lights: glints, a glowing logo. */
export function unlit(color: Color, opacity = 1): MeshBasicMaterial {
  return new MeshBasicMaterial({ color, transparent: opacity < 1, opacity });
}

/**
 * A square alpha ramp, `size` pixels a side: white, opaque at the middle
 * and gone by the edge (`falloff(d)` for d, the distance from the middle,
 * 0..1). Tinted by the material, it is a glow or a soft shadow; built from
 * numbers, so it needs no canvas.
 */
export function radialRamp(falloff: (d: number) => number, size = 64): DataTexture {
  const data = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const d = Math.hypot((x + 0.5) / size - 0.5, (y + 0.5) / size - 0.5) * 2;
      const i = (y * size + x) * 4;
      data.set([255, 255, 255, Math.round(255 * Math.min(1, Math.max(0, falloff(Math.min(1, d)))))], i);
    }
  }
  return dataTexture(data, size, size);
}

/** RGBA bytes as a smoothly filtered texture (a DataTexture samples nearest by default). */
export function dataTexture(data: Uint8Array, width: number, height: number): DataTexture {
  const texture = new DataTexture(data, width, height);
  texture.magFilter = LinearFilter;
  texture.minFilter = LinearFilter;
  texture.needsUpdate = true;
  return texture;
}

/** Smooth 1 → 0 between `inner` and 1 (the edge). */
export function softEdge(inner: number): (d: number) => number {
  return (d) => {
    const u = Math.min(1, Math.max(0, (d - inner) / (1 - inner)));
    return 1 - u * u * (3 - 2 * u);
  };
}
