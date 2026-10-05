import { fireEvent, screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { DraftDto, PreviewDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { EstimatePanel } from './EstimatePanel';

const melee = (): DraftDto => fixture<DraftDto>('designer-draft-melee');
const noop = () => 0;

describe('EstimatePanel', () => {
  it('sets the strength choice of simple mode in the draft answers', () => {
    const onChange = vi.fn();
    const draft = { ...melee(), calibration: 'simple' as const };
    renderWithProviders(
      <EstimatePanel
        draft={draft}
        estimate={undefined}
        onChange={onChange}
        onFill={noop}
        onStartQuiz={() => undefined}
      />,
    );
    expect(screen.getByRole('radio', { name: 'Typical' }).getAttribute('aria-checked')).toBe(
      'true',
    );
    fireEvent.click(screen.getByRole('radio', { name: 'Stronger' }));
    expect(onChange.mock.calls[0]?.[0].answers.strength).toEqual({
      kind: 'choice',
      choice: 'stronger',
    });
  });

  it('switches to the calibrate mode and starts the quiz', () => {
    const onChange = vi.fn();
    const onStart = vi.fn();
    const { rerender } = renderWithProviders(
      <EstimatePanel
        draft={{ ...melee(), calibration: 'simple' }}
        estimate={undefined}
        onChange={onChange}
        onFill={noop}
        onStartQuiz={onStart}
      />,
    );
    fireEvent.click(screen.getByRole('radio', { name: 'Calibrate' }));
    expect(onChange.mock.calls[0]?.[0].calibration).toBe('quiz');
    rerender(
      <EstimatePanel
        draft={melee()}
        estimate={undefined}
        onChange={onChange}
        onFill={noop}
        onStartQuiz={onStart}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Start the quiz' }));
    expect(onStart).toHaveBeenCalled();
  });

  it('counts the answers given and offers to continue', () => {
    const draft = {
      ...melee(),
      answers: {
        tier: { seq: 0, answer: { kind: 'tier', tier: 1 } },
        role: { seq: 1, answer: { kind: 'role', role: 'blade' } },
      },
    };
    renderWithProviders(
      <EstimatePanel
        draft={draft}
        estimate={undefined}
        onChange={() => undefined}
        onFill={noop}
        onStartQuiz={() => undefined}
      />,
    );
    expect(screen.getByText('2 answers given')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Continue the quiz' })).toBeTruthy();
  });

  it('fills the empty fields and says how many were filled', () => {
    const preview = fixture<PreviewDto>('designer-preview-melee');
    renderWithProviders(
      <EstimatePanel
        draft={melee()}
        estimate={preview.estimate}
        onChange={() => undefined}
        onFill={() => 3}
        onStartQuiz={() => undefined}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Fill empty fields from the estimate' }));
    expect(screen.getByRole('status').textContent).toBe('Filled 3 fields.');
  });

  it('shows the anchor instead of a mode choice for a clone', () => {
    const draft = {
      ...melee(),
      calibration: 'anchored' as const,
      clonedFrom: 'MeleeWeapon_Gladius',
    };
    renderWithProviders(
      <EstimatePanel
        draft={draft}
        estimate={undefined}
        onChange={() => undefined}
        onFill={noop}
        onStartQuiz={() => undefined}
      />,
    );
    expect(screen.getByText(/anchored to MeleeWeapon_Gladius/)).toBeTruthy();
    expect(screen.queryByRole('radio', { name: 'Calibrate' })).toBeNull();
  });
});
