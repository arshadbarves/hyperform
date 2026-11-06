import { describe, expect, it } from "vitest";
import type { SlashCommandInfo } from "../commands/engine";
import { activeAtToken, replaceAtToken, stripAtToken } from "../atFile";
import { addFile, addImage, dataUrlToImage, fileExtension, imageSrc, removeAt } from "./attachments";
import { createComposerHistory } from "./composerHistory";
import { composerIntent, type ComposerKey } from "./composerKeys";
import { planSubmission, isRememberText } from "./composerSubmit";
import { helpMarkdown } from "./helpText";
import { rememberArgs } from "./remember";
import { filterCommands, mergeCommands, parseSlash, slashQuery } from "./slashCommands";

const cmd = (name: string, kind: SlashCommandInfo["kind"], hint: string | null = null): SlashCommandInfo => ({
  name,
  description: `${name} command`,
  argumentHint: hint,
  source: "builtin",
  kind,
});
const commands = [cmd("compact", "engine"), cmd("review", "prompt", "[path]"), cmd("agents", "ui")];
const base = { attachments: [], busy: false, interrupt: false, commands };

describe("planSubmission", () => {
  it("submits prompts with attachments when idle", () => {
    const attachments = [{ type: "file" as const, path: "a.ts" }];
    expect(planSubmission({ ...base, text: " hi ", attachments })).toEqual({ kind: "submit", text: "hi", attachments });
  });

  it("steers while busy and interrupts with the modifier", () => {
    expect(planSubmission({ ...base, text: "also", busy: true })).toEqual({ kind: "steer", text: "also" });
    expect(planSubmission({ ...base, text: "now", busy: true, interrupt: true })).toEqual({ kind: "interrupt", text: "now" });
    expect(planSubmission({ ...base, text: "", busy: true, interrupt: true })).toEqual({ kind: "interrupt", text: null });
    expect(planSubmission({ ...base, text: "x", interrupt: true }).kind).toBe("submit");
  });

  it("routes shell, memory and slash input", () => {
    expect(planSubmission({ ...base, text: "!git status" })).toEqual({ kind: "shell", command: "git status" });
    expect(planSubmission({ ...base, text: "# use pnpm" })).toEqual({ kind: "remember", text: "use pnpm" });
    expect(planSubmission({ ...base, text: "/review src" })).toEqual({ kind: "command", name: "review", args: "src" });
    expect(planSubmission({ ...base, text: "/agents" })).toEqual({ kind: "ui", name: "agents", args: "" });
    expect(planSubmission({ ...base, text: "/usr/bin is odd" }).kind).toBe("submit");
    expect(planSubmission({ ...base, text: "!" })).toEqual({ kind: "none" });
  });

  it("sends image-only prompts but never empty ones", () => {
    const attachments = [{ type: "image" as const, mediaType: "image/png", data: "AA" }];
    expect(planSubmission({ ...base, text: "", attachments }).kind).toBe("submit");
    expect(planSubmission({ ...base, text: "  " })).toEqual({ kind: "none" });
  });

  it("treats markdown headings as text", () => {
    expect(isRememberText("## Heading")).toBe(false);
    expect(isRememberText("#note")).toBe(true);
  });
});

describe("composerIntent", () => {
  const key = (over: Partial<ComposerKey>): ComposerKey => ({
    key: "Enter",
    shiftKey: false,
    metaKey: false,
    ctrlKey: false,
    altKey: false,
    isComposing: false,
    ...over,
  });
  const state = { busy: false, text: "hello", caret: 5 };

  it("maps Enter, modifiers and Escape", () => {
    expect(composerIntent(key({}), state)).toBe("send");
    expect(composerIntent(key({ shiftKey: true }), state)).toBe("none");
    expect(composerIntent(key({ metaKey: true }), state)).toBe("interrupt");
    expect(composerIntent(key({ isComposing: true }), state)).toBe("none");
    expect(composerIntent(key({ key: "Escape" }), { ...state, busy: true })).toBe("cancel");
    expect(composerIntent(key({ key: "Escape" }), state)).toBe("clear");
    expect(composerIntent(key({ key: "Escape" }), { ...state, text: "" })).toBe("none");
  });

  it("toggles plan mode with Shift+Tab, not with modified Tab", () => {
    expect(composerIntent(key({ key: "Tab", shiftKey: true }), state)).toBe("togglePlan");
    expect(composerIntent(key({ key: "Tab", shiftKey: true }), { ...state, busy: true, text: "" })).toBe("togglePlan");
    expect(composerIntent(key({ key: "Tab", shiftKey: true, metaKey: true }), state)).toBe("none");
    expect(composerIntent(key({ key: "Tab" }), state)).toBe("none");
  });

  it("walks history only from the first or last line", () => {
    const multi = { busy: false, text: "one\ntwo", caret: 5 };
    expect(composerIntent(key({ key: "ArrowUp" }), multi)).toBe("none");
    expect(composerIntent(key({ key: "ArrowDown" }), multi)).toBe("historyNext");
    expect(composerIntent(key({ key: "ArrowUp" }), { ...multi, caret: 2 })).toBe("historyPrev");
  });
});

