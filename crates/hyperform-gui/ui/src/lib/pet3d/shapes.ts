import {
  CatmullRomCurve3,
  CubicBezierCurve3,
  ExtrudeGeometry,
  LineCurve3,
  Mesh,
  Shape,
  SphereGeometry,
  TubeGeometry,
  Vector3,
  type Curve,
  type Material,
  type Object3D,
  type Texture,
} from "three";

/**
 * Geometry helpers in the pet's own space. The scene is built in the SVG
 * pet's 32-unit box so every prop lands where the flat pet draws it: a
 * point (x, y) of that box (y down) is at (x - 16, 28 - y) in the scene
 * (y up), with the feet's pivot (16, 28) at the origin and z toward the
 * viewer.
 */

/** Where the rig pivots in the 32-unit box: the feet (rig3d's pivot). */
export const PIVOT = { x: 16, y: 28 } as const;

/** A point of the 32-unit box at depth `z`, in the scene. */
export function toScene(x: number, y: number, z = 0): Vector3 {
  return new Vector3(x - PIVOT.x, PIVOT.y - y, z);
}

export type Point3 = readonly [x: number, y: number, z: number];

/** A tube through box points (x, y, z): a straight rod for two points, a smooth curve for more. */
export function tube(points: readonly Point3[], radius: number, material: Material, segments = 16): Mesh {
  const at = points.map(([x, y, z]) => toScene(x, y, z));
  const path: Curve<Vector3> = at.length === 2 ? new LineCurve3(at[0], at[1]) : new CatmullRomCurve3(at);
  return new Mesh(new TubeGeometry(path, segments, radius, 8, false), material);
}

/** A tube along one cubic bézier of the box (the SVG `C` command), at depth `z`. */
export function curveTube(p0: Point3, c1: Point3, c2: Point3, p1: Point3, radius: number, material: Material): Mesh {
  const curve = new CubicBezierCurve3(toScene(...p0), toScene(...c1), toScene(...c2), toScene(...p1));
  return new Mesh(new TubeGeometry(curve, 24, radius, 8, false), material);
}

/** An outline step in box coordinates: move, line, quadratic or cubic bézier (absolute, like SVG `M L Q C`). */
export type Step =
  | readonly ["M", number, number]
  | readonly ["L", number, number]
  | readonly ["Q", number, number, number, number]
  | readonly ["C", number, number, number, number, number, number];

/** A flat outline traced from box coordinates, in scene units. */
export function outline(steps: readonly Step[]): Shape {
  const shape = new Shape();
  const p = (x: number, y: number) => [x - PIVOT.x, PIVOT.y - y] as const;
  for (const step of steps) {
    if (step[0] === "M") shape.moveTo(...p(step[1], step[2]));
    else if (step[0] === "L") shape.lineTo(...p(step[1], step[2]));
    else if (step[0] === "Q") shape.quadraticCurveTo(...p(step[1], step[2]), ...p(step[3], step[4]));
    else shape.bezierCurveTo(...p(step[1], step[2]), ...p(step[3], step[4]), ...p(step[5], step[6]));
  }
  return shape;
}

/** A rounded rectangle of the box (the SVG `rect` with `rx`), as an outline. */
export function roundedRect(x: number, y: number, width: number, height: number, r: number): Shape {
  const k = Math.min(r, width / 2, height / 2);
  const [r0, b0] = [x + width, y + height];
  return outline([
    ["M", x + k, y],
    ["L", r0 - k, y],
    ["Q", r0, y, r0, y + k],
    ["L", r0, b0 - k],
    ["Q", r0, b0, r0 - k, b0],
    ["L", x + k, b0],
    ["Q", x, b0, x, b0 - k],
    ["L", x, y + k],
    ["Q", x, y, x + k, y],
  ]);
}

/** An outline given thickness `depth` with a soft bevel, centred on z = 0 so the mesh can sit at any depth. */
export function slab(shape: Shape, depth: number, material: Material, bevel = 0.18): Mesh {
  const geometry = new ExtrudeGeometry(shape, {
    depth,
    bevelEnabled: bevel > 0,
    bevelThickness: bevel,
    bevelSize: bevel,
    bevelSegments: 3,
    curveSegments: 10,
  });
  geometry.translate(0, 0, -depth / 2);
  return new Mesh(geometry, material);
}

/** An ellipsoid with radii (rx, ry, rz), centred on box point (x, y) at depth `z`. */
export function ellipsoid(x: number, y: number, z: number, radii: Point3, material: Material): Mesh {
  const mesh = new Mesh(new SphereGeometry(1, 24, 16), material);
  mesh.position.copy(toScene(x, y, z));
  mesh.scale.set(...radii);
  return mesh;
}

const TEXTURE_SLOTS = ["map", "emissiveMap", "alphaMap"] as const;

/** Frees everything under `root` on the GPU: geometry, materials and their textures. */
export function disposeTree(root: Object3D) {
  root.traverse((node) => {
    const drawn = node as Object3D & { geometry?: { dispose(): void }; material?: Material | Material[] };
    drawn.geometry?.dispose();
    const materials = drawn.material ? (Array.isArray(drawn.material) ? drawn.material : [drawn.material]) : [];
    for (const material of materials) {
      for (const slot of TEXTURE_SLOTS) (material as Material & Partial<Record<(typeof TEXTURE_SLOTS)[number], Texture | null>>)[slot]?.dispose();
      material.dispose();
    }
  });
}
