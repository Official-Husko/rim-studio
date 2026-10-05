import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import type { LibraryScanResult } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import SetupPage from './SetupPage';
import { installTransport } from './testSupport';

describe('SetupPage', () => {
  it('shows the first run state when detection has not run', async () => {
    installTransport({ detect_get_report: () => ({}) });
    renderWithProviders(<SetupPage />);
    await screen.findByText('Detection has not run yet');
    expect(screen.getByRole('button', { name: 'Detect again' })).toBeTruthy();
  });

  it('shows the detection report with the plain warning text', async () => {
    installTransport({ dev: undefined as never });
    renderWithProviders(<SetupPage />);
    await screen.findByText('1.6.4871 rev598');
    expect(screen.getByText(/Steam left temporary copies/)).toBeTruthy();
    expect(screen.getByText('High confidence')).toBeTruthy();
    expect(screen.getByText(/691 items on disk, 693 listed by Steam/)).toBeTruthy();
  });

  it('runs detection and a scan from the buttons', async () => {
    const transport = installTransport();
    renderWithProviders(<SetupPage />);
    await screen.findByText('1.6.4871 rev598');
    fireEvent.click(screen.getByRole('button', { name: 'Scan' }));
    await screen.findByText('Diagnostics');
    await waitFor(() => expect(screen.getByText('loadfolders.ignored-attribute')).toBeTruthy());
    expect(screen.getByText('78,163')).toBeTruthy();
    expect(transport.calls.some((c) => c.name === 'library_scan')).toBe(true);
  });

  it('asks for a scan before it says anything about Combat Extended', async () => {
    const transport = installTransport();
    renderWithProviders(<SetupPage />);
    await screen.findByText('Scan the library to find out whether Combat Extended is in it.');
    expect(transport.calls.some((c) => c.name === 'sources_probe_folder')).toBe(false);
  });

  it('reads Combat Extended, the counts and the duplicates from the scan', async () => {
    installTransport({ library_scan: () => loadFixture('library-scan-with-custom') });
    renderWithProviders(<SetupPage />);
    await screen.findByText('1.6.4871 rev598');
    fireEvent.click(screen.getByRole('button', { name: 'Scan' }));
    await screen.findByText('In your library');
    expect(screen.getByText('16.7.3.0')).toBeTruthy();
    expect(screen.getByText('CETeam.CombatExtended')).toBeTruthy();
    expect(screen.getByText('19 mods exist only in your own folders')).toBeTruthy();
    expect(screen.getByRole('table', { name: 'Duplicate package ids' })).toBeTruthy();
    expect(screen.getByText('Husko.ATR')).toBeTruthy();
  });

  it('says Combat Extended is missing when the scan does not find it', async () => {
    const scan = loadFixture<LibraryScanResult>('library-scan');
    installTransport({ library_scan: () => ({ ...scan, ceInLibrary: { present: false } }) });
    renderWithProviders(<SetupPage />);
    await screen.findByText('1.6.4871 rev598');
    fireEvent.click(screen.getByRole('button', { name: 'Scan' }));
    await screen.findByText('Combat Extended was not found');
  });

  it('clears an override only when one is active', async () => {
    installTransport();
    renderWithProviders(<SetupPage />);
    await screen.findByText('1.6.4871 rev598');
    for (const button of screen.getAllByRole('button', { name: /^Clear override of / })) {
      expect((button as HTMLButtonElement).disabled).toBe(true);
    }
  });
});
