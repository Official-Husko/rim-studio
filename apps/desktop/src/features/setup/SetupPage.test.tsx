import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { renderWithProviders } from 'rimstudio-testkit';
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
    expect(screen.getByText('78,162')).toBeTruthy();
    expect(transport.calls.some((c) => c.name === 'library_scan')).toBe(true);
  });

  it('finds Combat Extended in the workshop folder', async () => {
    installTransport({
      sources_probe_folder: () => ({
        kind: 'single-mod',
        modCount: 1,
        suggestedDepth: 1,
        suggestedLayout: 'single-mod',
        warnings: [],
        overlaps: [],
        diagnostics: [],
        canSave: false,
      }),
    });
    renderWithProviders(<SetupPage />);
    await screen.findByText('In your library');
    expect(screen.getByText('ceteam.combatextended')).toBeTruthy();
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
