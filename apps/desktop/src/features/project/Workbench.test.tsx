import { fireEvent, screen, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { beforeEach, describe, expect, it } from 'vitest';
import type { ProjectLayoutCheckDto, ProjectSummaryDto, ProjectTreeDto } from 'rimstudio-ipc-types';
import { Workbench } from './Workbench';
import { loadProject, view } from './store';
import { gewehrRef, installTransport } from './testSupport';

beforeEach(async () => {
  installTransport();
  await loadProject(gewehrRef());
});

function current() {
  const shown = view.value;
  if (!shown) throw new Error('project not loaded');
  return shown;
}

describe('Workbench', () => {
  it('starts on the file tab with nothing chosen', () => {
    renderWithProviders(<Workbench view={current()} />);
    expect(screen.getByRole('tab', { name: 'File', selected: true })).toBeTruthy();
    expect(screen.getByText('No file selected')).toBeTruthy();
  });

  it('shows what a folder holds when it is chosen', () => {
    renderWithProviders(<Workbench view={current()} />);
    const tree = screen.getByRole('tree', { name: 'Project folders' });
    fireEvent.click(within(tree).getByRole('treeitem', { name: /Patches/ }));
    expect(screen.getByText('1 layout issue on or below this folder')).toBeTruthy();
  });

  it('counts the issues on the layout tab and opens the guide', () => {
    renderWithProviders(<Workbench view={current()} />);
    expect(screen.getByRole('tab', { name: /^Layout 3/ })).toBeTruthy();
    fireEvent.click(screen.getByRole('tab', { name: 'Layout guide' }));
    expect(screen.getByText(/The RimStudio layout keeps the names/)).toBeTruthy();
  });

  it('opens the file of an issue in the viewer', async () => {
    renderWithProviders(<Workbench view={current()} />);
    fireEvent.click(screen.getByRole('tab', { name: /^Layout 3/ }));
    fireEvent.click(screen.getByRole('button', { name: 'Patches/ce_patch.xml' }));
    expect(await screen.findByRole('tab', { name: 'File', selected: true })).toBeTruthy();
    expect(
      await screen.findByRole('region', { name: 'Contents of Patches/ce_patch.xml' }),
    ).toBeTruthy();
  });

  it('works for a project without issues', () => {
    renderWithProviders(
      <Workbench
        view={{
          summary: loadFixture<ProjectSummaryDto>('project-create-new'),
          tree: loadFixture<ProjectTreeDto>('project-tree-new'),
          check: loadFixture<ProjectLayoutCheckDto>('layout-check-new'),
        }}
      />,
    );
    expect(screen.getByRole('tab', { name: 'Layout' })).toBeTruthy();
  });
});
