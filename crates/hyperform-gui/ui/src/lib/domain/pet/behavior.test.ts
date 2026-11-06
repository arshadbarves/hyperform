import { describe, expect, it } from "vitest";
import { NAP_MS, boopReaction, petBehavior, pickWander, reactionPose, type BehaviorInput } from "./behavior";
import type { PerchId } from "./perches";
import { pose } from "./pose";

const all = new Set<PerchId>(["island", "composer", "sidebar", "panel"]);

function input(over: Partial<BehaviorInput> = {}): BehaviorInput {
  return {
    level: "lively",
    roam: true,
    status: { kind: "idle", tone: "quiet" },
    stage: "chat",
    typing: false,
    dragging: false,
    overlayOpen: false,
    idleMs: 0,
    available: all,
    current: "composer",
    wander: null,
    ...over,
  };
}

describe("petBehavior", () => {
  it("has no pet when it is off", () => {
    expect(petBehavior(input({ level: "off" }))).toBeNull();
  });

  it("follows the pointer while dragged, wherever it is", () => {
    expect(petBehavior(input({ dragging: true, status: { kind: "working", tone: "working" } }))).toMatchObject({
      perch: "composer",
      activity: "drag",
    });
  });

  it("goes to the island when needed and rides it while working", () => {
    expect(petBehavior(input({ status: { kind: "attention", tone: "attention" } }))).toMatchObject({ perch: "island", activity: "watch" });
    expect(petBehavior(input({ status: { kind: "working", tone: "working" } }))).toMatchObject({ perch: "island", activity: "ride", size: 28 });
    expect(petBehavior(input({ status: { kind: "retrying", tone: "attention" } }))?.perch).toBe("island");
  });

  it("stays in the island at the calm level", () => {
    expect(petBehavior(input({ level: "calm", stage: "home", available: new Set(["island", "hero"]) }))).toMatchObject({
      perch: "island",
      activity: "sit",
    });
  });

  it("watches you type from the composer and celebrates a good finish", () => {
    expect(petBehavior(input({ current: "sidebar", typing: true }))).toMatchObject({ perch: "composer", activity: "watch" });
    expect(petBehavior(input({ current: "sidebar", status: { kind: "done", tone: "ok" } }))).toMatchObject({
      perch: "sidebar",
      activity: "celebrate",
    });
  });

  it("is big on Home, and stays on the hero spot even without roaming", () => {
    const onHome = input({ stage: "home", current: null, available: new Set(["island", "hero", "composer"]) });
    expect(petBehavior(onHome)).toMatchObject({ perch: "hero", activity: "sit", size: 72 });
    expect(petBehavior({ ...onHome, roam: false })?.perch).toBe("hero");
    expect(petBehavior(input({ roam: false }))).toMatchObject({ perch: "island" });
  });

  it("wanders to the edge the timer picked, tucks away for overlays, naps when idle", () => {
    expect(petBehavior(input({ wander: "sidebar" }))).toMatchObject({ perch: "sidebar", activity: "walk" });
    expect(petBehavior(input({ wander: "sidebar", current: "sidebar" }))).toMatchObject({ activity: "sit" });
    expect(petBehavior(input({ overlayOpen: true }))).toMatchObject({ perch: "composer", activity: "tucked" });
    expect(petBehavior(input({ idleMs: NAP_MS }))).toMatchObject({ perch: "composer", activity: "nap" });
  });

  it("falls back to the island when its perch leaves the screen", () => {
    expect(petBehavior(input({ current: "panel", available: new Set(["island"]) }))).toMatchObject({ perch: "island" });
    expect(petBehavior(input({ available: new Set() }))?.perch).toBe("island");
  });

  it("sleeps in the empty inbox", () => {
    const inbox = input({ stage: "inbox", current: null, available: new Set(["island", "empty"]), idleMs: NAP_MS });
    expect(petBehavior(inbox)).toMatchObject({ perch: "empty", activity: "nap", size: 64 });
  });
});

describe("reactionPose", () => {
  const idle = pose("idle", "center", "quiet");
  const busy = pose("busy", "center", "working");

  it("lets a reaction win over any activity", () => {
    expect(reactionPose(busy, "ride", "levelUp")).toMatchObject({ mood: "excited", particles: "confetti", tone: "working" });
    expect(reactionPose(idle, "nap", "booped").mood).toBe("love");
    expect(reactionPose(idle, "sit", "giggle").mood).toBe("giggle");
    expect(reactionPose(idle, "sit", "applied").mood).toBe("excited");
    expect(reactionPose(idle, "drag", "dizzy").mood).toBe("dizzy");
  });

  it("is surprised when picked up, sleeps, hums on a walk, and peeks", () => {
    expect(reactionPose(idle, "drag", null).mood).toBe("surprised");
    expect(reactionPose(idle, "nap", null)).toMatchObject({ mood: "asleep", gaze: "down" });
    expect(reactionPose(idle, "walk", null).mood).toBe("content");
    expect(reactionPose(busy, "walk", null)).toBe(busy);
    expect(reactionPose(idle, "tucked", null).mood).toBe("peeking");
    expect(reactionPose(busy, "ride", null)).toBe(busy);
  });

  it("keeps a proud finish when it celebrates, and cheers up a plain one", () => {
    const proud = pose("proud", "center", "ok");
    expect(reactionPose(proud, "celebrate", null)).toBe(proud);
    expect(reactionPose(idle, "celebrate", null)).toMatchObject({ mood: "happy", particles: "sparkles" });
  });

  it("watches you type from the composer", () => {
    expect(reactionPose(pose("listening", "down", "quiet"), "watch", null).mood).toBe("watching");
    const asking = pose("asking", "center", "attention");
    expect(reactionPose(asking, "watch", null)).toBe(asking);
  });

  it("shows an idle routine on its face only while nothing else is going on", () => {
    expect(reactionPose(idle, "sit", null, "yawn").mood).toBe("yawning");
    expect(reactionPose(idle, "sit", null, "lookAround").gaze).toBe("scan");
    expect(reactionPose(idle, "sit", null, "wave").mood).toBe("greeting");
    expect(reactionPose(busy, "sit", null, "yawn")).toBe(busy);
    expect(reactionPose(idle, "sit", "booped", "yawn").mood).toBe("love");
  });
});

describe("boopReaction", () => {
  it("giggles at a second boop", () => {
    expect(boopReaction(null)).toBe("booped");
    expect(boopReaction("booped")).toBe("giggle");
    expect(boopReaction("giggle")).toBe("giggle");
    expect(boopReaction("levelUp")).toBe("booped");
  });
});

describe("idle choices", () => {
  it("wanders to another edge on screen", () => {
    expect(pickWander(all, "composer", 0)).toBe("sidebar");
    expect(pickWander(all, "composer", 0.99)).toBe("panel");
    expect(pickWander(new Set(["island", "composer"]), "composer", 0.5)).toBeNull();
  });
});
