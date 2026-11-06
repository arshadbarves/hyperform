/** Bind a `{ subscribe, getSnapshot }` store to a rune. Call from component init. */

export function bindStore<T>(store: {
  subscribe: (listener: () => void) => () => void;
  getSnapshot: () => T;
}): { readonly current: T } {
  let value = $state(store.getSnapshot());
  $effect(() => {
    value = store.getSnapshot();
    return store.subscribe(() => {
      value = store.getSnapshot();
    });
  });
  return {
    get current() {
      return value;
    },
  };
}
