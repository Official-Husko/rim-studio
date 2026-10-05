/** Timing used by the stores; replaced by a manual one in tests. */
export interface Scheduler {
  /** Run once before the next paint. Returns the cancel function. */
  frame(run: () => void): () => void;
  /** Run once after ms milliseconds. Returns the cancel function. */
  after(ms: number, run: () => void): () => void;
}

/** The scheduler of the browser: animation frames and timeouts. */
export const browserScheduler: Scheduler = {
  frame(run) {
    if (typeof requestAnimationFrame === 'function') {
      const id = requestAnimationFrame(() => run());
      return () => cancelAnimationFrame(id);
    }
    const id = setTimeout(run, 16);
    return () => clearTimeout(id);
  },
  after(ms, run) {
    const id = setTimeout(run, ms);
    return () => clearTimeout(id);
  },
};

/** Milliseconds of silence before an edited draft is saved. */
export const AUTOSAVE_IDLE_MS = 800;
/** Milliseconds of silence before the fit, the diff and the structure defaults are asked. */
export const SLOW_IDLE_MS = 200;
