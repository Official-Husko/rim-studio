import { screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { ProjectTreeDto } from 'rimstudio-ipc-types';
import { FolderSummary } from './FolderSummary';
import { findNode, issuesUnder } from './model';

const tree = loadFixture<ProjectTreeDto>('project-tree-gewehr');

describe('FolderSummary', () => {
  it('shows the role, size and the issues of a folder', () => {
    const node = findNode(tree.root, 'Patches');
    if (!node) throw new Error('fixture has no Patches folder');
    renderWithProviders(<FolderSummary node={node} issues={issuesUnder(tree.issues, 'Patches')} />);
    expect(screen.getByText('Patches', { selector: 'dd *' })).toBeTruthy();
    expect(screen.getByText('8.9 KiB')).toBeTruthy();
    expect(screen.getByText('1 layout issue on or below this folder')).toBeTruthy();
  });

  it('shows the folder name for the root', () => {
    renderWithProviders(<FolderSummary node={tree.root} issues={[]} />);
    expect(screen.getByText('[OH] Gewehr 41')).toBeTruthy();
  });
});
