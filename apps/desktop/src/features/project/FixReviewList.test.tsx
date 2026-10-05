import { screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { ProjectLayoutFixPlanDto } from 'rimstudio-ipc-types';
import { FixReviewList } from './FixReviewList';

const plan = loadFixture<ProjectLayoutFixPlanDto>('layout-fix-plan-lombax');

describe('FixReviewList', () => {
  it('lists the items for review with their reasons', () => {
    const items = plan.items.filter((i) => !i.applicable);
    renderWithProviders(<FixReviewList items={items} />);
    expect(screen.getAllByText('Needs review').length).toBeGreaterThan(1);
    expect(screen.getAllByText(/belongs to the game version folder 1.2/).length).toBe(2);
    expect(screen.getByText(/moving one of them means splitting the file/)).toBeTruthy();
  });

  it('shows the references found, cut after a few', () => {
    const base = plan.items.find((i) => !i.applicable);
    if (!base) throw new Error('fixture has a review item');
    const references = Array.from({ length: 7 }, (_, n) => ({
      path: 'Defs/a.xml',
      line: n + 1,
      text: `<texPath>old${n}</texPath>`,
    }));
    renderWithProviders(<FixReviewList items={[{ ...base, references }]} />);
    expect(screen.getByText('7 places mention the old path')).toBeTruthy();
    expect(screen.getByText('and 2 more')).toBeTruthy();
  });

  it('renders nothing for an empty list', () => {
    const { container } = renderWithProviders(<FixReviewList items={[]} />);
    expect(container.textContent).toBe('');
  });
});
