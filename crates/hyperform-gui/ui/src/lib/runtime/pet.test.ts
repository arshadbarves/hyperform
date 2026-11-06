import { afterEach, describe, expect, it, vi } from "vitest";

const saved = vi.hoisted(() => ({ value: { version: 1, xp: 40, lastDay: "2026-09-29", streak: 1, counted: [] } as unknown }));
const writes = vi.hoisted(() => [] as unknown[]);

vi.mock("../commands/pet", () => ({
  loadPetGrowth: () => Promise.resolve(saved.value),
  savePetGrowth: (growth: unknown) => {
    writes.push(growth);
    return Promise.resolve();
  },
}));

const { pet } = await import("./pet.svelte");

afterEach(() => {
  vi.useRealTimers();
});

describe("pet runtime", () => {
  it("applies signals that arrive before the load, then celebrates a level-up once", async () => {
    vi.useFakeTimers();
    const day = new Date(2026, 8, 29, 12);
    pet.record({ kind: "turn", turnId: "t1", completed: true, verified: false }, day);
    expect(pet.growth.xp).toBe(0);

    await pet.load();
    expect(pet.growth.xp).toBe(50);
    expect(pet.level).toBe(2);
    expect(pet.levelUp).toMatchObject({ level: 2, accessories: ["sprout"] });
    pet.celebrated();
    expect(pet.levelUp).toBeNull();

    pet.record({ kind: "turn", turnId: "t1", completed: true, verified: false }, day);
    expect(pet.growth.xp).toBe(50);

    await vi.advanceTimersByTimeAsync(1000);
    expect(writes).toHaveLength(1);
    expect(writes[0]).toMatchObject({ xp: 50, counted: ["turn:t1"] });
  });

  it("wears only what it has unlocked, and saves the choice", async () => {
    vi.useFakeTimers();
    pet.wear("star");
    expect(pet.growth.wearing).toBeNull();
    pet.wear("sprout");
    expect(pet.growth.wearing).toBe("sprout");
    await vi.advanceTimersByTimeAsync(1000);
    expect(writes.at(-1)).toMatchObject({ wearing: "sprout" });
  });
});
