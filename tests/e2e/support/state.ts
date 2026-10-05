import type { ChildProcess } from 'node:child_process';

/** The processes the global setup started, for the global teardown to stop. */
export const started: ChildProcess[] = [];
