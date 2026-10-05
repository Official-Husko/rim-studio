import { fireEvent, screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectLayoutFixPlanDto } from 'rimstudio-ipc-types';
import { FixPlanView } from './FixPlanView';

const plan = loadFixture<ProjectLayoutFixPlanDto>('layout-fix-plan-lombax');

function renderPlan(p: ProjectLayoutFixPlanDto, selected: string[] = []) {
  const props = {
    onToggle: vi.fn(),
    onRename: vi.fn(),
    onTickSafe: vi.fn(),
    onTickNone: vi.fn(),
  };
  renderWithProviders(<FixPlanView plan={p} selected={selected} rename={[]} {...props} />);
  return props;
}

describe('FixPlanView', () => {
  it('counts the items and lists the applicable ones and the review ones apart', () => {
    renderPlan(plan);
    expect(screen.getByText('5 can be applied')).toBeTruthy();
    expect(screen.getByText('3 need review')).toBeTruthy();
    const list = screen.getByRole('list', { name: 'Changes to apply' });
    expect(list.querySelectorAll('li').length).toBeGreaterThanOrEqual(5);
    expect(screen.getByRole('region', { name: 'Needs review' })).toBeTruthy();
  });

  it('ticks and unticks through the buttons and the checkboxes', () => {
    const first = plan.items.find((i) => i.applicable);
    if (!first) throw new Error('fixture has an applicable item');
    const props = renderPlan(plan, [first.id]);
    fireEvent.click(screen.getByRole('button', { name: 'Tick all safe' }));
    fireEvent.click(screen.getByRole('button', { name: 'Untick all' }));
    expect(props.onTickSafe).toHaveBeenCalled();
    expect(props.onTickNone).toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole('checkbox', { name: new RegExp(first.to.replace('.', '\\.')) }),
    );
    expect(props.onToggle).toHaveBeenCalledWith(first.id, false);
  });

  it('says so when there is nothing to carry out', () => {
    renderPlan({ ...plan, items: [], safe: 0, needsReview: 0, conflicts: 0 });
    expect(screen.getByText('Nothing to carry out')).toBeTruthy();
  });
});
