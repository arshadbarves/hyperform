import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({ start: vi.fn(), poll: vi.fn(), loadCredentials: vi.fn() }));

vi.mock("../commands", () => ({ copilotLoginStart: ipc.start, copilotLoginPoll: ipc.poll }));
vi.mock("./settings.svelte", () => ({ settingsStore: { loadCredentials: ipc.loadCredentials } }));

const { CopilotLogin } = await import("./copilotLogin.svelte");

const URL = "https://api.githubcopilot.com";
const code = { userCode: "ABCD-1234", verificationUri: "https://github.com/login/device", deviceCode: "dc", interval: 5, expiresIn: 900 };

beforeEach(() => {
  vi.useFakeTimers();
  ipc.start.mockResolvedValue(code);
  ipc.loadCredentials.mockResolvedValue(undefined);
});

afterEach(() => {
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe("copilot sign-in loop", () => {
  it("shows the code, polls after interval plus margin, and reloads keys when done", async () => {
    ipc.poll.mockResolvedValueOnce({ status: "pending" }).mockResolvedValueOnce({ status: "done" });
    const login = new CopilotLogin();
    await login.start(URL);
    expect(ipc.start).toHaveBeenCalledWith(URL);
    expect(login.state).toEqual({ phase: "waiting", userCode: "ABCD-1234", verificationUri: code.verificationUri });
    expect(login.busy).toBe(true);

    await vi.advanceTimersByTimeAsync(7999);
    expect(ipc.poll).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(ipc.poll).toHaveBeenCalledWith(URL, "dc");
    await vi.advanceTimersByTimeAsync(8000);
    expect(ipc.poll).toHaveBeenCalledTimes(2);
    expect(login.state).toEqual({ phase: "done" });
    expect(ipc.loadCredentials).toHaveBeenCalledOnce();
  });

  it("waits GitHub's slower interval after slow down", async () => {
    ipc.poll.mockResolvedValueOnce({ status: "slowDown", interval: 10 }).mockResolvedValue({ status: "pending" });
    const login = new CopilotLogin();
    await login.start(URL);
    await vi.advanceTimersByTimeAsync(8000);
    await vi.advanceTimersByTimeAsync(12_999);
    expect(ipc.poll).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(ipc.poll).toHaveBeenCalledTimes(2);
    login.cancel();
  });

  it("stops polling on cancel", async () => {
    ipc.poll.mockResolvedValue({ status: "pending" });
    const login = new CopilotLogin();
    await login.start(URL);
    login.cancel();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(ipc.poll).not.toHaveBeenCalled();
    expect(login.state).toEqual({ phase: "idle" });
  });

  it("gives up when the code expires", async () => {
    ipc.start.mockResolvedValue({ ...code, expiresIn: 10 });
    ipc.poll.mockResolvedValue({ status: "pending" });
    const login = new CopilotLogin();
    await login.start(URL);
    await vi.advanceTimersByTimeAsync(16_000);
    expect(ipc.poll).toHaveBeenCalledTimes(2);
    expect(login.state).toMatchObject({ phase: "failed", reason: expect.stringContaining("expired") });
    await vi.advanceTimersByTimeAsync(60_000);
    expect(ipc.poll).toHaveBeenCalledTimes(2);
  });

  it("reports a refused sign-in or an unreachable GitHub", async () => {
    ipc.poll.mockResolvedValue({ status: "failed", reason: "access denied" });
    const login = new CopilotLogin();
    await login.start(URL);
    await vi.advanceTimersByTimeAsync(8000);
    expect(login.state).toEqual({ phase: "failed", reason: "access denied" });
    expect(login.busy).toBe(false);

    ipc.start.mockRejectedValueOnce(new Error("GitHub Copilot is experimental"));
    await login.start(URL);
    expect(login.state).toEqual({ phase: "failed", reason: "GitHub Copilot is experimental" });
    expect(ipc.loadCredentials).not.toHaveBeenCalled();
  });
});
