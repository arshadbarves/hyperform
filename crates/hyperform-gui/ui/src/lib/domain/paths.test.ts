import { describe, expect, it } from "vitest";
import { absolutePath } from "./paths";

describe("absolutePath", () => {
  it("joins a project-relative path onto the root", () => {
    expect(absolutePath("/repo/", "src/a.rs")).toBe("/repo/src/a.rs");
    expect(absolutePath("C:\\repo", "src\\a.rs")).toBe("C:\\repo\\src\\a.rs");
  });

  it("keeps a path that is already absolute", () => {
    expect(absolutePath("/repo", "/other/b.rs")).toBe("/other/b.rs");
    expect(absolutePath("C:\\repo", "D:\\x\\b.rs")).toBe("D:\\x\\b.rs");
  });
});
