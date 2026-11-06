import { describe, expect, it } from "vitest";
import { BODY_DEPTH, BODY_MIDDLE, PROFILE, radiusAt, surfaceNormal, surfacePoint } from "./body";

describe("the body's outline", () => {
  it("climbs from the base to the crown of the SVG egg", () => {
    expect(PROFILE[0]).toEqual([0, 27.2]);
    expect(PROFILE.at(-1)).toEqual([0, 8.6]);
    for (let i = 1; i < PROFILE.length; i++) expect(PROFILE[i][1]).toBeLessThanOrEqual(PROFILE[i - 1][1]);
  });

  it("is widest at its middle and closed at both ends", () => {
    expect(radiusAt(BODY_MIDDLE)).toBeCloseTo(10.6, 5);
    expect(radiusAt(14)).toBeLessThan(radiusAt(BODY_MIDDLE));
    expect(radiusAt(25)).toBeLessThan(radiusAt(BODY_MIDDLE));
    expect(radiusAt(8)).toBe(0);
    expect(radiusAt(28)).toBe(0);
  });

  it("puts the face's centre on the front of the body, facing the viewer", () => {
    const front = surfacePoint(16, BODY_MIDDLE);
    expect(front.x).toBe(0);
    expect(front.y).toBeCloseTo(28 - BODY_MIDDLE, 5);
    expect(front.z).toBeCloseTo(10.6 * BODY_DEPTH, 5);
    const normal = surfaceNormal(16, BODY_MIDDLE);
    expect(normal.z).toBeCloseTo(1, 3);
  });

  it("leans the surface outward toward the sides and the crown", () => {
    expect(surfaceNormal(24, BODY_MIDDLE).x).toBeGreaterThan(0.5);
    expect(surfaceNormal(8, BODY_MIDDLE).x).toBeLessThan(-0.5);
    expect(surfaceNormal(16, 10).y).toBeGreaterThan(0.5);
    expect(surfacePoint(40, BODY_MIDDLE).z).toBe(0);
  });
});
