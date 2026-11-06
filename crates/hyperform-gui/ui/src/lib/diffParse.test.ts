import { describe, expect, it } from "vitest";
import { looksLikeDiff, parseGitDiff, parseUnifiedDiff } from "./diffParse";

const SAMPLE = `--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,3 @@
 fn main() {
-    println!("old");
+    println!("new");
 }
`;

describe("looksLikeDiff", () => {
  it("accepts similar/git unified diffs", () => {
    expect(looksLikeDiff(SAMPLE)).toBe(true);
    expect(looksLikeDiff("diff --git a/a b/a\n--- a/a\n+++ b/a\n")).toBe(true);
    expect(looksLikeDiff("just a path")).toBe(false);
  });
});

describe("parseUnifiedDiff", () => {
  it("tags add/del/hunk/meta without treating --- as a deletion", () => {
    const kinds = parseUnifiedDiff(SAMPLE).map((l) => l.kind);
    expect(kinds).toEqual(["meta", "meta", "hunk", "ctx", "del", "add", "ctx"]);
  });

  it("keeps untracked-file previews (--- /dev/null, all additions)", () => {
    const lines = parseUnifiedDiff("--- /dev/null\n+++ b/new.txt\n+hello\n");
    expect(lines.map((l) => l.kind)).toEqual(["meta", "meta", "add"]);
    expect(lines[2].text).toBe("+hello");
  });
});

describe("parseGitDiff", () => {
  it("strips markers, numbers lines, and hides ---/+++ headers", () => {
    const d = parseGitDiff(SAMPLE);
    expect(d.path).toBe("src/lib.rs");
    expect(d.added).toBe(1);
    expect(d.deleted).toBe(1);
    expect(d.rows.map((r) => [r.kind, r.oldNo, r.newNo, r.text])).toEqual([
      ["ctx", 1, 1, "fn main() {"],
      ["del", 2, null, '    println!("old");'],
      ["add", null, 2, '    println!("new");'],
      ["ctx", 3, 3, "}"],
    ]);
  });

  it("treats a new file as numbered additions without raw unified headers", () => {
    const d = parseGitDiff(
      "--- a//Users/me/Desktop/dummy_sentinel.txt\n+++ b//Users/me/Desktop/dummy_sentinel.txt\n@@ -0,0 +1 @@\n+Hello, this is a dummy file created by Sentinel.\n",
    );
    expect(d.path).toBe("/Users/me/Desktop/dummy_sentinel.txt");
    expect(d.rows).toEqual([
      {
        kind: "add",
        oldNo: null,
        newNo: 1,
        text: "Hello, this is a dummy file created by Sentinel.",
      },
    ]);
  });

  it("shows created-file content even when @@ hunk header is missing", () => {
    const d = parseGitDiff("--- /dev/null\n+++ b/new.txt\n+hello\n+world\n");
    expect(d.path).toBe("new.txt");
    expect(d.added).toBe(2);
    expect(d.deleted).toBe(0);
    expect(d.rows.map((r) => r.text)).toEqual(["hello", "world"]);
    expect(d.rows.every((r) => r.kind === "add")).toBe(true);
  });

  it("inserts a hunk separator when later hunks skip lines", () => {
    const d = parseGitDiff(`--- a/a.rs
+++ b/a.rs
@@ -1,1 +1,1 @@
-one
+ONE
@@ -10,1 +10,1 @@
-ten
+TEN
`);
    expect(d.rows.map((r) => r.kind)).toEqual(["del", "add", "hunk", "del", "add"]);
    expect(d.rows[2]).toMatchObject({ kind: "hunk", newNo: 10 });
  });
});
