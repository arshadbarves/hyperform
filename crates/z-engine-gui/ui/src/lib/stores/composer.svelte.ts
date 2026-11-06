import { addFile, addImage, dataUrlToImage, removeAt } from "../domain/attachments";
import { fileToDataUrl } from "../imageUtil";
import type { Attachment } from "../protocol/Attachment";

/** The composer draft, shared so starters, rewinds and `/memory` can fill it. */
class ComposerStore {
  draft = $state("");
  attachments: Attachment[] = $state.raw([]);
  /** Bumped to ask the textarea to take focus. */
  focusTick = $state(0);

  setDraft(text: string, focus = true) {
    this.draft = text;
    if (focus) this.focusTick += 1;
  }

  focus() {
    this.focusTick += 1;
  }

  addFile(path: string) {
    this.attachments = addFile(this.attachments, path);
  }

  addImage(image: Attachment) {
    this.attachments = addImage(this.attachments, image);
  }

  /** Downscale and attach pasted, dropped or picked images; other files are ignored. */
  async addImageFiles(files: File[]) {
    for (const file of files.filter((f) => f.type.startsWith("image/")).slice(0, 4)) {
      const url = await fileToDataUrl(file);
      const image = url ? dataUrlToImage(url) : null;
      if (image) this.addImage(image);
    }
  }

  removeAttachment(index: number) {
    this.attachments = removeAt(this.attachments, index);
  }

  /** Hand the attachments to a submit and clear them. */
  takeAttachments(): Attachment[] {
    const taken = this.attachments;
    this.attachments = [];
    return taken;
  }

  /** Put attachments back after a failed submit. */
  restoreAttachments(list: Attachment[]) {
    this.attachments = [...list, ...this.attachments];
  }

  clear() {
    this.draft = "";
    this.attachments = [];
  }
}

export const composer = new ComposerStore();
