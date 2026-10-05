import { fireEvent, render, screen } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto, WritePlanDto } from 'rimstudio-ipc-types';
import { describe, expect, it, vi } from 'vitest';
import { PlanView } from './PlanView';

const candidate = loadFixture<ConvertScanDto>('designer_convert_scan').candidates[0];
if (!candidate) throw new Error('fixture');
const ready = loadFixture<WritePlanDto>('patches-plan-ready');
const open = loadFixture<WritePlanDto>('patches-plan-open');

describe('PlanView', () => {
  it('shows the files, the derived numbers and the notes of a ready plan', () => {
    render(
      <PlanView
        candidate={candidate}
        entry={{ key: 'k', phase: 'ready', plan: ready }}
        onRetry={() => {}}
        onOpenQuestions={() => {}}
      />,
    );
    expect(screen.getByText('The plan is ready: 2 files will be written.')).toBeTruthy();
    expect(screen.getByRole('region', { name: 'LoadFolders.xml' })).toBeTruthy();
    expect(screen.getByRole('grid', { name: 'Derived numbers' })).toBeTruthy();
    expect(screen.getByText(/add loadAfter ceteam.combatextended/)).toBeTruthy();
    expect(screen.getByRole('button', { name: /Rules that did not run/ })).toBeTruthy();
  });

  it('points to the questions while some are open', () => {
    const onOpenQuestions = vi.fn();
    render(
      <PlanView
        candidate={candidate}
        entry={{ key: 'k', phase: 'ready', plan: open }}
        onRetry={() => {}}
        onOpenQuestions={onOpenQuestions}
      />,
    );
    expect(screen.getByText('9 questions are still open')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Go to the questions' }));
    expect(onOpenQuestions).toHaveBeenCalled();
    expect(screen.queryByRole('region', { name: 'LoadFolders.xml' })).toBeNull();
  });

  it('shows loading and error states', () => {
    const onRetry = vi.fn();
    const { rerender } = render(
      <PlanView
        candidate={candidate}
        entry={undefined}
        onRetry={onRetry}
        onOpenQuestions={() => {}}
      />,
    );
    expect(screen.getByLabelText('Building the plan')).toBeTruthy();
    rerender(
      <PlanView
        candidate={candidate}
        entry={{
          key: 'k',
          phase: 'error',
          error: { code: 'designer.internal', message: 'broke', errorId: 'e' },
        }}
        onRetry={onRetry}
        onOpenQuestions={() => {}}
      />,
    );
    expect(screen.getByText('designer.internal')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(onRetry).toHaveBeenCalled();
  });
});
