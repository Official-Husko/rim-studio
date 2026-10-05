import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = process.cwd();
const DASHES = new RegExp(`[${String.fromCharCode(0x2013)}${String.fromCharCode(0x2014)}]`);

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'dist') continue;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

const files = [
  ...walk(join(ROOT, 'src')),
  ...walk(join(ROOT, '../../packages/ui/src')),
  ...walk(join(ROOT, '../../packages/testkit/src')),
  join(ROOT, 'index.html'),
].filter((f) => /\.(ts|tsx|css|json|html)$/.test(f));

describe('project text rules', () => {
  it('has no em dashes, en dashes or emojis', () => {
    const offenders = files.filter(
      (f) =>
        DASHES.test(readFileSync(f, 'utf8')) ||
        /\p{Extended_Pictographic}/u.test(readFileSync(f, 'utf8')),
    );
    expect(offenders).toEqual([]);
  });

  it('mentions no AI tool and uses no old product name', () => {
    const offenders = files
      .filter((f) => !f.endsWith('hygiene.test.ts'))
      .filter((f) =>
        /\b(claude|anthropic|copilot|chatgpt)\b|rimforge/i.test(readFileSync(f, 'utf8')),
      );
    expect(offenders).toEqual([]);
  });

  it('keeps component files small', () => {
    const big = files
      .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f))
      .filter((f) => readFileSync(f, 'utf8').split('\n').length > 300);
    expect(big).toEqual([]);
  });

  it('gives every ui component a colocated test', () => {
    const components = files.filter((f) =>
      /packages\/ui\/src\/components\/[A-Z][A-Za-z]+\.tsx$/.test(f),
    );
    expect(components.length).toBeGreaterThan(30);
    const missing = components.filter((f) => !files.includes(f.replace(/\.tsx$/, '.test.tsx')));
    expect(missing).toEqual([]);
  });

  it('does not import an XML library or Tauri outside shared/platform and the Tauri transport', () => {
    const offenders = files
      .filter((f) => /\.tsx?$/.test(f) && !f.includes('shared/platform'))
      .filter((f) => !f.endsWith('shared/ipc/transports/tauri.ts'))
      .filter((f) => !f.endsWith('.test.ts') && !f.endsWith('.test.tsx'))
      .filter((f) =>
        /from ['"](@tauri-apps\/|fast-xml-parser|xml2js|@xmldom)/.test(readFileSync(f, 'utf8')),
      );
    expect(offenders).toEqual([]);
  });

  it('lets the Tauri transport import only the core and event APIs', () => {
    const source = readFileSync(join(ROOT, 'src/shared/ipc/transports/tauri.ts'), 'utf8');
    const imported = [...source.matchAll(/from ['"](@tauri-apps\/[^'"]+)['"]/g)].map((m) => m[1]);
    expect(imported.sort()).toEqual(['@tauri-apps/api/core', '@tauri-apps/api/event']);
  });

  it('keeps the Tauri packages out of every module but the two that need them', () => {
    const importers = files
      .filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f))
      .filter((f) => /@tauri-apps\//.test(readFileSync(f, 'utf8')))
      .map((f) => f.slice(ROOT.length + 1));
    expect(importers.sort()).toEqual([
      'src/shared/ipc/transports/tauri.ts',
      'src/shared/platform/dialogs.ts',
    ]);
  });
});
