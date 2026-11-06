import { describe, expect, it } from "vitest";
import { PerspectiveCamera, Scene } from "three";
import { environment, renderView, webgl, webglSupported } from "./renderer.svelte";

describe("renderer", () => {
  it("reports no WebGL where there is no document, and draws nothing", () => {
    expect(webglSupported()).toBe(false);
    expect(environment()).toBeNull();
    expect(renderView({} as HTMLCanvasElement, new Scene(), new PerspectiveCamera(), 40, 40)).toBe(false);
    expect(webgl.lost).toBe(false);
    expect(webgl.restores).toBe(0);
  });
});