describe("slash commands", () => {
  it("merges local UI commands behind engine ones", () => {
    const merged = mergeCommands([cmd("help", "engine")]);
    expect(merged.filter((c) => c.name === "help")).toHaveLength(1);
    expect(merged.find((c) => c.name === "help")?.kind).toBe("engine");
    expect(merged.some((c) => c.name === "agents")).toBe(true);
  });

  it("filters by prefix before substring", () => {
    expect(filterCommands(commands, "re").map((c) => c.name)).toEqual(["review"]);
    expect(filterCommands(commands, "pac").map((c) => c.name)).toEqual(["compact"]);
    expect(filterCommands(commands, "")).toHaveLength(3);
  });

  it("matches descriptions only from three characters", () => {
    expect(filterCommands(commands, "co").map((c) => c.name)).toEqual(["compact"]);
    expect(filterCommands(commands, "command").map((c) => c.name)).toEqual(["compact", "review", "agents"]);
  });

  it("parses the query and arguments", () => {
    expect(slashQuery("/rev")).toBe("rev");
    expect(slashQuery("/review src")).toBeNull();
    expect(slashQuery("plain")).toBeNull();
    expect(parseSlash("/review  src/lib ")).toEqual({ name: "review", args: "src/lib" });
    expect(parseSlash("/")).toBeNull();
  });

  it("documents commands and keys in /help", () => {
    const md = helpMarkdown(commands, "⌘");
    expect(md).toContain("`/review [path]`");
    expect(md).toContain("**App commands**");
    expect(md).toContain("⌘Enter");
  });
});

describe("mentions and attachments", () => {
  it("replaces the active @token", () => {
    expect(activeAtToken("see @src/li", 11)).toBe("src/li");
    expect(activeAtToken("email@x", 7)).toBeNull();
    expect(replaceAtToken("see @src/li now", 11, "@src/lib.rs ")).toEqual({ text: "see @src/lib.rs  now", caret: 16 });
    expect(stripAtToken("use @src", 8)).toEqual({ text: "use ", caret: 4 });
    expect(replaceAtToken("plain", 5, "x")).toEqual({ text: "plain", caret: 5 });
  });

  it("parses data URLs into image attachments", () => {
    const image = dataUrlToImage("data:image/jpeg;base64,QUJD");
    expect(image).toEqual({ type: "image", mediaType: "image/jpeg", data: "QUJD" });
    expect(image && image.type === "image" ? imageSrc(image) : "").toBe("data:image/jpeg;base64,QUJD");
    expect(dataUrlToImage("data:text/plain;base64,QUJD")).toBeNull();
  });

  it("dedupes files and caps images", () => {
    const files = addFile(addFile([], "a.ts"), "a.ts");
    expect(files).toHaveLength(1);
    const image = { type: "image" as const, mediaType: "image/png", data: "x" };
    let list = files;
    for (let i = 0; i < 8; i++) list = addImage(list, image, 3);
    expect(list).toHaveLength(4);
    expect(removeAt(list, 0)).toHaveLength(3);
    expect(fileExtension("src/app.svelte")).toBe("SVELTE");
  });

  it("formats remember arguments", () => {
    expect(rememberArgs("local", "  prefer pnpm ")).toBe("local prefer pnpm");
  });
});

describe("composer history", () => {
  it("walks back, then restores the unsent draft", () => {
    const history = createComposerHistory();
    history.push("first");
    history.push("second");
    history.push("second");
    expect(history.prev("draft")).toBe("second");
    expect(history.prev("ignored")).toBe("first");
    expect(history.prev("ignored")).toBeNull();
    expect(history.next()).toBe("second");
    expect(history.next()).toBe("draft");
    expect(history.next()).toBeNull();
  });
});
