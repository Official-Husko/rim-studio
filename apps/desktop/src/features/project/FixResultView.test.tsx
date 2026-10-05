import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import type { ProjectLayoutFixApplyDto } from 'rimstudio-ipc-types';
import { resetFixFlow, undoFlow } from './fixStore';
import { FixResultView } from './FixResultView';
import { gewehrRef, installWithProject } from './testSupport';
import { loadProject } from './store';

const result = loadFixture<ProjectLayoutFixApplyDto>('layout-fix-apply-gewehr');

beforeEach(async () => {
  installWithProject();
  resetFixFlow();
  await loadProject(gewehrRef());
});

describe('FixResultView', () => {
  it('lists what was carried out, the file edited and the check afterwards', () => {
    renderWithProviders(<FixResultView result={result} />);
    expect(screen.getByText('4 changes carried out')).toBeTruthy();
    expect(screen.getByText('1 file edited or created')).toBeTruthy();
    expect(
      screen.getByText('Patches/ce_patch.xml → Compat/CombatExtended/Patches/ce_patch.xml'),
    ).toBeTruthy();
    expect(screen.getByText('The layout check finds nothing now.')).toBeTruthy();
    expect(screen.getByText(`Undo journal ${result.applyId}`)).toBeTruthy();
  });

  it('lists skipped items with their reasons and a stopped apply', () => {
    renderWithProviders(
      <FixResultView
        result={{
          ...result,
          cancelled: true,
          skipped: [{ id: 'fix-009-aaaaaa', reason: 'the destination exists' }],
        }}
      />,
    );
    expect(screen.getByText('1 item skipped')).toBeTruthy();
    expect(screen.getByText('the destination exists')).toBeTruthy();
    expect(screen.getByText(/stopped between items/)).toBeTruthy();
  });

  it('undoes the apply and says what went back', async () => {
    renderWithProviders(<FixResultView result={result} />);
    fireEvent.click(screen.getByRole('button', { name: 'Undo this fix' }));
    await waitFor(() => expect(undoFlow.value?.result).toBeDefined());
    expect(await screen.findByText(/1 moved back, 1 restored, 5 folders removed/)).toBeTruthy();
  });

  it('shows why an undo was refused', async () => {
    installWithProject({
      project_layout_fix_undo: () => {
        throw {
          code: 'project.fix-undo-refused',
          message: 'no longer what was moved',
          errorId: 'e',
        };
      },
    });
    await loadProject(gewehrRef());
    renderWithProviders(<FixResultView result={result} />);
    fireEvent.click(screen.getByRole('button', { name: 'Undo this fix' }));
    expect(await screen.findByText('no longer what was moved')).toBeTruthy();
  });
});
