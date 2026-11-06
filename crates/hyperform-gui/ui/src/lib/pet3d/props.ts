import { BoxGeometry, CircleGeometry, Group, Mesh, TorusGeometry, type Material } from "three";
import type { PetProp } from "../domain/pet/emotions";
import type { PetLook } from "../domain/pet/looks";
import { surfacePoint } from "./body";
import { glass, glossy, satin, unlit } from "./materials";
import { bodyColor, mixColors, petColor, propAlpha, propColor, toneColor } from "./palette";
import { ellipsoid, outline, roundedRect, slab, toScene, tube } from "./shapes";

/**
 * What the pet holds for its work, as small 3D models placed where
 * PetProps draws them in the 32-unit box: glasses to read, a magnifier to
 * search, a laptop to code, a clipboard to plan, a broom to tidy, a sign
 * to present a plan, and paws clasped to plead. How they pop in and loop
 * is in propRig.ts.
 */

/** A prop on the face (the glasses) slides with the gaze; the rest move with the body. */
export type PropAnchor = "face" | "body";

export interface PropModel {
  parts: Group;
  anchor: PropAnchor;
  /** The glowing parts (the laptop's logo), whose opacity the loop sets. */
  glows: Material[];
}

/** The front of the body at box point (x, y), plus `gap` toward the viewer. */
const front = (x: number, y: number, gap: number) => surfacePoint(x, y).z + gap;

/** A thin bar of the box from x0 to x1 at height y, for printed lines. */
function bar(x0: number, x1: number, y: number, z: number, height: number, material: Material): Mesh {
  const mesh = new Mesh(new BoxGeometry(x1 - x0, height, 0.06), material);
  mesh.position.copy(toScene((x0 + x1) / 2, y, z));
  return mesh;
}

function disc(x: number, y: number, z: number, r: number, material: Material): Mesh {
  const mesh = new Mesh(new CircleGeometry(r, 36), material);
  mesh.position.copy(toScene(x, y, z));
  return mesh;
}

function ring(x: number, y: number, z: number, r: number, thick: number, material: Material): Mesh {
  const mesh = new Mesh(new TorusGeometry(r, thick, 10, 40), material);
  mesh.position.copy(toScene(x, y, z));
  return mesh;
}

function glasses(): Group {
  const ink = glossy(petColor("eye"), 0.25);
  const lens = glass(propColor("glass"), propAlpha("glass"));
  const z = front(12.4, 18.4, 1.1);
  const group = new Group();
  for (const x of [12.4, 19.6]) group.add(disc(x, 18.4, z - 0.05, 2.5, lens), ring(x, 18.4, z, 2.6, 0.24, ink));
  group.add(tube([[15, 18.1, z], [16, 17.6, z + 0.15], [17, 18.1, z]], 0.22, ink));
  for (const [x, side] of [[9.8, -1], [22.2, 1]] as const) {
    const bend = 16 + side * 8.8;
    const back = 16 + side * 10.1;
    group.add(tube([[x, 17.8, z], [bend, 16.7, front(bend, 16.7, 0.6)], [back, 16.4, front(back, 16.4, 0.6)]], 0.2, ink));
  }
  return group;
}

function magnifier(): Group {
  const z = front(20.8, 18.6, 2);
  const group = new Group();
  group.add(
    tube([[23.4, 21.4, z], [26.8, 25.2, z - 0.4]], 0.6, satin(propColor("wood"), 0.5)),
    disc(20.8, 18.6, z - 0.05, 3.3, glass(propColor("glass"), propAlpha("glass"))),
    ring(20.8, 18.6, z, 3.5, 0.42, glossy(propColor("metal"), 0.3)),
    tube([[19, 16.9, z + 0.3], [19.9, 16.2, z + 0.35], [21.1, 16.1, z + 0.3]], 0.18, unlit(petColor("light"), 0.85)),
  );
  return group;
}

