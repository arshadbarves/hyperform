import { copilotLoginPoll, copilotLoginStart, type CopilotDeviceCode, type CopilotLoginPoll } from "../commands";
import {
  codeDeadline,
  EXPIRED_REASON,
  hasExpired,
  pollDelayMs,
  pollStep,
  type CopilotLoginState,
} from "../domain/settings/copilotLogin";
import { errorText } from "../runtime/toasts";
import { settingsStore } from "./settings.svelte";

type Loop = { run: number; baseUrl: string; deviceCode: string; interval: number; deadline: number };

/**
 * One GitHub Copilot sign-in: asks GitHub for a device code, then polls until
 * the user approves it, it fails, or the code expires. Starting again or
 * cancelling stops the earlier loop; the owner cancels when it unmounts.
 */
export class CopilotLogin {
  state = $state.raw<CopilotLoginState>({ phase: "idle" });
  #run = 0;
  #timer: ReturnType<typeof setTimeout> | undefined;

  get busy(): boolean {
    return this.state.phase === "starting" || this.state.phase === "waiting";
  }

  async start(baseUrl: string): Promise<void> {
    const run = this.#restart();
    this.state = { phase: "starting" };
    let code: CopilotDeviceCode;
    try {
      code = await copilotLoginStart(baseUrl);
    } catch (e) {
      this.#fail(run, errorText(e));
      return;
    }
    if (run !== this.#run) return;
    this.state = { phase: "waiting", userCode: code.userCode, verificationUri: code.verificationUri };
    const deadline = codeDeadline(code, Date.now());
    this.#schedule({ run, baseUrl, deviceCode: code.deviceCode, interval: code.interval, deadline });
  }

  cancel() {
    this.#restart();
    this.state = { phase: "idle" };
  }

  #restart(): number {
    clearTimeout(this.#timer);
    this.#timer = undefined;
    return ++this.#run;
  }

  #fail(run: number, reason: string) {
    if (run === this.#run) this.state = { phase: "failed", reason };
  }

  #schedule(loop: Loop) {
    this.#timer = setTimeout(() => void this.#poll(loop), pollDelayMs(loop.interval));
  }

  async #poll(loop: Loop) {
    let poll: CopilotLoginPoll;
    try {
      poll = await copilotLoginPoll(loop.baseUrl, loop.deviceCode);
    } catch (e) {
      this.#fail(loop.run, errorText(e));
      return;
    }
    if (loop.run !== this.#run) return;
    const step = pollStep(poll, loop.interval);
    if (step.next === "done") {
      this.state = { phase: "done" };
      await settingsStore.loadCredentials();
    } else if (step.next === "failed") {
      this.#fail(loop.run, step.reason);
    } else if (hasExpired(Date.now(), loop.deadline)) {
      this.#fail(loop.run, EXPIRED_REASON);
    } else {
      this.#schedule({ ...loop, interval: step.interval });
    }
  }
}
