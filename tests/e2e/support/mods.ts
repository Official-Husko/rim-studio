import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/**
 * The stripped copies of the owner's Gewehr 41 and Lone Wolf mods that the toolkit tests use: byte
 * for byte the About, definition and patch files of the two mods, with placeholders for art and
 * sounds. The specs copy these instead of the live folders, so an edit of the owner's mods (a patch
 * renamed, a folder moved) cannot change what a test sees.
 */
const TREES = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../../crates/rimstudio-toolkit/tests/data/layout_fix',
);

/** Copies chosen top level entries of a committed tree into a temporary folder; absent ones are skipped. */
function copyTree(tree: 'gewehr41' | 'lonewolf', workDir: string, entries: string[], target: string): string {
  const source = join(TREES, tree);
  const out = join(workDir, target);
  mkdirSync(out, { recursive: true });
  for (const entry of entries) {
    if (existsSync(join(source, entry))) cpSync(join(source, entry), join(out, entry), { recursive: true });
  }
  return out;
}

/** Every file under a folder, as full paths. */
function filesUnder(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? filesUnder(join(dir, entry.name)) : [join(dir, entry.name)],
  );
}

/**
 * Writes an empty PNG for every texture path that the definitions of a copied mod name. The committed
 * trees keep no art, and the layout check would report each missing texture as a note; with the
 * placeholders a fully fixed mod reads as clean, as the owner's mod with its real art does.
 */
function addPlaceholderTextures(mod: string): void {
  const defs = join(mod, 'Defs');
  if (!existsSync(defs)) return;
  const paths = new Set<string>();
  for (const file of filesUnder(defs).filter((f) => f.endsWith('.xml'))) {
    for (const match of readFileSync(file, 'utf8').matchAll(/<texPath>([^<]+)<\/texPath>/g)) {
      const path = match[1]?.trim();
      if (path) paths.add(path);
    }
  }
  for (const path of paths) {
    const target = join(mod, 'Textures', `${path}.png`);
    if (existsSync(target)) continue;
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, '');
  }
}

/** A temporary copy of the Gewehr 41 mod without its Combat Extended patch, art and source files. */
export function copyGewehr(workDir: string): string {
  return copyTree('gewehr41', workDir, ['About', 'Defs'], 'Gewehr41');
}

/** A copy of Gewehr 41 with its Combat Extended patch outside the gated folder (a layout finding). */
export function copyGewehrWithPatch(workDir: string): string {
  const mod = copyTree('gewehr41', workDir, ['About', 'Defs', 'Patches', 'Config', 'Textures'], 'Gewehr41Layout');
  addPlaceholderTextures(mod);
  return mod;
}

/** A copy of the Lone Wolf package without its large art and sound folders. */
export function copyLoneWolf(workDir: string): string {
  return copyTree('lonewolf', workDir, ['About', 'Common', 'Patches'], 'LoneWolfLayout');
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
