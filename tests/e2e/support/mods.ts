import { cpSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { REAL } from './env.ts';

/**
 * A temporary copy of the owner's Gewehr 41 mod without its Combat Extended patch, art and
 * source files (the owner's folders are only read, never written).
 */
export function copyGewehr(workDir: string): string {
  const source = join(REAL.customDir, '[OH] Gewehr 41');
  const target = join(workDir, 'Gewehr41');
  mkdirSync(target, { recursive: true });
  cpSync(join(source, 'About'), join(target, 'About'), { recursive: true });
  cpSync(join(source, 'Defs'), join(target, 'Defs'), { recursive: true });
  return target;
}
