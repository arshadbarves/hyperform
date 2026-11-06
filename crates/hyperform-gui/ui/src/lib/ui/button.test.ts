import { describe, expect, it } from "vitest";
import { buttonClass } from "./button";

describe("buttonClass", () => {
  it("maps each variant to its class", () => {
    expect(buttonClass()).toBe("btn-ghost");
    expect(buttonClass("accent")).toBe("btn-accent");
    expect(buttonClass("icon")).toBe("icon-btn");
    expect(buttonClass("icon-mini")).toBe("icon-btn-mini");
  });

  it("adds sizes to text buttons only", () => {
    expect(buttonClass("secondary", { size: "l" })).toBe("btn-secondary size-l");
    expect(buttonClass("secondary", { size: "m" })).toBe("btn-secondary");
    expect(buttonClass("icon", { size: "s" })).toBe("icon-btn");
  });

  it("keeps solid for danger and appends states and extra classes", () => {
    expect(buttonClass("danger", { solid: true })).toBe("btn-danger is-solid");
    expect(buttonClass("ghost", { solid: true })).toBe("btn-ghost");
    expect(buttonClass("icon", { active: true, spinning: true, className: "diff-refresh" })).toBe(
      "icon-btn is-active spinning diff-refresh",
    );
  });
});
