import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createMockTransport, renderWithProviders } from 'rimstudio-testkit';
import { connection, setTransport } from '~/shared/ipc';
import { applyEvent, resetJobs } from '~/shared/ipc/jobs';
import { AppShell } from './AppShell';
import { navigate, route } from './route';
import { visibleTools } from './tools';

beforeEach(() => {
  setTransport(createMockTransport());
  connection.value = 'mock';
  route.value = 'setup';
  resetJobs();
});
afterEach(() => {
  location.hash = '';
});

describe('AppShell', () => {
  it('renders the top bar, the rail and the first page', async () => {
    renderWithProviders(<AppShell />);
    expect(screen.getByText('RimStudio')).toBeTruthy();
    const nav = screen.getByRole('navigation', { name: 'Tools' });
    for (const name of ['Setup', 'Mod', 'Weapons', 'Patches', 'Gallery']) {
      expect(nav.textContent).toContain(name);
    }
    expect(screen.getByRole('link', { name: /Setup/ }).getAttribute('aria-current')).toBe('page');
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Setup' })).toBeTruthy());
  });

  it('shows the connection status chip', () => {
    renderWithProviders(<AppShell />);
    expect(screen.getByRole('status', { name: 'Connection' }).textContent).toContain('Mock data');
  });

  it('has a project selector slot with a default and accepts a replacement', () => {
    const { unmount } = renderWithProviders(<AppShell />);
    expect(screen.getByRole('button', { name: /No project open/ })).toBeTruthy();
    unmount();
    renderWithProviders(<AppShell projectSlot={<button type="button">Plasma Carbine</button>} />);
    expect(screen.getByRole('button', { name: 'Plasma Carbine' })).toBeTruthy();
  });

  it('switches pages from the rail and keeps visited pages mounted but hidden', async () => {
    const { container } = renderWithProviders(<AppShell />);
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Setup' })).toBeTruthy());
    fireEvent.click(screen.getByRole('link', { name: /Weapons/ }));
    await waitFor(() => expect(screen.getByText('No draft open')).toBeTruthy());
    expect(route.value).toBe('weapons');
    expect(screen.getByRole('link', { name: /Weapons/ }).getAttribute('aria-current')).toBe('page');
    expect(container.querySelectorAll('main > div[hidden]').length).toBe(1);
    expect(screen.queryByRole('heading', { name: 'Setup' })).toBeNull();
    navigate('setup');
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Setup' })).toBeTruthy());
  });

  it('opens the task centre from the top bar and lists running jobs', () => {
    applyEvent({
      type: 'job-progress',
      jobId: 'j1',
      command: 'library_scan',
      message: '',
      done: 1,
      total: 4,
    });
    renderWithProviders(<AppShell />);
    const button = screen.getByRole('button', { name: /Tasks, 1 task running/ });
    fireEvent.click(button);
    expect(screen.getByRole('complementary', { name: 'Tasks' })).toBeTruthy();
    expect(screen.getByRole('progressbar', { name: 'library_scan' })).toBeTruthy();
  });

  it('offers the gallery only in development builds', () => {
    expect(visibleTools(true).map((t) => t.id)).toContain('gallery');
    expect(visibleTools(false).map((t) => t.id)).not.toContain('gallery');
  });
});