/** The laptop faces the pet, so the viewer sees the back of its lid and the logo, over a thin deck. */
function laptop(glows: Material[]): Group {
  const deck = new Mesh(new BoxGeometry(13.6, 0.45, 6), glossy(propColor("metal"), 0.32));
  deck.position.copy(toScene(16, 28.4, 7.6));
  const hinge = toScene(16, 28.2, 10.6);
  const lid = new Group();
  lid.position.copy(hinge);
  lid.rotation.x = -0.2;
  const screen = slab(roundedRect(10.4, 23.8, 11.2, 4.4, 0.8), 0.3, glossy(propColor("screen"), 0.25), 0.08);
  screen.position.set(-hinge.x, -hinge.y, 0);
  const logo = unlit(toneColor("working", "pearl"), 0.5);
  glows.push(logo);
  const mark = disc(16, 26.1, 0, 0.7, logo);
  mark.position.sub(hinge).setZ(0.26);
  lid.add(screen, mark);
  const group = new Group();
  group.add(deck, lid);
  return group;
}

function clipboard(): Group {
  const z = front(22.8, 23.9, 1.4);
  const ink = satin(petColor("shade"));
  const board = slab(roundedRect(19.2, 19.6, 7.2, 8.6, 0.9), 0.35, satin(propColor("board"), 0.55), 0.1);
  const paper = slab(roundedRect(19.9, 20.9, 5.8, 6.6, 0.4), 0.06, satin(propColor("paper"), 0.8), 0.02);
  const clip = slab(roundedRect(21.3, 19, 3, 1.5, 0.6), 0.5, glossy(propColor("metal"), 0.3), 0.12);
  board.position.z = z;
  paper.position.z = z + 0.26;
  clip.position.z = z + 0.35;
  const group = new Group();
  group.add(board, paper, clip, bar(20.8, 24.6, 22.8, z + 0.32, 0.35, ink), bar(20.8, 24.8, 24.4, z + 0.32, 0.35, ink), bar(20.8, 23.4, 26, z + 0.32, 0.35, ink));
  return group;
}

function broom(): Group {
  const straw = slab(
    outline([["M", 19.2, 26.1], ["L", 23.4, 27.6], ["L", 22.2, 30.9], ["Q", 19.3, 30.6, 17.6, 28.7], ["L", 19.2, 26.1]]),
    1.4,
    satin(propColor("straw"), 0.8),
    0.3,
  );
  straw.position.z = 6.2;
  const group = new Group();
  group.add(
    tube([[27.2, 11.6, 3.6], [21.4, 27, 6.2]], 0.6, satin(propColor("wood"), 0.5)),
    straw,
    tube([[19.6, 26.3, 7.1], [23.2, 27.6, 7.1]], 0.32, satin(petColor("scarf"))),
  );
  return group;
}

function sign(): Group {
  const z = 3.4;
  const ink = satin(petColor("shade"));
  const check = satin(toneColor("ok", "pearl"), 0.4);
  const board = slab(roundedRect(20.6, 5.2, 11.2, 7.8, 1.3), 0.45, satin(propColor("paper"), 0.7), 0.15);
  board.position.z = z;
  const face = z + 0.42;
  const group = new Group();
  group.add(
    tube([[26.4, 29.4, z - 0.6], [26.4, 12.6, z - 0.6]], 0.6, satin(propColor("wood"), 0.5)),
    board,
    tube([[22.4, 8.1, face], [23.3, 9, face]], 0.34, check),
    tube([[23.3, 9, face], [24.8, 7.2, face]], 0.34, check),
    bar(25.8, 30, 8.2, face, 0.5, ink),
    bar(22.6, 29.8, 10.9, face, 0.5, ink),
  );
  return group;
}

function paws(look: PetLook): Group {
  const fur = satin(mixColors(bodyColor(look), petColor("shade"), 0.8), 0.5);
  const group = new Group();
  for (const x of [14.5, 17.5]) group.add(ellipsoid(x, 25, front(x, 25, 0.25), [1.8, 1.35, 1.3], fur));
  return group;
}

/** Builds the model for a prop; paws take the body's color. */
export function buildProp(prop: PetProp, look: PetLook): PropModel {
  const glows: Material[] = [];
  const parts =
    prop === "glasses"
      ? glasses()
      : prop === "magnifier"
        ? magnifier()
        : prop === "laptop"
          ? laptop(glows)
          : prop === "clipboard"
            ? clipboard()
            : prop === "broom"
              ? broom()
              : prop === "sign"
                ? sign()
                : paws(look);
  return { parts, anchor: prop === "glasses" ? "face" : "body", glows };
}
