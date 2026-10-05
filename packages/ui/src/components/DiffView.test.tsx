import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { DiffView, parseUnifiedDiff } from './DiffView';

const DIFF = `--- a/Defs/Gun.xml
+++ b/Defs/Gun.xml
@@ -3,3 +3,3 @@
 <ThingDef>
-  <label>old</label>
+  <label>new</label>
 </ThingDef>
`;

describe('parseUnifiedDiff', () => {
  it('classifies lines and numbers them from the hunk header', () => {
    const lines = parseUnifiedDiff(DIFF);
    expect(lines.map((l) => l.kind)).toEqual([
      'meta',
      'meta',
      'hunk',
      'context',
      'remove',
      'add',
      'context',
    ]);
    expect(lines[3]).toMatchObject({ oldNo: 3, newNo: 3 });
    expect(lines[4]).toMatchObject({ oldNo: 4, text: '  <label>old</label>' });
    expect(lines[5]).toMatchObject({ newNo: 4 });
    expect(lines[6]).toMatchObject({ oldNo: 5, newNo: 5 });
  });

  it('handles an empty diff', () => {
    expect(parseUnifiedDiff('')).toEqual([]);
  });

  it('handles a new file with a single hunk', () => {
    const lines = parseUnifiedDiff('@@ -0,0 +1,2 @@\n+a\n+b\n');
    expect(lines.filter((l) => l.kind === 'add')).toHaveLength(2);
  });
});

describe('DiffView', () => {
  it('shows signs for added and removed lines', () => {
    const { container } = render(<DiffView label="Changes" diff={DIFF} />);
    expect(screen.getByRole('region', { name: 'Changes' })).toBeTruthy();
    expect(container.querySelector('[data-kind="add"]')?.textContent).toContain('+');
    expect(container.querySelector('[data-kind="remove"]')?.textContent).toContain('-');
  });

  it('shows the empty text when there is nothing to show', () => {
    render(<DiffView label="Changes" diff="" emptyText="Unchanged" />);
    expect(screen.getByText('Unchanged')).toBeTruthy();
  });

  it('can be reached with the keyboard and takes extra classes', () => {
    render(<DiffView label="Edit" diff={'@@ -1 +1 @@\n-a\n+b\n'} class="max-h-60" />);
    const region = screen.getByRole('region', { name: 'Edit' });
    expect(region.getAttribute('tabindex')).toBe('0');
    expect(region.className).toContain('max-h-60');
  });
});
