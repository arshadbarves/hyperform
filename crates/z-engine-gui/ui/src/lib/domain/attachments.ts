import type { Attachment } from "../protocol/Attachment";
import { baseName } from "./tools/toolInput";

/** `data:image/png;base64,AAA` -> image attachment (base64 without the data-URL prefix). */
export function dataUrlToImage(url: string): Attachment | null {
  const match = /^data:([\w.+-]+\/[\w.+-]+);base64,(.+)$/s.exec(url);
  if (!match || !match[1].startsWith("image/")) return null;
  return { type: "image", mediaType: match[1], data: match[2] };
}

export function imageSrc(attachment: Extract<Attachment, { type: "image" }>): string {
  return `data:${attachment.mediaType};base64,${attachment.data}`;
}

export function addFile(list: Attachment[], path: string): Attachment[] {
  const clean = path.trim();
  if (!clean || list.some((a) => a.type === "file" && a.path === clean)) return list;
  return [...list, { type: "file", path: clean }];
}

export function addImage(list: Attachment[], image: Attachment, max = 6): Attachment[] {
  const images = list.filter((a) => a.type === "image").length;
  return images >= max ? list : [...list, image];
}

export function removeAt(list: Attachment[], index: number): Attachment[] {
  return list.filter((_, i) => i !== index);
}

export function attachmentLabel(attachment: Attachment): string {
  return attachment.type === "file" ? baseName(attachment.path) : "Image";
}

export function fileExtension(path: string): string {
  const name = baseName(path);
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toUpperCase() : "FILE";
}
