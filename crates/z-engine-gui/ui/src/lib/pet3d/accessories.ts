import { CapsuleGeometry, CatmullRomCurve3, Group, Mesh, TubeGeometry, Vector3, type Object3D } from "three";
import type { PetAccessory } from "../domain/pet/looks";
import { BODY_DEPTH, radiusAt } from "./body";
import { glossy, satin } from "./materials";
import { petColor } from "./palette";
import { curveTube, outline, slab, toScene, tube, type Point3 } from "./shapes";

/**
 * What the pet wears, as small 3D models on its body where PetAccessory
 * draws them in the 32-unit box: a sprout on its crown, a scarf round its
 * middle, headphones over its head, a star floating by it. They move with
 * the body and hold still (the star does not twinkle, so wearing one costs
 * no frames).
 */

/** `object` turned `yaw` about the vertical line through box point (x, y). */
function turned(object: Object3D, x: number, y: number, yaw: number): Group {
  const pivot = new Group();
  pivot.position.copy(toScene(x, y));
  object.position.sub(pivot.position);
  pivot.rotation.y = yaw;
  pivot.add(object);
  return pivot;
}

function sprout(): Group {
  const leaf = satin(petColor("leaf"), 0.45);
  const young = satin(petColor("leafLight"), 0.45);
  const big = slab(outline([["M", 16.8, 5.8], ["C", 17.6, 3.9, 19.9, 3.3, 21.3, 4], ["C", 20.6, 5.8, 18.6, 6.6, 16.8, 5.8]]), 0.16, leaf, 0.1);
  const small = slab(outline([["M", 16.4, 7], ["C", 15.4, 5.6, 13.6, 5.4, 12.6, 6.1], ["C", 13.4, 7.4, 15, 7.8, 16.4, 7]]), 0.16, young, 0.1);
  const group = new Group();
  group.add(
    curveTube([16, 9.2, 0], [16, 7.8, 0], [16.2, 6.6, 0], [16.9, 5.6, 0], 0.42, satin(petColor("stem"), 0.5)),
    turned(big, 16.8, 5.8, -0.45),
    turned(small, 16.4, 7, 0.35),
  );
  return group;
}

/** The band follows the body round, low at the front and higher behind, as the SVG's curve dips in the middle. */
function scarf(): Group {
  const points: Vector3[] = [];
  for (let i = 0; i < 28; i++) {
    const phi = (i / 28) * Math.PI * 2;
    const y = 23.3 + 2.55 * Math.cos(phi);
    const r = radiusAt(y) + 0.8;
    points.push(toScene(16 + Math.sin(phi) * r, y, Math.cos(phi) * r * BODY_DEPTH));
  }
  const cloth = satin(petColor("scarf"), 0.7);
  const band = new Mesh(new TubeGeometry(new CatmullRomCurve3(points, true), 72, 0.95, 10, true), cloth);
  const tail = slab(outline([["M", 21.6, 25.2], ["L", 23.2, 29.6], ["L", 25.4, 28.8], ["L", 24.2, 24.6], ["L", 21.6, 25.2]]), 0.35, satin(petColor("scarfShade"), 0.7), 0.14);
  tail.position.z = 5.6;
  const group = new Group();
  group.add(band, tail);
  return group;
}

/** The band arches over the crown just clear of the skin; a cup covers each side. */
function headphones(): Group {
  const ink = glossy(petColor("eye"), 0.3);
  const side = (y: number, s: -1 | 1): Point3 => [16 + s * (radiusAt(y) + 0.7), y, 0];
  const arch: Point3[] = [side(18, -1), side(14, -1), side(11, -1), side(9.3, -1), [16, 7.9, 0], side(9.3, 1), side(11, 1), side(14, 1), side(18, 1)];
  const group = new Group();
  group.add(tube(arch, 0.6, ink, 48));
  for (const s of [-1, 1] as const) {
    const cup = new Mesh(new CapsuleGeometry(1.8, 2, 6, 16), ink);
    cup.position.copy(toScene(16 + s * (radiusAt(18) + 0.9), 18, 0.5));
    cup.scale.x = 0.6;
    group.add(cup);
  }
  return group;
}

function star(): Group {
  const gold = glossy(petColor("gold"), 0.25);
  gold.emissive.copy(petColor("gold"));
  gold.emissiveIntensity = 0.3;
  const shape = outline([
    ["M", 23.4, 3.2],
    ["L", 24.4, 5.3],
    ["L", 26.7, 5.6],
    ["L", 25, 7.2],
    ["L", 25.4, 9.5],
    ["L", 23.4, 8.4],
    ["L", 21.4, 9.5],
    ["L", 21.8, 7.2],
    ["L", 20.1, 5.6],
    ["L", 22.4, 5.3],
    ["L", 23.4, 3.2],
  ]);
  const group = new Group();
  group.add(slab(shape, 0.7, gold, 0.25));
  return group;
}

/** Builds the model for an accessory. */
export function buildAccessory(kind: PetAccessory): Group {
  if (kind === "sprout") return sprout();
  if (kind === "scarf") return scarf();
  if (kind === "headphones") return headphones();
  return star();
}
