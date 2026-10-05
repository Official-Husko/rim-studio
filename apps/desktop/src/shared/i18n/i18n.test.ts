import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import en from '~/locales/en.json';
import { t, tn, type MessageKey } from './index';

// vitest runs with the package folder as the working directory
const SRC = join(process.cwd(), 'src');
const DASHES = new RegExp(`[${String.fromCharCode(0x2013)}${String.fromCharCode(0x2014)}]`);
const catalog = en as Record<string, string>;

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else if (/\.(ts|tsx)$/.test(name)) out.push(full);
  }
  return out;
}

// The gallery shows component props with sample text; it is not part of the catalog.
const sources = walk(SRC).filter(
  (f) => !/\.test\.tsx?$/.test(f) && !relative(SRC, f).startsWith('gallery'),
);
const text = sources.map((f) => readFileSync(f, 'utf8')).join('\n');

describe('t', () => {
  it('returns the English text and fills placeholders', () => {
    expect(t('app.name')).toBe('RimStudio');
    expect(t('setup.remove.body', { name: 'X' })).toBe('X will no longer be scanned.');
  });

  it('keeps an unknown placeholder visible', () => {
    expect(t('setup.remove.body')).toBe('{name} will no longer be scanned.');
  });

  it('falls back to the key for an unknown key', () => {
    expect(t('does.not.exist' as MessageKey)).toBe('does.not.exist');
  });

  it('picks plural forms', () => {
    expect(tn('tasks.running', 1)).toBe('1 task running');
    expect(tn('tasks.running', 3)).toBe('3 tasks running');
  });
});

describe('catalog', () => {
  it('has every key that the code asks for', () => {
    const used = new Set<string>();
    for (const m of text.matchAll(/\bt\(\s*['"]([\w.-]+)['"]/g)) used.add(m[1] ?? '');
    for (const m of text.matchAll(/\btn\(\s*['"]([\w.-]+)['"]/g)) {
      used.add(`${m[1]}.one`);
      used.add(`${m[1]}.other`);
    }
    for (const m of text.matchAll(/(?:titleKey|labelKey|textKey):\s*['"]([\w.-]+)['"]/g))
      used.add(m[1] ?? '');
    expect(used.size).toBeGreaterThan(10);
    const missing = [...used].filter((k) => !(k in catalog));
    expect(missing).toEqual([]);
  });

  it('has no key that nothing uses', () => {
    const unused = Object.keys(catalog).filter((key) => {
      const base = key.replace(/\.(one|other)$/, '');
      return (
        !text.includes(`'${key}'`) &&
        !text.includes(`"${key}"`) &&
        !text.includes(`'${base}'`) &&
        !text.includes(`"${base}"`)
      );
    });
    expect(unused).toEqual([]);
  });

  it('writes no dashes or emojis in copy', () => {
    for (const value of Object.values(catalog)) {
      expect(value).not.toMatch(DASHES);
      expect(value).not.toMatch(/\p{Extended_Pictographic}/u);
    }
  });
});
