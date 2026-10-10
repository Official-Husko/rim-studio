import { fireEvent, render, screen, within } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { NoticesDialog, parseNotice } from './NoticesDialog';

const SOURCE = `# Notices

Intro with a [link](https://example.test/a) and a bare https://example.test/b, then more.

## First

Text with \`code\` and a [relative](docs/x.md) label.

| Set | Where |
|---|---|
| one | \`a/**\` |
| two | https://example.test/c |
`;

describe('parseNotice', () => {
  it('reads headings, paragraphs and tables', () => {
    const blocks = parseNotice(SOURCE);
    expect(blocks.map((b) => b.kind)).toEqual(['heading', 'paragraph', 'heading', 'paragraph', 'table']);
    const table = blocks.at(-1);
    expect(table).toEqual({
      kind: 'table',
      head: ['Set', 'Where'],
      rows: [
        ['one', '`a/**`'],
        ['two', 'https://example.test/c'],
      ],
    });
  });
});

describe('NoticesDialog', () => {
  it('shows the notice without its title heading, as a table and with links', () => {
    render(<NoticesDialog open onClose={() => undefined} source={SOURCE} />);
    const dialog = screen.getByRole('dialog', { name: 'Notices and attributions' });
    expect(within(dialog).getByRole('heading', { name: 'First' })).toBeTruthy();
    expect(within(dialog).getByRole('table')).toBeTruthy();
    expect(within(dialog).getByRole('link', { name: 'link' }).getAttribute('href')).toBe(
      'https://example.test/a',
    );
    // a relative link keeps its label and is not a link
    expect(within(dialog).queryByRole('link', { name: 'relative' })).toBeNull();
    expect(within(dialog).getByText('relative', { exact: false })).toBeTruthy();
    // the trailing comma is not part of the address
    expect(
      within(dialog)
        .getAllByRole('link')
        .map((a) => a.getAttribute('href')),
    ).toContain('https://example.test/b');
  });

  it('opens a web address through the platform opener, not in place', () => {
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    render(<NoticesDialog open onClose={() => undefined} source={SOURCE} />);
    fireEvent.click(screen.getByRole('link', { name: 'link' }));
    expect(open).toHaveBeenCalledTimes(1);
    open.mockRestore();
  });

  it('carries the real notice with the RimWorld disclaimer and the Combat Extended licence', () => {
    render(<NoticesDialog open onClose={() => undefined} />);
    const dialog = screen.getByRole('dialog', { name: 'Notices and attributions' });
    expect(dialog.textContent).toContain('trademarks and/or copyrighted works of Ludeon Studios Inc.');
    expect(dialog.textContent).toContain('Creative Commons Attribution-NonCommercial-ShareAlike 4.0');
  });

  it('closes from the button', () => {
    const onClose = vi.fn();
    render(<NoticesDialog open onClose={onClose} source={SOURCE} />);
    const dialog = screen.getByRole('dialog', { name: 'Notices and attributions' });
    const buttons = within(dialog).getAllByRole('button', { name: 'Close' });
    fireEvent.click(buttons[buttons.length - 1] as HTMLElement);
    expect(onClose).toHaveBeenCalled();
  });
});
