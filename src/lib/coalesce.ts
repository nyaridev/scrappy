// coalesce: turns a burst of calls (a job event per file, for a thousand files) into a few: the first runs at once,
// the rest become one more run when the wait is over.

/** A function that runs `run` at most once every `ms`, and always once more after the last call. */
export function coalesce(run: () => void, ms: number): () => void {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let again = false;
  const end = () => {
    timer = null;
    if (!again) return;
    again = false;
    run();
    timer = setTimeout(end, ms);
  };
  return () => {
    if (timer) {
      again = true;
      return;
    }
    run();
    timer = setTimeout(end, ms);
  };
}
