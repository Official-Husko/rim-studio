import { screen } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { QuizStepDto } from 'rimstudio-ipc-types';
import { fixture } from '../../testSupport';
import { EstimateSummary } from './EstimateSummary';

describe('EstimateSummary', () => {
  it('shows the class, the strength and what the answers imply', () => {
    const step = fixture<QuizStepDto>('designer_quiz_answer' as never) as unknown as {
      step: QuizStepDto;
    };
    const estimate = step.step.estimate;
    if (!estimate) throw new Error('fixture without estimate');
    renderWithProviders(<EstimateSummary estimate={estimate} implied={{ tier: 'Medieval' }} />);
    expect(screen.getByText('Strength index')).toBeTruthy();
    expect(screen.getByText('Medieval')).toBeTruthy();
    expect(screen.getByText(/50 %/)).toBeTruthy();
  });
});
