import { screen, within } from '@testing-library/preact';
import type { ArchetypeProposalDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { ProposalGroup } from './ProposalGroup';
import { groupRows } from './wizard-rows';

describe('ProposalGroup', () => {
  it('shows each number with the derived chip and its plain reason', () => {
    const proposal = fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper');
    const group = groupRows(proposal, undefined).find((g) => g.id === 'damage');
    if (!group) throw new Error('no group');
    renderWithProviders(<ProposalGroup group={group} melee={false} />);
    const list = screen.getByRole('list', { name: 'Damage and penetration' });
    expect(within(list).getAllByText('Derived')).toHaveLength(2);
    expect(within(list).getByText(/The install's median is 12/)).toBeTruthy();
    expect(within(list).getByText('28')).toBeTruthy();
  });

  it('says when a number is not written and when the user typed it', () => {
    const proposal = fixture<ArchetypeProposalDto>('designer-wizard-propose-thin');
    const group = groupRows(proposal, undefined).find((g) => g.id === 'damage');
    if (!group) throw new Error('no group');
    const rows = group.rows.map((r) =>
      r.key === '/ranged/damage' ? { ...r, yours: 33, value: { ...r.value, locked: true } } : r,
    );
    renderWithProviders(<ProposalGroup group={{ ...group, rows }} melee={false} />);
    expect(
      screen.getByText(/You typed this number, so it stays\. The proposal would give 26\./),
    ).toBeTruthy();
    expect(screen.getByText('33')).toBeTruthy();
    expect(screen.getByText(/Not written to the file\./)).toBeTruthy();
  });

  it('lists the cost list, the stuff and the price with the weight group', () => {
    const proposal = fixture<ArchetypeProposalDto>('designer-wizard-propose-sword');
    const group = groupRows(proposal, undefined).find((g) => g.id === 'weight');
    if (!group) throw new Error('no group');
    renderWithProviders(<ProposalGroup group={group} melee extras={proposal} />);
    expect(screen.getByText('Made from Metallic')).toBeTruthy();
    expect(screen.getByText('Market value')).toBeTruthy();
    expect(screen.getByText(/It is not written/)).toBeTruthy();
  });

  it('lists the cost list of a gun', () => {
    const proposal = fixture<ArchetypeProposalDto>('designer-wizard-propose-sniper');
    const group = groupRows(proposal, undefined).find((g) => g.id === 'weight');
    if (!group) throw new Error('no group');
    renderWithProviders(<ProposalGroup group={group} melee={false} extras={proposal} />);
    expect(screen.getByText('Steel')).toBeTruthy();
    expect(screen.getByText('x 64')).toBeTruthy();
  });
});
