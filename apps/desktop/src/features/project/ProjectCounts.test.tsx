import { screen, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { ProjectTreeDto } from 'rimstudio-ipc-types';
import { ProjectCounts } from './ProjectCounts';

describe('ProjectCounts', () => {
  it('shows every count of the project', () => {
    const { counts } = loadFixture<ProjectTreeDto>('project-tree-lonewolf');
    renderWithProviders(<ProjectCounts counts={counts} />);
    const list = screen.getByRole('list', { name: 'Contents' });
    expect(within(list).getAllByRole('listitem')).toHaveLength(6);
    expect(within(list).getByText('Textures').previousSibling?.textContent).toBe('44');
    expect(within(list).getByText('Sounds').previousSibling?.textContent).toBe('6');
  });
});
