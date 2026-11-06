import type { DecisionModelTest } from "../../commands/settings";

/** The Decision model card: endpoint checks and the "Test connection" result. */

/** A problem with an endpoint the user typed, or null when it can be used. */
export function endpointProblem(text: string): string | null {
  try {
    const url = new URL(text);
    if (url.protocol !== "http:" && url.protocol !== "https:") return "Use an http:// or https:// address.";
    return null;
  } catch {
    return "Enter a full address, such as http://127.0.0.1:8000.";
  }
}

function percent(confidence: number | null): string {
  return confidence === null ? "unknown" : `${Math.round(confidence * 100)}%`;
}

export type TestTone = "ok" | "warn" | "error";

/** What a connection test found, as one sentence and a tone. */
export function testSummary(test: DecisionModelTest): { tone: TestTone; text: string } {
  if (!test.ok) {
    const error = test.error ?? "the model did not answer";
    const text = test.warmingUp
      ? `No answer yet: ${error}. The model started moments ago and may still be loading; try again in a minute.`
      : `Not connected: ${error}.`;
    return { tone: "error", text };
  }
  const answered = `Answered in ${test.latencyMs} ms (${percent(test.confidence)} confident).`;
  if (test.answer !== "yes") {
    return {
      tone: "warn",
      text: `${answered} It answered the test question wrongly, so check the checkpoint.`,
    };
  }
  if (test.latencyMs > test.timeoutMs) {
    return {
      tone: "warn",
      text: `${answered} That is slower than the ${test.timeoutMs} ms timeout, so features would fall back; raise the timeout or use a faster setup.`,
    };
  }
  return { tone: "ok", text: `Connected. ${answered}` };
}
