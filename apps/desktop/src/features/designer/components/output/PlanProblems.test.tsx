import { fireEvent, screen } from '@testing-library/preact';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { problemsOf } from '../../output-model';
import { fixture } from '../../testSupport';
import { PlanProblems } from './PlanProblems';

describe('PlanProblems', () => {
  it('lists the errors first and counts them', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-nothing-accepted');
    renderWithProviders(<PlanProblems problems={problemsOf(plan)} onGoTo={() => {}} />);
    expect(screen.getByText('7 errors')).toBeTruthy();
    const items = screen.getAllByRole('listitem');
    expect(items[0]?.textContent).toContain('CE bulk is required');
    expect(screen.getAllByText('design.required-missing').length).toBeGreaterThan(0);
  });

  it('links a problem back to its field', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-nothing-accepted');
    const onGoTo = vi.fn();
    renderWithProviders(<PlanProblems problems={problemsOf(plan)} onGoTo={onGoTo} />);
    fireEvent.click(screen.getAllByRole('button', { name: 'Go to field' })[0] as HTMLElement);
    expect(onGoTo).toHaveBeenCalledWith(expect.stringMatching(/^\//));
  });

  it('says so when nothing is wrong', () => {
    renderWithProviders(<PlanProblems problems={[]} onGoTo={() => {}} />);
    expect(screen.getByText('No errors')).toBeTruthy();
    expect(screen.getByText('Nothing to report.')).toBeTruthy();
  });
});
