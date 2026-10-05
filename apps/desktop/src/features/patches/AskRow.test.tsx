import { fireEvent, render, screen } from '@testing-library/preact';
import type { AskItemDto } from 'rimstudio-ipc-types';
import { describe, expect, it, vi } from 'vitest';
import { AskRow } from './AskRow';

const base: AskItemDto = {
  field: '/ce/shotSpread',
  label: 'CE shot spread',
  kind: 'number',
  options: [],
};

describe('AskRow', () => {
  it('shows the reason and the rejected estimate of a number and uses it on request', () => {
    const onChange = vi.fn();
    render(
      <AskRow
        ask={{
          ...base,
          reason: 'the estimate 0.15 is not written: it is rated unreliable',
          suggestion: 0.15,
        }}
        value={undefined}
        from={undefined}
        options={[]}
        onChange={onChange}
      />,
    );
    expect(screen.getByText(/rated unreliable/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Use 0.15' }));
    expect(onChange).toHaveBeenCalledWith(0.15);
  });

  it('drops the rejected estimate and its reason once the number is answered', () => {
    render(
      <AskRow
        ask={{ ...base, reason: 'the estimate 0.15 is not written', suggestion: 0.15 }}
        value={0.15}
        from={undefined}
        options={[]}
        onChange={vi.fn()}
      />,
    );
    expect(screen.queryByText(/is not written/)).toBeNull();
    expect(screen.queryByRole('button', { name: 'Use 0.15' })).toBeNull();
  });

  it('answers a flag with yes or no and leaves it open until then', () => {
    const onChange = vi.fn();
    render(
      <AskRow
        ask={{ ...base, field: '/ce/beltFed', label: 'Is the weapon belt fed?', kind: 'flag' }}
        value={undefined}
        from={undefined}
        options={[]}
        onChange={onChange}
      />,
    );
    const yes = screen.getByRole('radio', { name: 'Yes' });
    const no = screen.getByRole('radio', { name: 'No' });
    expect(yes.getAttribute('aria-checked')).toBe('false');
    expect(no.getAttribute('aria-checked')).toBe('false');
    fireEvent.click(no);
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('offers a choice with its hints and marks an answer that comes from the family', () => {
    render(
      <AskRow
        ask={{
          ...base,
          field: '/ce/ammoSet',
          label: 'Which caliber?',
          kind: 'choice',
          options: ['A', 'B'],
        }}
        value="A"
        from="family"
        options={[
          { value: 'A', label: 'A', hint: 'used by 2 converted weapons' },
          { value: 'B', label: 'B' },
        ]}
        rankNote="The 1 choices at the top are ranked."
        onChange={() => {}}
      />,
    );
    expect(
      (screen.getByRole('combobox', { name: 'Which caliber?' }) as HTMLInputElement).value,
    ).toBe('A');
    expect(screen.getByText('Shared with the family')).toBeTruthy();
    expect(screen.getByText('The 1 choices at the top are ranked.')).toBeTruthy();
  });
});
