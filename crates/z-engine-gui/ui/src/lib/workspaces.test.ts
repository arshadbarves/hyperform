import { describe, expect, it } from "vitest";
import { joinPath, sameWorkspacePath, wsBasename } from "./workspaces";

describe("joinPath", () => {
  it("joins with the root's separator and no doubled slashes", () => {
    expect(joinPath("/Users/me/proj/", "/src/a.rs")).toBe("/Users/me/proj/src/a.rs");
    expect(joinPath("C:\\work\\proj", "src/a.rs")).toBe("C:\\work\\proj\\src/a.rs");
  });
});

describe("sameWorkspacePath", () => {
  it("treats trailing slashes as the same folder", () => {
    expect(sameWorkspacePath("/Users/me/proj", "/Users/me/proj/")).toBe(true);
    expect(sameWorkspacePath("/Users/me/proj", "/Users/me/other")).toBe(false);
    expect(sameWorkspacePath(null, "/x")).toBe(false);
    expect(sameWorkspacePath("C:\\Users\\me\\proj", "C:/Users/me/proj")).toBe(true);
  });
});

describe("wsBasename", () => {
  it("uses the last path segment", () => {
    expect(wsBasename("/Users/me/z-engine")).toBe("z-engine");
    expect(wsBasename("/Users/me/z-engine/")).toBe("z-engine");
  });
});
