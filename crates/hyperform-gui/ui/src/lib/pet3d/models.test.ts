import { Box3, Mesh, type Object3D } from "three";
import { describe, expect, it } from "vitest";
import type { EyeShape, PetProp } from "../domain/pet/emotions";
import { isDrawnEye } from "../domain/pet/faceShapes";
import { PET_ACCESSORIES } from "../domain/pet/looks";
import { buildAccessory } from "./accessories";
import { PetEyes } from "./eyes";
import { HelperOrbs, MAX_HELPERS, orbitAt } from "./helpers";
import { PropRig } from "./propRig";
import { buildProp } from "./props";

const PROPS: PetProp[] = ["glasses", "magnifier", "laptop", "clipboard", "broom", "sign", "paws"];
const EYES: EyeShape[] = ["open", "wide", "narrow", "half", "closed", "sad", "puppy", "uneven", "happy", "star", "heart", "spiral"];
const still = { x: 0, y: 0 };

/** The full view's canvas, in scene units: 48 across, centred above the feet. */
function inCanvas(root: Object3D) {
  root.updateMatrixWorld(true);
  const box = new Box3().setFromObject(root);
  expect(box.isEmpty()).toBe(false);
  for (const v of [box.min, box.max]) {
    expect(Number.isFinite(v.x + v.y + v.z)).toBe(true);
    expect(Math.abs(v.x)).toBeLessThanOrEqual(24);
    expect(v.y).toBeGreaterThanOrEqual(-12);
    expect(v.y).toBeLessThanOrEqual(36);
  }
}

function meshes(root: Object3D): Mesh[] {
  const out: Mesh[] = [];
  root.traverse((node) => node instanceof Mesh && out.push(node));
  return out;
}

describe("props", () => {
  it.each(PROPS)("builds the %s inside the canvas", (prop) => {
    const model = buildProp(prop, "pearl");
    expect(meshes(model.parts).length).toBeGreaterThan(0);
    expect(model.anchor).toBe(prop === "glasses" ? "face" : "body");
    inCanvas(model.parts);
  });

  it("pops a prop in, then holds or loops it, and swaps it when the mood changes", () => {
    const rig = new PropRig();
    rig.update("broom", "pearl", 0, false);
    expect(rig.body.children).toHaveLength(1);
    expect(rig.rate(100)).toBe("move");
    rig.update("broom", "pearl", 800, false);
    expect(rig.rate(800)).toBe("loop");
    rig.update("glasses", "pearl", 900, false);
    expect(rig.body.children).toHaveLength(0);
    expect(rig.face.children).toHaveLength(1);
    rig.update("glasses", "pearl", 2000, false);
    expect(rig.rate(2000)).toBeNull();
    rig.update(null, "pearl", 2100, false);
    expect(rig.rate(2100)).toBeNull();
    expect(rig.face.children).toHaveLength(0);
  });

  it("holds still under Reduce Motion", () => {
    const rig = new PropRig();
    rig.update("magnifier", "pearl", 0, true);
    expect(rig.rate(0)).toBeNull();
    expect(rig.body.children[0].scale.x).toBe(1);
  });
});

describe("accessories", () => {
  it.each(PET_ACCESSORIES.map((a) => a.id))("builds the %s inside the canvas", (kind) => {
    const model = buildAccessory(kind);
    expect(meshes(model).length).toBeGreaterThan(0);
    inCanvas(model);
  });
});

describe("eyes", () => {
  it.each(EYES)("shows %s eyes as beads or leaves them to the face texture", (shape) => {
    const eyes = new PetEyes();
    eyes.update({ eyes: shape, stage: "sprout", blinking: false, gaze: still, still: true }, 0);
    const beads = meshes(eyes.group).filter((m) => m.visible);
    if (isDrawnEye(shape)) expect(beads).toHaveLength(0);
    else {
      expect(beads.length).toBeGreaterThanOrEqual(2);
      inCanvas(eyes.group);
      for (const bead of beads) expect(bead.position.z).toBeGreaterThan(5);
    }
  });

  it("squeezes flat to blink and needs frames only while it does", () => {
    const eyes = new PetEyes();
    eyes.update({ eyes: "open", stage: "sprout", blinking: false, gaze: still, still: false }, 0);
    eyes.update({ eyes: "open", stage: "sprout", blinking: false, gaze: still, still: false }, 1000);
    expect(eyes.rate(1000)).toBeNull();
    const open = meshes(eyes.group)[0].scale.y;
    eyes.update({ eyes: "open", stage: "sprout", blinking: true, gaze: still, still: false }, 1000);
    expect(eyes.rate(1030)).toBe("move");
    eyes.update({ eyes: "open", stage: "sprout", blinking: true, gaze: still, still: false }, 1200);
    expect(meshes(eyes.group)[0].scale.y).toBeLessThan(open * 0.2);
    expect(eyes.rate(1200)).toBeNull();
  });

  it("slides over the body with the gaze", () => {
    const eyes = new PetEyes();
    eyes.update({ eyes: "open", stage: "sprout", blinking: false, gaze: still, still: true }, 0);
    const ahead = meshes(eyes.group)[0].position.clone();
    eyes.update({ eyes: "open", stage: "sprout", blinking: false, gaze: { x: 2, y: -1 }, still: true }, 0);
    const moved = meshes(eyes.group)[0].position;
    expect(moved.x - ahead.x).toBeCloseTo(2, 0);
    expect(moved.y - ahead.y).toBeGreaterThan(0.5);
  });
});

describe("helper orbs", () => {
  it("shows at most four, a quarter turn apart, in front at the bottom and behind at the top", () => {
    const orbs = new HelperOrbs();
    orbs.update(7, 0, false);
    expect(orbs.group.children.filter((o) => o.visible)).toHaveLength(MAX_HELPERS);
    expect(orbs.rate()).toBe("loop");
    const [front, , back] = [0, 1, 2].map((i) => orbitAt(i, 0));
    expect(front.z).toBeGreaterThan(0);
    expect(back.z).toBeLessThan(0);
    expect(front.y).toBeLessThan(back.y);
    orbs.update(0, 0, false);
    expect(orbs.rate()).toBeNull();
  });
});
