import { describe, expect, it } from "vitest";
import { PET_MOODS, canBlink, emotion, expression } from "./emotions";

describe("emotion", () => {
  it("has about thirty moods, each with a face and a body motion", () => {
    expect(PET_MOODS.length).toBeGreaterThanOrEqual(30);
    for (const mood of PET_MOODS) {
      const e = emotion(mood);
      expect(e.face.eyes).toBeTruthy();
      expect(e.body).toBeTruthy();
    }
  });

  it("gives work its props and bubbles", () => {
    expect(emotion("reading").prop).toBe("glasses");
    expect(emotion("searching").prop).toBe("magnifier");
    expect(emotion("coding").prop).toBe("laptop");
    expect(emotion("planning")).toMatchObject({ prop: "clipboard", bubble: "idea" });
    expect(emotion("tidying")).toMatchObject({ prop: "broom", body: "sweep" });
    expect(emotion("presenting").prop).toBe("sign");
    expect(emotion("thinking").bubble).toBe("dots");
    expect(emotion("asking").bubble).toBe("question");
  });

  it("shows how things went on its face", () => {
    expect(emotion("sad").face).toMatchObject({ eyes: "sad", mouth: "frown", tears: true });
    expect(emotion("worried").face.sweat).toBe(true);
    expect(emotion("determined").face).toMatchObject({ brows: "firm", sweat: true });
    expect(emotion("excited")).toMatchObject({ face: { eyes: "star" }, fx: "sparkles" });
    expect(emotion("dizzy")).toMatchObject({ face: { eyes: "spiral" }, fx: "stars" });
    expect(emotion("love")).toMatchObject({ face: { eyes: "heart" }, bubble: "hearts" });
    expect(emotion("asleep").bubble).toBe("zzz");
  });

  it("reads differently for every mood", () => {
    const looks = new Set(PET_MOODS.map((m) => JSON.stringify(emotion(m))));
    expect(looks.size).toBe(PET_MOODS.length);
  });
});

describe("expression", () => {
  it("adds a pose's particles on top of its mood", () => {
    expect(expression({ mood: "happy", particles: "sparkles" }).fx).toBe("sparkles");
    expect(expression({ mood: "excited", particles: "confetti" }).fx).toBe("confetti");
    expect(expression({ mood: "idle", particles: "thought" }).bubble).toBe("dots");
    expect(expression({ mood: "idle", particles: "sleep" }).bubble).toBe("zzz");
    expect(expression({ mood: "idle", particles: "none" })).toEqual(emotion("idle"));
  });

  it("blinks only open eyes", () => {
    expect(canBlink("open")).toBe(true);
    expect(canBlink("happy")).toBe(false);
    expect(canBlink("closed")).toBe(false);
  });
});
