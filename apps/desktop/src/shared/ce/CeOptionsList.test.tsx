import { fireEvent, render, screen } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import type { CeOptionDto } from 'rimstudio-ipc-types';
import { CeOptionsList } from './CeOptionsList';

const BASE = { oneHanded: false, beltFed: false };
const TAGS: CeOptionDto = {
  id: 'extra-tags',
  field: '/ce/extraTags',
  label: 'Companion weapon tags',
  value: { kind: 'tags', value: ['CE_AI_AssaultWeapon', 'CE_Rifle'] },
  examples: 3,
  of: 4,
  why: 'converted guns of the class also carry CE_AI_AssaultWeapon (3 of 4)',
};
const RELOAD: CeOptionDto = {
  id: 'reload-one-at-a-time',
  field: '/ce/reloadOneAtATime',
  label: 'Reload one round at a time',
  value: { kind: 'flag', value: true },
  examples: 2,
  of: 2,
  why: '2 of the 2 converted guns load one round at a time',
};

describe('CeOptionsList', () => {
  it('shows nothing without suggestions', () => {
    const { container } = render(
      <CeOptionsList options={[]} block={BASE} onChange={() => undefined} />,
    );
    expect(container.textContent).toBe('');
  });

  it('shows the habit, how many weapons share it, and writes only on request', () => {
    const onChange = vi.fn();
    render(<CeOptionsList options={[TAGS, RELOAD]} block={BASE} onChange={onChange} />);
    expect(screen.getByText('3 of 4 weapons')).toBeTruthy();
    expect(screen.getByText(/also carry CE_AI_AssaultWeapon/)).toBeTruthy();
    expect(onChange).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Take: Companion weapon tags' }));
    expect(onChange).toHaveBeenLastCalledWith({
      extraTags: ['CE_AI_AssaultWeapon', 'CE_Rifle'],
    });
    fireEvent.click(screen.getByRole('button', { name: 'Take: Reload one round at a time' }));
    expect(onChange).toHaveBeenLastCalledWith({ reloadOneAtATime: true });
  });

  it('merges tags with the ones already held and disables a suggestion that is taken', () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <CeOptionsList
        options={[TAGS]}
        block={{ ...BASE, extraTags: ['CE_Rifle'] }}
        onChange={onChange}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Take: Companion weapon tags' }));
    expect(onChange).toHaveBeenLastCalledWith({ extraTags: ['CE_Rifle', 'CE_AI_AssaultWeapon'] });
    rerender(
      <CeOptionsList
        options={[TAGS]}
        block={{ ...BASE, extraTags: ['CE_AI_AssaultWeapon', 'CE_Rifle'] }}
        onChange={onChange}
      />,
    );
    expect(
      (screen.getByRole('button', { name: 'Take: Companion weapon tags' }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });
});
