import { fireEvent, screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectLayoutFixPlanDto } from 'rimstudio-ipc-types';
import { FixItemRow } from './FixItemRow';

const plan = loadFixture<ProjectLayoutFixPlanDto>('layout-fix-plan-gewehr');
const conflictPlan = loadFixture<ProjectLayoutFixPlanDto>('layout-fix-plan-conflict');

function renderRow(item: ProjectLayoutFixPlanDto['items'][number], extra = {}) {
  const props = {
    checked: true,
    renamed: false,
    requiredPaths: [] as string[],
    onToggle: vi.fn(),
    onRename: vi.fn(),
    ...extra,
  };
  renderWithProviders(
    <ul>
      <FixItemRow item={item} {...props} />
    </ul>,
  );
  return props;
}

describe('FixItemRow', () => {
  it('shows the move with from, to, why and the risk', () => {
    const move = plan.items.find((i) => i.kind === 'move-file');
    if (!move) throw new Error('fixture has a move');
    const props = renderRow(move);
    expect(screen.getByText('Move file')).toBeTruthy();
    expect(screen.getByText(move.from)).toBeTruthy();
    expect(screen.getByText(move.why)).toBeTruthy();
    expect(screen.getByText('Safe')).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: new RegExp(`Move file ${move.to}`) }));
    expect(props.onToggle).toHaveBeenCalledWith(false);
  });

  it('shows the LoadFolders.xml edit as a diff and names what it is applied with', () => {
    const edit = plan.items.find((i) => i.diff);
    if (!edit) throw new Error('fixture has an edit');
    renderRow(edit, { requiredPaths: ['Compat/x.xml'] });
    expect(screen.getByRole('region', { name: 'Edit of LoadFolders.xml' })).toBeTruthy();
    expect(screen.getByText('Applied together with Compat/x.xml')).toBeTruthy();
  });

  it('blocks a conflicting move until the numbered name is chosen', () => {
    const move = conflictPlan.items.find((i) => i.conflict?.destinationExists);
    if (!move) throw new Error('fixture has a conflict');
    const props = renderRow(move, { checked: false });
    expect(
      screen.getByRole('checkbox', { name: new RegExp(`Move file ${move.to}`) }),
    ).toHaveProperty('disabled', true);
    fireEvent.click(
      screen.getByRole('checkbox', { name: `Use ${move.conflict?.suggestedTo} instead` }),
    );
    expect(props.onRename).toHaveBeenCalledWith(true);
  });
});
