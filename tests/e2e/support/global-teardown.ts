import { started } from './state.ts';

/** Stops the bridge and Vite. */
export default function globalTeardown(): void {
  for (const child of started) {
    try {
      child.kill('SIGTERM');
    } catch {
      /* already gone */
    }
  }
}
