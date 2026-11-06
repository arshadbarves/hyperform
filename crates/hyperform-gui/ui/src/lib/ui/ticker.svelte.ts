/** A clock that ticks only while `active()` is true (live elapsed timers). Call during component init. */
export function ticker(active: () => boolean, intervalMs = 1000) {
  let now = $state(Date.now());
  $effect(() => {
    if (!active()) return;
    now = Date.now();
    const id = window.setInterval(() => {
      now = Date.now();
    }, intervalMs);
    return () => window.clearInterval(id);
  });
  return {
    get now() {
      return now;
    },
  };
}
