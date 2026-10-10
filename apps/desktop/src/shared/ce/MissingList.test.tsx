import { fireEvent, render, screen, within } from '@testing-library/preact';
import { describe, expect, it, vi } from 'vitest';
import { MissingList } from './MissingList';

const ITEMS = [
  {
    key: '/ce/ammoSet',
    label: 'Which ammo set does the weapon use?',
    why: 'It is never chosen for you.',
  },
  { key: '/ce/weaponTagClass', label: 'Which weapon class tag does the weapon get?' },
];

describe('MissingList', () => {
  it('lists what is missing with its reason and a shortcut per row', () => {
    const onGoTo = vi.fn();
    render(
      <MissingList
        title="2 answers are missing"
        items={ITEMS}
        goLabel="Answer"
        onGoTo={onGoTo}
        footer="The plan follows your answers."
      />,
    );
    const section = screen.getByRole('region', { name: '2 answers are missing' });
    expect(within(section).getAllByRole('listitem')).toHaveLength(2);
    expect(within(section).getByText('It is never chosen for you.')).toBeTruthy();
    expect(within(section).getByText('The plan follows your answers.')).toBeTruthy();
    fireEvent.click(within(section).getAllByRole('button', { name: 'Answer' })[1] as HTMLElement);
    expect(onGoTo).toHaveBeenCalledWith('/ce/weaponTagClass');
  });

  it('has no buttons without a handler and renders nothing when nothing is missing', () => {
    const { container, rerender } = render(
      <MissingList title="Missing" items={ITEMS} goLabel="Answer" />,
    );
    expect(screen.queryByRole('button')).toBeNull();
    rerender(<MissingList title="Missing" items={[]} goLabel="Answer" />);
    expect(container.textContent).toBe('');
  });
});
