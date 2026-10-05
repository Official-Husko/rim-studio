import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
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

/** Copies chosen top level entries of one of the owner's mods into a temporary folder. */
export function copyOwnerMod(workDir: string, modName: string, entries: string[], target: string): string {
  const source = join(REAL.customDir, modName);
  const out = join(workDir, target);
  mkdirSync(out, { recursive: true });
  for (const entry of entries) cpSync(join(source, entry), join(out, entry), { recursive: true });
  return out;
}

/** A copy of Gewehr 41 with its Combat Extended patch outside the gated folder (a layout finding). */
export function copyGewehrWithPatch(workDir: string): string {
  return copyOwnerMod(workDir, '[OH] Gewehr 41', ['About', 'Defs', 'Patches', 'Config', 'Textures'], 'Gewehr41Layout');
}

/** A copy of the Lone Wolf package without its large art and sound folders. */
export function copyLoneWolf(workDir: string): string {
  return copyOwnerMod(workDir, '[OH] The Lone Wolf Weapon Package', ['About', 'Common', 'Patches'], 'LoneWolfLayout');
}

/** The sha256 of every file under a folder and the list of its folders, keyed by relative path. */
export function snapshot(root: string, dir = ''): Record<string, string> {
  const out: Record<string, string> = {};
  for (const entry of readdirSync(join(root, dir), { withFileTypes: true })) {
    const rel = dir === '' ? entry.name : `${dir}/${entry.name}`;
    if (entry.isDirectory()) {
      out[`${rel}/`] = 'dir';
      Object.assign(out, snapshot(root, rel));
    } else {
      out[rel] = createHash('sha256').update(readFileSync(join(root, rel))).digest('hex');
    }
  }
  return out;
}
