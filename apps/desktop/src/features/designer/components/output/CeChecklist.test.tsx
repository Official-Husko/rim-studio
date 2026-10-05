import { fireEvent, screen } from '@testing-library/preact';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { pendingAnswers } from '../../output-model';
import { fixture } from '../../testSupport';
import { CeChecklist } from './CeChecklist';

describe('CeChecklist', () => {
  it('lists every answer the plan waits for, once, and jumps to it', () => {
    const plan = fixture<WritePlanDto>('designer-output-plan-ce-needs-answer');
    const onGoTo = vi.fn();
    const labels = new Map([['/ce/ammoSet', 'Which caliber (ammo set) does the weapon use?']]);
    renderWithProviders(
      <CeChecklist pending={pendingAnswers(plan)} labels={labels} onGoTo={onGoTo} />,
    );
    expect(screen.getByText('5 answers still needed')).toBeTruthy();
    expect(screen.getByText('Which caliber (ammo set) does the weapon use?')).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Go to' })[0] as HTMLElement);
    expect(onGoTo).toHaveBeenCalledWith('/ce/ammoSet');
  });

  it('says so when everything is answered', () => {
    renderWithProviders(<CeChecklist pending={[]} labels={new Map()} onGoTo={() => {}} />);
    expect(screen.getByText('Every Combat Extended question is answered.')).toBeTruthy();
  });
});
