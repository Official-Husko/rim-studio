import { expect, gotoRoute, test } from '../support/fixtures.ts';

/**
 * Dark theme check on the live page: the page is dark, and no style rule outside the token
 * definitions carries a colour literal (every colour comes from a --rs- token).
 */
test.describe('dark theme', () => {
  test('the page background and text are dark and light', async ({ page, env }) => {
    await gotoRoute(page, env, '/setup');
    const colours = await page.evaluate(() => {
      const body = getComputedStyle(document.body);
      return { background: body.backgroundColor, color: body.color };
    });
    const channels = (text: string): number[] => (/rgba?\(([^)]+)\)/.exec(text)?.[1] ?? '').split(/[ ,/]+/).slice(0, 3).map(Number);
    const luminance = (c: number[]): number => 0.2126 * (c[0] ?? 0) + 0.7152 * (c[1] ?? 0) + 0.0722 * (c[2] ?? 0);
    expect(luminance(channels(colours.background))).toBeLessThan(60);
    expect(luminance(channels(colours.color))).toBeGreaterThan(180);
  });

  test('no colour literal outside the token definitions', async ({ page, env }) => {
    await gotoRoute(page, env, '/gallery');
    await page.waitForTimeout(500);
    const offenders = await page.evaluate(() => {
      const literal = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(|\boklch\(|\boklab\(|\blab\(|\blch\(|\bcolor\(/;
      const found: string[] = [];
      const visit = (rules: CSSRuleList): void => {
        for (const rule of Array.from(rules)) {
          if (rule instanceof CSSStyleRule) {
            // the token files define the literals; everything else must use var(--rs-...)
            const isTokenBlock = Array.from(rule.style).some((p) => p.startsWith('--rs-') && literal.test(rule.style.getPropertyValue(p)));
            for (const prop of Array.from(rule.style)) {
              if (prop.startsWith('--')) continue;
              const value = rule.style.getPropertyValue(prop);
              if (literal.test(value) && !isTokenBlock) found.push(`${rule.selectorText} { ${prop}: ${value} }`);
            }
          } else if ('cssRules' in rule) {
            visit((rule as CSSGroupingRule).cssRules);
          }
        }
      };
      for (const sheet of Array.from(document.styleSheets)) {
        try {
          visit(sheet.cssRules);
        } catch {
          /* a cross origin sheet cannot be read */
        }
      }
      return found;
    });
    expect(offenders.slice(0, 20)).toEqual([]);
  });
});
