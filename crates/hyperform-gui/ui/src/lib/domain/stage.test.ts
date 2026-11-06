import { describe, expect, it } from "vitest";
import { emptyView } from "./sessionView/types";
import { hasChatContent, stageFor } from "./stage";

describe("stageFor", () => {
  it("shows the inbox whenever it is asked for", () => {
    expect(stageFor("inbox", { hasContent: true, hydrating: false })).toBe("inbox");
    expect(stageFor("inbox", null)).toBe("inbox");
  });

  it("shows the project home without an open chat, or when home is asked for", () => {
    expect(stageFor("chat", null)).toBe("home");
    expect(stageFor("home", { hasContent: true, hydrating: false })).toBe("home");
  });

  it("keeps an empty new chat on the home screen until something is sent", () => {
    expect(stageFor("chat", { hasContent: false, hydrating: false })).toBe("home");
    expect(stageFor("chat", { hasContent: true, hydrating: false })).toBe("chat");
  });

  it("shows the chat while its history is still loading", () => {
    expect(stageFor("chat", { hasContent: false, hydrating: true })).toBe("chat");
  });
});

describe("hasChatContent", () => {
  it("is false for a fresh chat and for no chat", () => {
    expect(hasChatContent(null)).toBe(false);
    expect(hasChatContent(emptyView("s1"))).toBe(false);
  });

  it("is true once work starts or the user is asked something", () => {
    expect(hasChatContent({ ...emptyView("s1"), status: "busy" })).toBe(true);
    expect(hasChatContent({ ...emptyView("s1"), trustRequest: { projectRoot: "/p", defines: [] } })).toBe(true);
  });

  it("ignores terminal-drawer output, which is not part of the transcript", () => {
    const shell = { id: 1, name: "shell", markdown: "ls", afterMessageId: null, at: 0 };
    expect(hasChatContent({ ...emptyView("s1"), outputs: [shell] })).toBe(false);
    expect(hasChatContent({ ...emptyView("s1"), outputs: [{ ...shell, name: "cost" }] })).toBe(true);
  });
});
