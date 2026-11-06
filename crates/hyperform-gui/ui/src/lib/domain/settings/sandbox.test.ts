import { describe, expect, it } from "vitest";
import { sandboxPlatform, sandboxPlatformNote, writableDirError } from "./sandbox";

describe("sandbox settings", () => {
  it("picks the platform and explains its backend", () => {
    expect(sandboxPlatform(true, false)).toBe("mac");
    expect(sandboxPlatform(false, true)).toBe("windows");
    expect(sandboxPlatform(false, false)).toBe("linux");
    expect(sandboxPlatformNote("mac")).toContain("sandbox-exec");
    expect(sandboxPlatformNote("linux")).toContain("bubblewrap");
    expect(sandboxPlatformNote("windows")).toContain("Not available");
  });

  it("refuses entries that open up the whole disk", () => {
    expect(writableDirError("~/scratch")).toBeNull();
    expect(writableDirError("build/out")).toBeNull();
    expect(writableDirError("/")).not.toBeNull();
    expect(writableDirError("~")).not.toBeNull();
    expect(writableDirError("~/")).not.toBeNull();
    expect(writableDirError("a\nb")).toBe("Enter one directory per entry.");
  });
});
