import { describe, expect, it } from "vitest";
import { assistant, text, user } from "../testFixtures";
import { visibleText } from "./blocks";
import { commandChip } from "./commandChip";

const wrapped = (name: string, body: string) => `<command name="${name}">\n${body}\n</command>`;

describe("commandChip", () => {
  it("parses the invocation and the wrapped body", () => {
    const message = user("u1", [
      text("/review src/lib.rs"),
      text(wrapped("review", "Review code changes.\n\nTarget: src/lib.rs")),
      text("<system-reminder>\nplan mode\n</system-reminder>"),
    ]);
    expect(commandChip(message)).toEqual({
      name: "review",
      args: "src/lib.rs",
      invocation: "/review src/lib.rs",
      body: "Review code changes.\n\nTarget: src/lib.rs",
    });
  });

  it("handles commands without arguments and MCP prompt names", () => {
    const chip = commandChip(user("u1", [text("/mcp__fake__review"), text(wrapped("mcp__fake__review", "Please"))]));
    expect(chip?.args).toBe("");
    expect(chip?.name).toBe("mcp__fake__review");
  });

  it("keeps the body out of the visible text", () => {
    const message = user("u1", [text("/commit"), text(wrapped("commit", "Commit the changes."))]);
    expect(visibleText(message)).toBe("/commit");
  });

  it("ignores ordinary prompts and mismatched wrappers", () => {
    expect(commandChip(user("u1", "/review this please"))).toBeNull();
    expect(commandChip(user("u1", [text("/review"), text(wrapped("commit", "x"))]))).toBeNull();
    expect(commandChip(user("u1", [text("hello"), text("world")]))).toBeNull();
    expect(commandChip(assistant("a1", [text("/review"), text(wrapped("review", "x"))]))).toBeNull();
  });
});
