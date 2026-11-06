import { describe, expect, it } from "vitest";
import { activityLabel } from "./activityLabel";

describe("activityLabel", () => {
  it("names file work by the file, not the path", () => {
    expect(activityLabel("Read", { file_path: "/repo/src/auth.rs" })).toBe("Reading auth.rs");
    expect(activityLabel("MultiEdit", { file_path: "src/lib.rs" })).toBe("Editing lib.rs");
    expect(activityLabel("Write", { file_path: "docs/a.md" })).toBe("Writing a.md");
  });

  it("turns a Bash description into an ongoing action", () => {
    expect(activityLabel("Bash", { command: "cargo test", description: "Run the parser unit tests" })).toBe(
      "Running the parser unit tests",
    );
    expect(activityLabel("Bash", { command: "x", description: "Cargo tests for the parser" })).toBe(
      "Cargo tests for the parser",
    );
    expect(activityLabel("Bash", { command: "npm test\nnpm run lint" })).toBe("Running npm test");
  });

  it("describes searches and the web", () => {
    expect(activityLabel("Grep", { pattern: "parseConfig" })).toBe("Searching for parseConfig");
    expect(activityLabel("Glob", { pattern: "**/*.ts" })).toBe("Finding **/*.ts");
    expect(activityLabel("WebFetch", { url: "https://www.example.com/docs" })).toBe("Reading example.com");
    expect(activityLabel("WebSearch", { query: "svelte runes" })).toBe("Searching the web for svelte runes");
  });

  it("covers agents, plans, jobs, MCP and engine tools", () => {
    expect(activityLabel("Agent", { description: "Find the bug" })).toBe("Delegating: Find the bug");
    expect(activityLabel("TodoWrite", { todos: [] })).toBe("Updating the plan");
    expect(activityLabel("JobKill", { job_id: "j1" })).toBe("Stopping a background job");
    expect(activityLabel("JobOutput", { job_id: "j1" })).toBe("Checking a background job");
    expect(activityLabel("mcp__github__create_issue", {})).toBe("Using github · create issue");
    expect(activityLabel("Verify", {})).toBe("Checking the changes");
    expect(activityLabel("SomethingNew", {})).toBe("Running SomethingNew");
  });

  it("clips long subjects", () => {
    const label = activityLabel("Grep", { pattern: "x".repeat(80) });
    expect(label.length).toBe(52);
    expect(label.endsWith("…")).toBe(true);
  });
});
