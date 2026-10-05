import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { QuizStepDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { AnchorCardView } from './AnchorCardView';

describe('AnchorCardView', () => {
  it('shows the real numbers of the anchor weapon', () => {
    const step = fixture<QuizStepDto>('designer_quiz_next');
    const card = step.estimate?.anchors[0];
    if (!card) throw new Error('fixture without anchors');
    renderWithProviders(<AnchorCardView card={card} caption="Compare with" />);
    expect(screen.getByRole('heading', { name: 'longsword' })).toBeTruthy();
    expect(screen.getByText('Swing damage')).toBeTruthy();
    expect(screen.getByText('Compare with')).toBeTruthy();
  });
});
