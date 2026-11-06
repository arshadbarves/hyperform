import { describe, expect, it } from "vitest";
import type { DecisionModelStatus } from "../../commands/settings";
import { formatBytes, nativeModelView, RUNTIME_OPTIONS } from "./nativeModel";

function status(patch: Partial<DecisionModelStatus> = {}): DecisionModelStatus {
  return {
    checkpoint: "multilingual",
    available: true,
    repo: "onnx-community/laya-multilingual-ONNX",
    revision: "46b77bbf5642fec5f14e540570228a8cbe8ab81f",
    dir: "/data/models/laya/46b77bbf5642fec5f14e540570228a8cbe8ab81f",
    totalBytes: 1_325_555_768,
    downloadedBytes: 0,
    installed: false,
    downloading: false,
    error: null,
    nativeBuilt: true,
    ...patch,
  };
}

describe("formatBytes", () => {
  it("uses decimal units with one decimal below 100", () => {
    expect(formatBytes(512)).toBe("512 bytes");
    expect(formatBytes(34_363_188)).toBe("34.4 MB");
    expect(formatBytes(1_325_555_768)).toBe("1.3 GB");
    expect(formatBytes(150_000_000)).toBe("150 MB");
  });
});

describe("nativeModelView", () => {
  it("offers a download when nothing is on disk", () => {
    const view = nativeModelView(status());
    expect(view).toMatchObject({ tone: "quiet", action: "download", actionLabel: "Download", progress: null });
    expect(view.text).toContain("1.3 GB");
  });

  it("shows progress and a cancel while downloading", () => {
    const view = nativeModelView(status({ downloading: true, downloadedBytes: 662_777_884 }));
    expect(view).toMatchObject({ tone: "working", action: "cancel" });
    expect(view.progress).toBeCloseTo(0.5, 3);
    expect(view.text).toContain("(50%)");
  });

  it("resumes a partial or failed download", () => {
    expect(nativeModelView(status({ downloadedBytes: 10 })).actionLabel).toBe("Resume");
    const failed = nativeModelView(status({ downloadedBytes: 10, error: "connection reset" }));
    expect(failed).toMatchObject({ tone: "warn", action: "download", actionLabel: "Resume" });
    expect(failed.text).toContain("kept for the next try");
  });

  it("offers removal once downloaded, and warns when the app cannot run it", () => {
    const done = status({ installed: true, downloadedBytes: 1_325_555_768 });
    expect(nativeModelView(done)).toMatchObject({ tone: "ok", action: "remove" });
    expect(nativeModelView(done).text).toContain("46b77bb");
    const unbuilt = nativeModelView({ ...done, nativeBuilt: false });
    expect(unbuilt.tone).toBe("warn");
    expect(unbuilt.text).toContain("use the sidecar");
  });

  it("explains a checkpoint without a model", () => {
    const view = nativeModelView(status({ available: false, error: 'no model for checkpoint "gpt"' }));
    expect(view).toMatchObject({ tone: "danger", action: null });
    expect(view.text).toContain('"gpt"');
  });

  it("lists both runtimes", () => {
    expect(RUNTIME_OPTIONS.map((option) => option.value)).toEqual(["sidecar", "native"]);
  });
});
