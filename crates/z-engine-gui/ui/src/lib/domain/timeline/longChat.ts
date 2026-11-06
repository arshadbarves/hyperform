import type { ContentBlock } from "../../protocol/ContentBlock";
import type { Message } from "../../protocol/Message";
import type { SessionSnapshot } from "../../protocol/SessionSnapshot";
import type { TurnRecord } from "../../protocol/TurnRecord";
import type { Usage } from "../../protocol/Usage";

/**
 * A long, finished chat for performance checks: a unit test builds its
 * timeline, and in dev builds `__zengine.longChat(1000)` opens it in the app
 * (lib/runtime/devChat.ts). Turns vary: plain answers, one call, runs of reads
 * and edits, a failed command, code blocks and tables.
 */

const USAGE: Usage = { inputTokens: 1200, outputTokens: 400, cacheReadTokens: 0, cacheWriteTokens: 0, reasoningTokens: 0 };
const FILES = ["src/lib.rs", "src/engine/run.rs", "ui/src/App.svelte", "crates/z-engine-host/src/fs.rs", "README.md"];

function answer(i: number): string {
  const file = FILES[i % FILES.length];
  const parts = [
    `## Turn ${i + 1}\n\nHere is what I found in \`${file}\`. The loop reads each **event**, applies it, and hands the result on.`,
    "- The reducer stays pure.\n- Effects run after the state settles.\n- Nothing blocks the render.",
  ];
  if (i % 3 === 0) parts.push("```rust\nfn apply(state: &mut State, event: Event) {\n    state.seq += 1;\n    state.events.push(event);\n}\n```");
  if (i % 5 === 0) parts.push("| Step | Time |\n| --- | --- |\n| parse | 2 ms |\n| render | 6 ms |");
  if (i % 4 === 1) parts.push("```ts\nexport function total(items: number[]): number {\n  return items.reduce((a, b) => a + b, 0);\n}\n```");
  parts.push(`That settles question ${i + 1}; the rest of the module is unchanged.`);
  return parts.join("\n\n");
}

function prompt(i: number): string {
  const long = i % 7 === 3 ? "\n\nAlso, while you are there, check how errors travel back to the caller and whether retries are capped." : "";
  return `Question ${i + 1}: how does ${FILES[i % FILES.length]} handle a new event?${long}`;
}

function call(id: string, name: string, input: Record<string, string>): ContentBlock {
  return { type: "toolUse", id, name, input };
}

function result(id: string, text: string, isError = false): ContentBlock {
  return { type: "toolResult", toolUseId: id, content: [{ type: "text", text }], isError };
}

/** The messages of turn `i`, starting at time `t`. */
function turnMessages(i: number, t: number): Message[] {
  const file = FILES[i % FILES.length];
  const say = (role: Message["role"], n: number, content: ContentBlock[]): Message => ({ id: `m${i}-${n}`, role, content, createdAt: t + n });
  const user: Message = { id: `u${i}`, role: "user", content: [{ type: "text", text: prompt(i) }], createdAt: t };
  const final = say("assistant", 9, [{ type: "text", text: answer(i) }]);
  if (i % 6 === 5) return [user, final];
  if (i % 6 === 4) {
    return [user, say("assistant", 1, [call(`c${i}a`, "Read", { file_path: file })]), say("user", 2, [result(`c${i}a`, "fn main() {}")]), final];
  }
  const failed = i % 10 === 2;
  return [
    user,
    say("assistant", 1, [
      { type: "thinking", text: `The user asks about ${file}. Read it first, then the tests.`, signature: null },
      { type: "text", text: "Let me look at the code first." },
      call(`c${i}a`, "Read", { file_path: file }),
      call(`c${i}b`, "Grep", { pattern: "fn apply" }),
    ]),
    say("user", 2, [result(`c${i}a`, "fn apply() {}\n".repeat(20)), result(`c${i}b`, `${file}:12: fn apply`)]),
    say("assistant", 3, [call(`c${i}c`, "Edit", { file_path: file, old_string: "a", new_string: "b" }), call(`c${i}d`, "Bash", { command: "cargo test" })]),
    say("user", 4, [result(`c${i}c`, "ok"), result(`c${i}d`, failed ? "error: 1 test failed" : "test result: ok. 12 passed", failed)]),
    final,
  ];
}

export function longChatSnapshot(turns = 1000, sessionId = "dev-long-chat", projectRoot = "/tmp/long-chat"): SessionSnapshot {
  const messages: Message[] = [];
  const records: TurnRecord[] = [];
  for (let i = 0; i < turns; i++) {
    const t = 1_700_000_000_000 + i * 120_000;
    messages.push(...turnMessages(i, t));
    records.push({
      turnId: `t${i}`,
      messageId: `u${i}`,
      outcome: { type: "completed" },
      verification: { status: "notApplicable" },
      usage: USAGE,
      costUsd: 0.01,
      startedAt: t,
      finishedAt: t + 20_000 + (i % 9) * 7_000,
    });
  }
  const at = messages.at(-1)?.createdAt ?? 0;
  return {
    info: {
      sessionId,
      title: `Long chat (${turns} turns)`,
      projectRoot,
      model: "anthropic/claude-sonnet-4",
      mode: "default",
      planning: false,
      effort: null,
      createdAt: 1,
      updatedAt: at,
      legacy: false,
    },
    status: "idle",
    messages,
    compactions: [],
    taskViews: [],
    turns: records,
    todos: [],
    agents: [],
    jobs: [],
    checks: [],
    checkpoints: [],
    pendingApprovals: [],
    pendingQuestions: [],
    pendingPlans: [],
    queued: [],
    usage: USAGE,
    costUsd: turns * 0.01,
    contextTokens: 42_000,
    contextLimit: 200_000,
  };
}
