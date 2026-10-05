import { fireEvent, screen } from '@testing-library/preact';
import type { CeSuggestedFieldDto, CeSuggestionDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { fixture } from '../../testSupport';
import { CeFieldRow } from './CeFieldRow';

const field = (name: string): CeSuggestedFieldDto => {
  const found = fixture<CeSuggestionDto>('designer-output-suggest-on').fields.find(
    (f) => f.field === name,
  );
  if (!found) throw new Error(name);
  return found;
};

function setup(f: CeSuggestedFieldDto, accepted = false) {
  const onAccept = vi.fn();
  const onValue = vi.fn();
  renderWithProviders(
    <CeFieldRow field={f} accepted={accepted} onAccept={onAccept} onValue={onValue} />,
  );
  return { onAccept, onValue };
}

describe('CeFieldRow', () => {
  it('shows a derived value as a hint with its rating, source and band, and does not write it', () => {
    const { onValue } = setup(field('/ce/bulk'));
    expect(screen.getByText('Rough')).toBeTruthy();
    expect(screen.getByText('Derived')).toBeTruthy();
    expect(screen.getByText('predicted from 4 converted weapons')).toBeTruthy();
    expect(screen.getByText(/Likely 5.616 to 10.752, wide 4.538 to 13.307/)).toBeTruthy();
    const input = screen.getByRole('spinbutton', { name: /CE bulk/ }) as HTMLInputElement;
    expect(input.value).toBe('');
    expect(input.placeholder).toBe('7.771');
    expect(onValue).not.toHaveBeenCalled();
  });

  it('takes the derived value only when the box is ticked', () => {
    const { onAccept } = setup(field('/ce/bulk'));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Use 7.771' }));
    expect(onAccept).toHaveBeenCalledWith(true);
  });

  it('records a typed number as typed', () => {
    const { onValue } = setup(field('/ce/bulk'));
    const input = screen.getByRole('spinbutton', { name: /CE bulk/ });
    fireEvent.input(input, { target: { value: '8.5' } });
    fireEvent.blur(input);
    expect(onValue).toHaveBeenCalledWith(8.5, 'typed');
  });

  it('says why an unreliable number is asked and offers the estimate as an answer', () => {
    const { onValue } = setup(field('/ce/shotSpread'));
    expect(screen.getByText('Unreliable')).toBeTruthy();
    expect(
      (screen.getByRole('spinbutton', { name: /CE shot spread/ }) as HTMLInputElement).placeholder,
    ).toBe('');
    expect(screen.getByText(/is not written: it is rated unreliable/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Use estimate 0.165' }));
    expect(onValue).toHaveBeenCalledWith(0.165, 'answered');
  });

  it('shows a held number with its source and clears it on request', () => {
    const held: CeSuggestedFieldDto = {
      ...field('/ce/shotSpread'),
      status: 'held',
      held: 0.2,
      heldSource: { kind: 'answered' },
    };
    const { onValue } = setup(held);
    expect(
      (screen.getByRole('spinbutton', { name: /CE shot spread/ }) as HTMLInputElement).value,
    ).toBe('0.2');
    expect(screen.getByText('your answer')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Clear' }));
    expect(onValue).toHaveBeenCalledWith(undefined, 'typed');
  });
});
