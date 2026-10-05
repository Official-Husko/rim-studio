import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = process.cwd();
const css = readFileSync(join(ROOT, 'src/styles/tokens.css'), 'utf8');
const dark = css.slice(css.indexOf(':root,'), css.indexOf('/* Placeholder for the light theme'));

function token(name: string): string {
  const match = new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})\\b`).exec(dark);
  if (!match?.[1]) throw new Error(`no 6 digit hex token ${name}`);
  return match[1];
}

function luminance(hex: string): number {
  const channel = (i: number): number => {
    const v = parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
}

function ratio(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

// [foreground, background, documented ratio] from docs/design/design-prompt.md B2 (dark theme).
const PAIRS: Array<[string, string, number]> = [
  ['--rs-text', '--rs-bg', 15.5],
  ['--rs-text', '--rs-surface', 13.8],
  ['--rs-text', '--rs-surface-hover', 10.0],
  ['--rs-text-muted', '--rs-bg', 9.7],
  ['--rs-text-muted', '--rs-surface', 8.6],
  ['--rs-text-muted', '--rs-surface-hover', 6.2],
  ['--rs-text-faint', '--rs-bg', 6.4],
  ['--rs-text-faint', '--rs-surface', 5.7],
  ['--rs-text-faint', '--rs-surface-raised', 5.0],
  ['--rs-accent', '--rs-bg', 9.1],
  ['--rs-accent', '--rs-surface', 8.1],
  ['--rs-accent-contrast', '--rs-accent', 8.4],
  ['--rs-warning', '--rs-surface', 9.1],
  ['--rs-danger', '--rs-surface', 5.7],
  ['--rs-danger', '--rs-surface-raised', 5.0],
  ['--rs-success', '--rs-surface', 9.4],
  ['--rs-info', '--rs-surface', 6.8],
  ['--rs-rule-about', '--rs-surface', 6.8],
  ['--rs-rule-community', '--rs-surface', 6.3],
  ['--rs-rule-user', '--rs-surface', 8.2],
  ['--rs-focus', '--rs-bg', 12.1],
  ['--rs-focus', '--rs-surface-raised', 9.4],
  ['--rs-q-awful', '--rs-surface', 5.1],
  ['--rs-q-poor', '--rs-surface', 7.4],
  ['--rs-q-normal', '--rs-surface', 13.8],
  ['--rs-q-good', '--rs-surface', 9.6],
  ['--rs-q-excellent', '--rs-surface', 7.3],
  ['--rs-q-masterwork', '--rs-surface', 7.2],
  ['--rs-q-legendary', '--rs-surface', 9.7],
];

describe('dark theme tokens', () => {
  it.each(PAIRS)('%s on %s keeps the verified contrast of %f', (fg, bg, documented) => {
    expect(ratio(token(fg), token(bg))).toBeGreaterThanOrEqual(documented - 0.15);
  });

  it('keeps every text role at 4.5 or more on the surfaces it is allowed on', () => {
    for (const text of ['--rs-text', '--rs-text-muted']) {
      for (const surface of [
        '--rs-bg',
        '--rs-surface',
        '--rs-surface-raised',
        '--rs-surface-hover',
      ]) {
        expect(ratio(token(text), token(surface))).toBeGreaterThanOrEqual(4.5);
      }
    }
    for (const surface of ['--rs-bg', '--rs-surface', '--rs-surface-raised']) {
      expect(ratio(token('--rs-text-faint'), token(surface))).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('defines every token name of the brief', () => {
    const names = [
      '--rs-bg',
      '--rs-surface',
      '--rs-surface-raised',
      '--rs-surface-hover',
      '--rs-text',
      '--rs-text-muted',
      '--rs-text-faint',
      '--rs-accent',
      '--rs-accent-hover',
      '--rs-accent-press',
      '--rs-accent-contrast',
      '--rs-accent-tint',
      '--rs-warning',
      '--rs-danger',
      '--rs-success',
      '--rs-info',
      '--rs-rule-about',
      '--rs-rule-community',
      '--rs-rule-user',
      '--rs-q-awful',
      '--rs-q-poor',
      '--rs-q-normal',
      '--rs-q-good',
      '--rs-q-excellent',
      '--rs-q-masterwork',
      '--rs-q-legendary',
      '--rs-focus',
      '--rs-border-subtle',
      '--rs-border',
      '--rs-border-strong',
      '--rs-grid-minor',
      '--rs-grid-major',
      '--rs-hatch',
      '--rs-font-sans',
      '--rs-font-display',
      '--rs-font-mono',
      '--rs-row-h',
      '--rs-control-h',
      '--rs-radius-sm',
      '--rs-radius-md',
      '--rs-radius-lg',
      '--rs-scrim',
      '--rs-diff-added',
      '--rs-diff-removed',
      '--rs-band-p50',
      '--rs-band-p80',
      '--rs-source-official',
      '--rs-source-install',
      '--rs-source-workshop',
      '--rs-source-custom',
      '--rs-chart-1',
      '--rs-chart-6',
      '--rs-group-1',
      '--rs-group-8',
      '--rs-dur-fast',
      '--rs-dur-base',
      '--rs-dur-slow',
      '--rs-z-menu',
      '--rs-z-drawer',
      '--rs-z-dialog',
      '--rs-z-toast',
    ];
    const missing = names.filter((n) => !css.includes(`${n}:`));
    expect(missing).toEqual([]);
  });

  it('uses the exact alpha values of the brief for the border and texture tokens', () => {
    expect(css).toContain('--rs-border-subtle: #78beff2e');
    expect(css).toContain('--rs-border: #78beff52');
    expect(css).toContain('--rs-border-strong: #78beff8c');
  });
});

function walk(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules' || name === 'dist') continue;
    const full = join(dir, name);
    if (statSync(full).isDirectory()) walk(full, out);
    else out.push(full);
  }
  return out;
}

describe('tokens only', () => {
  const files = [...walk(join(ROOT, 'src')), ...walk(join(ROOT, '../../packages/ui/src'))];
  const code = files.filter((f) => /\.(ts|tsx|css)$/.test(f) && !/\.test\.tsx?$/.test(f));

  it('has no colour literals outside the token file and the legacy notice', () => {
    const offenders = code
      .filter((f) => !f.endsWith('tokens.css') && !f.endsWith('unsupported.ts'))
      .filter((f) => /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/.test(readFileSync(f, 'utf8')));
    expect(offenders).toEqual([]);
  });

  it('has no arbitrary length values in class names', () => {
    const offenders = code.filter((f) =>
      /\[\d+(\.\d+)?(px|rem|em)\]/.test(readFileSync(f, 'utf8')),
    );
    expect(offenders).toEqual([]);
  });
});
