import { Mesh, Object3D, PerspectiveCamera, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { PET_MOODS, emotion } from "../domain/pet/emotions";
import { EYES } from "../domain/pet/faceShapes";
import { PET_FEET } from "../domain/pet/perches";
import type { PetMood } from "../domain/pet/pose";
import { PORTRAIT_FRAME, PORTRAIT_GAZE, PORTRAIT_PITCH, PORTRAIT_YAW } from "../domain/pet/portrait";
import { FACE_YAW } from "../domain/pet/rig3d";
import { surfacePoint } from "./body";
import { PetScene, faster, frameCamera, type PetSceneState } from "./petScene";
import { toScene } from "./shapes";

function state(mood: PetMood, over: Partial<PetSceneState> = {}): PetSceneState {
  const { face, body, prop } = emotion(mood);
  return {
    face,
    body,
    prop,
    mood,
    tone: "quiet",
    look: "pearl",
    stage: "sprout",
    wearing: null,
    routine: null,
    helpers: 0,
    gaze: { x: 0, y: 0 },
    scan: false,
    searching: false,
    blinking: false,
    turn: 0,
    lift: [0, 0],
    bob: 0,
    head: null,
    still: false,
    ...over,
  };
}

/** Settles a scene into `s`: a first update, then one long after every pop-in and morph. */
function settled(s: PetSceneState, scene = new PetScene()) {
  scene.update(s, 0);
  scene.update(s, 5000);
  return scene;
}

/** Where box point (x, y) lands on the canvas, as fractions from its top left. */
function onCanvas(camera: PerspectiveCamera, x: number, y: number) {
  camera.updateMatrixWorld(true);
  const p = toScene(x, y).project(camera);
  return { x: (p.x + 1) / 2, y: (1 - p.y) / 2 };
}

describe("framing", () => {
  it("shows the whole box with a quarter of it spare each side, the feet where the flat pet's are", () => {
    const camera = new PerspectiveCamera();
    frameCamera(camera, "full");
    const feet = onCanvas(camera, 16, PET_FEET * 32);
    expect(feet.x).toBeCloseTo(0.5, 5);
    expect(feet.y).toBeCloseTo((PET_FEET * 32 + 8) / 48, 5);
    expect(onCanvas(camera, 0, 0).x).toBeCloseTo(8 / 48, 5);
    expect(onCanvas(camera, 32, 32).y).toBeCloseTo(40 / 48, 5);
  });

  it("closes in on the face for the portrait", () => {
    const camera = new PerspectiveCamera();
    frameCamera(camera, "portrait");
    const { cx, cy, span } = PORTRAIT_FRAME;
    expect(onCanvas(camera, cx, cy).y).toBeCloseTo(0.5, 5);
    expect(onCanvas(camera, cx, cy - span / 2).y).toBeCloseTo(0, 5);
    expect(onCanvas(camera, cx + span / 2, cy).x).toBeCloseTo(1, 5);
  });

  it("keeps the portrait's window filled to the crown and both eyes in it, in every mood and turn", () => {
    const turns = [null, ...Array.from({ length: 8 }, (_, k) => [Math.cos((k * Math.PI) / 4), Math.sin((k * Math.PI) / 4)] as const)];
    const v = new Vector3();
    for (const stage of ["seed", "sprout"] as const)
      for (const mood of PET_MOODS)
        for (const turn of turns) {
          const head = turn ? { yaw: turn[0] * PORTRAIT_YAW, pitch: turn[1] * PORTRAIT_PITCH } : null;
          // Looking its furthest toward the pet, as gazeToward reaches.
          const gaze = turn ? { x: turn[0] * 2, y: turn[1] * 1.8 } : { x: 0, y: 0 };
          const scene = new PetScene("portrait");
          const edges = (["left", "right"] as const).flatMap((side) =>
            [[-1, 0], [1, 0], [0, -1], [0, 1]].map(([a, b]) => {
              const { cx, cy, rx, ry } = EYES[side];
              const mark = new Object3D();
              mark.position.copy(surfacePoint(cx + a * rx + gaze.x * PORTRAIT_GAZE, cy + b * ry + gaze.y * PORTRAIT_GAZE));
              scene.rig.add(mark);
              return mark;
            }),
          );
          scene.update(state(mood, { stage, head, gaze, still: true }), 0);
          scene.scene.updateMatrixWorld(true);
          scene.camera.updateMatrixWorld(true);
          const skin = scene.body.skin.geometry.getAttribute("position");
          let crown = -Infinity;
          for (let i = 0; i < skin.count; i++) {
            v.fromBufferAttribute(skin, i).applyMatrix4(scene.body.skin.matrixWorld).project(scene.camera);
            if (Math.abs(v.x) < 0.15) crown = Math.max(crown, v.y);
          }
          const at = `${stage} ${mood} ${turn?.join(",") ?? "ahead"}`;
          // The status ring covers the window's outer tenth.
          expect(crown, at).toBeGreaterThan(0.9);
          for (const edge of edges) {
            edge.getWorldPosition(v).project(scene.camera);
            expect(Math.hypot(v.x, v.y), at).toBeLessThan(0.92);
          }
        }
  });
});

describe("the pet scene", () => {
  it("needs no frames while it only breathes", () => {
    const scene = settled(state("idle"));
    expect(scene.rate(5000)).toBeNull();
    expect(scene.rate(60_000)).toBeNull();
  });

  it("needs every frame while things pop in, then 30 fps while a mood loops", () => {
    const scene = new PetScene();
    scene.update(state("coding"), 0);
    expect(scene.rate(10)).toBe("move");
    scene.update(state("coding"), 5000);
    expect(scene.rate(5000)).toBe("loop");
  });

  it("holds a still pose under Reduce Motion", () => {
    const scene = new PetScene();
    scene.update(state("excited", { still: true, helpers: 2 }), 0);
    expect(scene.rate(0)).toBeNull();
  });

  it("plays the mood's move on the rig", () => {
    const scene = new PetScene();
    scene.update(state("excited"), 0);
    scene.update(state("excited"), 450);
    expect(scene.rig.position.y).toBeGreaterThan(1);
    const leaning = settled(state("watching"));
    expect(leaning.rig.rotation.z).toBeLessThan(0);
  });

  it("turns to face its way, bobs as it walks and lifts its feet", () => {
    const scene = settled(state("idle", { turn: 1, bob: 1, lift: [0.8, 0] }));
    expect(scene.turn.rotation.y).toBeCloseTo(FACE_YAW, 5);
    expect(scene.stride.position.y).toBe(-1);
    const [left, right] = scene.body.feet;
    expect(left.position.y - right.position.y).toBeCloseTo(0.8, 5);
    expect(right.getWorldPosition(new Vector3()).y).toBeCloseTo(toScene(16, 27.2).y, 5);
  });

  it("turns and tips the portrait's head, and keeps the seed small", () => {
    const scene = settled(state("idle", { head: { yaw: 0.3, pitch: 0.2 }, stage: "seed" }));
    expect(scene.turn.rotation.y).toBeCloseTo(0.3, 5);
    expect(scene.tip.rotation.x).toBeCloseTo(0.2, 5);
    expect(scene.turn.scale.x).toBeCloseTo(0.86, 5);
  });

  it("orbits helpers and needs frames for them", () => {
    const scene = settled(state("idle", { helpers: 2 }));
    expect(scene.rate(5000)).toBe("loop");
  });

  it("wears an accessory and swaps it", () => {
    const scene = settled(state("idle", { wearing: "scarf" }));
    const count = scene.rig.children.length;
    settled(state("idle", { wearing: "headphones" }), scene);
    expect(scene.rig.children).toHaveLength(count);
    settled(state("idle"), scene);
    expect(scene.rig.children).toHaveLength(count - 1);
  });

  it("frees its geometry and materials", () => {
    const scene = settled(state("coding", { wearing: "star", helpers: 1 }));
    let freed = 0;
    scene.scene.traverse((node) => node instanceof Mesh && node.geometry.addEventListener("dispose", () => freed++));
    scene.dispose();
    expect(freed).toBeGreaterThan(5);
  });

  it("takes the quicker frame rate", () => {
    expect(faster(null, "loop", null)).toBe("loop");
    expect(faster("loop", "move")).toBe("move");
    expect(faster(null, null)).toBeNull();
  });
});
