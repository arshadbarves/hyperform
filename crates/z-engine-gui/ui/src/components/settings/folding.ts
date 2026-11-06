import { getContext, setContext } from "svelte";

const FOLD_GROUPS = Symbol("settings-fold-groups");

/** Every settings group below the caller folds, so a long page opens calm. */
export function foldGroups(): void {
  setContext(FOLD_GROUPS, true);
}

export function groupsFold(): boolean {
  return getContext<boolean | undefined>(FOLD_GROUPS) ?? false;
}
