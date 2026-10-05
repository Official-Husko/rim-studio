import { screen, within } from '@testing-library/preact';
import type { ArchetypeProposalDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { CompareTable } from './CompareTable';

describe('CompareTable', () => {
  it('puts the typical value and range of the class next to each number', () => {
    const report = fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper').fit;
    if (!report) throw new Error('no fit');
    renderWithProviders(<CompareTable report={report} />);
    const table = screen.getByRole('grid', { name: 'Compared with your install' });
    const rows = within(table).getAllByRole('row');
    expect(rows.length).toBe(report.perStat.length + 1);
    const damage = rows.find((r) => within(r).queryByText('Damage'));
    expect(damage && within(damage).getByText('28')).toBeTruthy();
    expect(within(table).getByRole('columnheader', { name: 'Typical' })).toBeTruthy();
  });
});
