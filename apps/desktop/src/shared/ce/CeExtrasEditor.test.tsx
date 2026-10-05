import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { CeExtrasEditor } from './CeExtrasEditor';

const BASE = { oneHanded: false, beltFed: false };

describe('CeExtrasEditor', () => {
  it('lists every group with a one line state', () => {
    render(<CeExtrasEditor block={BASE} onChange={() => undefined} />);
    for (const title of [
      'Bow, ammo and reload',
      'Companion weapon tags',
      'Tool list',
      'Weapon platform',
      'Under barrel unit',
      'Other elements',
    ]) {
      expect(screen.getByText(title)).toBeTruthy();
    }
  });

  it('adds a companion tag through the block patch', () => {
    const onChange = vi.fn();
    render(<CeExtrasEditor block={{ ...BASE, extraTags: ['A'] }} onChange={onChange} />);
    fireEvent.input(screen.getByRole('textbox', { name: 'Weapon tags to add' }), {
      target: { value: 'B' },
    });
    fireEvent.click(screen.getAllByRole('button', { name: 'Add' })[0] as HTMLElement);
    expect(onChange).toHaveBeenLastCalledWith({ extraTags: ['A', 'B'] });
  });

  it('shows the suggestions at the top when there are some', () => {
    render(
      <CeExtrasEditor
        block={BASE}
        onChange={() => undefined}
        options={[
          {
            id: 'recoil-pattern',
            field: '/ce/recoilPattern',
            label: 'Recoil pattern',
            value: { kind: 'text', value: 'Mounted' },
            examples: 2,
            of: 3,
            why: 'two of three use it',
          },
        ]}
      />,
    );
    expect(screen.getByText('Suggested by your conversions')).toBeTruthy();
  });
});
