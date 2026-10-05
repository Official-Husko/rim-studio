import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { StepHeader } from './StepHeader';
import { WIZARD_STEPS } from './wizard-model';

describe('StepHeader', () => {
  it('marks the current step and opens a reachable one', () => {
    const onGo = vi.fn();
    renderWithProviders(
      <StepHeader
        steps={WIZARD_STEPS}
        current="describe"
        reachable={(s) => s !== 'name'}
        onGo={onGo}
      />,
    );
    expect(screen.getByRole('button', { name: /Describe/ }).getAttribute('aria-current')).toBe(
      'step',
    );
    fireEvent.click(screen.getByRole('button', { name: /Type/ }));
    expect(onGo).toHaveBeenCalledWith('category');
    expect((screen.getByRole('button', { name: /Name/ }) as HTMLButtonElement).disabled).toBe(true);
  });
});
