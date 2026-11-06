/** A question asked before an irreversible action; answered by the one ConfirmDialog in App. */
export interface ConfirmRequest {
  title: string;
  description?: string;
  confirmLabel?: string;
  tone?: "danger" | "default";
}

class ConfirmStore {
  request = $state<ConfirmRequest | null>(null);
  #resolve: ((ok: boolean) => void) | null = null;

  /** Resolves true when the user confirms; a newer question cancels an older one. */
  ask(request: ConfirmRequest): Promise<boolean> {
    this.#resolve?.(false);
    this.request = request;
    return new Promise((resolve) => {
      this.#resolve = resolve;
    });
  }

  settle(ok: boolean) {
    const resolve = this.#resolve;
    this.#resolve = null;
    this.request = null;
    resolve?.(ok);
  }
}

export const confirmStore = new ConfirmStore();
