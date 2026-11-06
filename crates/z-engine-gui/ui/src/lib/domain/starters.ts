/** A ready-made first prompt; `kind` picks its icon. */
export interface Starter {
  id: string;
  kind: "review" | "docs" | "explain" | "fix" | "test" | "clean";
  title: string;
  prompt: string;
}

const GENERAL: Starter[] = [
  {
    id: "explain",
    kind: "explain",
    title: "Explain how this project works",
    prompt: "Give me a short tour of this project: what it does, how it is laid out, and where to start reading.",
  },
  {
    id: "bug",
    kind: "fix",
    title: "Find and fix a real bug",
    prompt: "Look for a real bug in this project, explain why it is a bug, then fix it and add a test that proves it.",
  },
  {
    id: "tests",
    kind: "test",
    title: "Add tests where they matter",
    prompt: "Find an important piece of code that has no tests and add focused tests for its main paths and edge cases.",
  },
  {
    id: "cleanup",
    kind: "clean",
    title: "Simplify something messy",
    prompt: "Find one area with dead code or needless complexity and simplify it without changing its behavior.",
  },
];

/** Starters that fit the project: its own situation first, then general ones; three up front. */
export function startersFor(input: { changed: number; hasInstructions: boolean | null }): {
  top: Starter[];
  more: Starter[];
} {
  const own: Starter[] = [];
  if (input.changed > 0) {
    own.push({
      id: "review",
      kind: "review",
      title: `Review my ${input.changed} uncommitted change${input.changed === 1 ? "" : "s"}`,
      prompt:
        "Review my uncommitted changes. Point out bugs, risky edits and anything that needs a test, and don't change any code.",
    });
  }
  if (input.hasInstructions === false) {
    own.push({
      id: "agents-md",
      kind: "docs",
      title: "Write an AGENTS.md for this project",
      prompt:
        "Explore this project and write an AGENTS.md that tells an AI agent how to build, test and find its way around it. Keep it short and factual.",
    });
  }
  const all = [...own, ...GENERAL];
  return { top: all.slice(0, 3), more: all.slice(3) };
}
