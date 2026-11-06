import type { ApprovalRequest } from "../protocol/ApprovalRequest";
import { firstLine } from "./tools/toolInput";

/** Lines of preview an approval shows before "Show all". */
export const PREVIEW_LINES = 6;

/** The approval as the question it is: "Allow Bash to run cargo test?". */
export function approvalQuestion(request: ApprovalRequest): string {
  const preview = request.preview;
  if (preview?.type === "command") return `Allow ${request.tool} to run ${firstLine(preview.command)}?`;
  if (preview?.type === "diff") return `Allow ${request.tool} to change ${preview.path}?`;
  const title = request.title.trim();
  return title.endsWith("?") ? title : `${title}?`;
}

export function previewLines(text: string): number {
  const body = text.replace(/\n+$/, "");
  return body ? body.split("\n").length : 0;
}
