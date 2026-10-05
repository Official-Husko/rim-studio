import { fireEvent, screen, within } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import type { ProjectLayoutCheckDto, ProjectSummaryDto, ProjectTreeDto } from 'rimstudio-ipc-types';
import { ProjectHeader } from './ProjectHeader';
import type { ProjectView } from './store';

function viewOf(name: 'gewehr' | 'lonewolf'): ProjectView {
  return {
    summary: loadFixture<ProjectSummaryDto>(`project-open-${name}`),
    tree: loadFixture<ProjectTreeDto>(`project-tree-${name}`),
    check: loadFixture<ProjectLayoutCheckDto>(`layout-check-${name}`),
  };
}

const handlers = { onRefresh: vi.fn(), onOpen: vi.fn(), onNew: vi.fn(), onClose: vi.fn() };

describe('ProjectHeader', () => {
  it('shows the name, the package id, the folder and the status of a game style mod', () => {
    renderWithProviders(<ProjectHeader view={viewOf('gewehr')} refreshing={false} {...handlers} />);
    expect(screen.getByRole('heading', { name: "Huskos's Gewehr 41" })).toBeTruthy();
    expect(screen.getByText('oh.weapons.gewehr41')).toBeTruthy();
    const status = screen.getByRole('list', { name: 'Status of the mod' });
    expect(within(status).getByText('1.4')).toBeTruthy();
    expect(within(status).getByText('Game style')).toBeTruthy();
    expect(within(status).getByText('3 layout issues')).toBeTruthy();
    expect(within(status).getByText('12 folders, 27 files, 10.9 MiB')).toBeTruthy();
  });

  it('shows both versions and a flat layout', () => {
    renderWithProviders(
      <ProjectHeader view={viewOf('lonewolf')} refreshing={false} {...handlers} />,
    );
    const status = screen.getByRole('list', { name: 'Status of the mod' });
    expect(within(status).getByText('1.2')).toBeTruthy();
    expect(within(status).getByText('1.3')).toBeTruthy();
    expect(within(status).getByText('Flat')).toBeTruthy();
  });

  it('says how LoadFolders.xml is used', () => {
    const view = viewOf('gewehr');
    view.summary.hasLoadFolders = true;
    view.summary.hasCeGate = true;
    renderWithProviders(<ProjectHeader view={view} refreshing={false} {...handlers} />);
    expect(
      screen.getByText('LoadFolders.xml: Present, gates a Combat Extended folder'),
    ).toBeTruthy();
  });

  it('says the layout is fine when the check found nothing', () => {
    const view = viewOf('gewehr');
    view.check = { ...view.check, issues: [] };
    renderWithProviders(<ProjectHeader view={view} refreshing={false} {...handlers} />);
    expect(screen.getByText('Layout ok')).toBeTruthy();
  });

  it('calls the four actions', () => {
    renderWithProviders(<ProjectHeader view={viewOf('gewehr')} refreshing={false} {...handlers} />);
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));
    fireEvent.click(screen.getByRole('button', { name: 'Open another' }));
    fireEvent.click(screen.getByRole('button', { name: 'New mod' }));
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(handlers.onRefresh).toHaveBeenCalled();
    expect(handlers.onOpen).toHaveBeenCalled();
    expect(handlers.onNew).toHaveBeenCalled();
    expect(handlers.onClose).toHaveBeenCalled();
  });

  it('shows the diagnostics of the summary', () => {
    const view = viewOf('gewehr');
    view.summary.diagnostics = [
      { code: 'x.y', severity: 'warning', message: 'About.xml has no description' },
    ];
    renderWithProviders(<ProjectHeader view={view} refreshing={false} {...handlers} />);
    expect(screen.getByText('About.xml has no description')).toBeTruthy();
  });
});
