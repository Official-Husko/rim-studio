import { fireEvent, screen, waitFor } from '@testing-library/preact';
import { beforeEach, describe, expect, it } from 'vitest';
import { createMockTransport, renderWithProviders } from 'rimstudio-testkit';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import { FolderPickerHost } from '~/shared/platform';
import SetupPage from './SetupPage';

beforeEach(() => {
  clearQueries();
  setTransport(createMockTransport());
  connection.value = 'mock';
});

describe('SetupPage', () => {
  it('shows the bridge information from the info route', async () => {
    renderWithProviders(<SetupPage />);
    await waitFor(() => expect(screen.getByText('/home/mock')).toBeTruthy());
    expect(screen.getByText('0.1.0')).toBeTruthy();
    expect(screen.getByText(/No bridge is running/)).toBeTruthy();
  });

  it('shows an error card when the info call fails', async () => {
    setTransport(createMockTransport({ handlers: {} }));
    const t = createMockTransport();
    t.dev = () => Promise.reject({ code: 'ipc.transport', message: 'offline', errorId: 'e' });
    setTransport(t);
    renderWithProviders(<SetupPage />);
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('offline'));
  });

  it('opens the folder browser and reports the chosen folder', async () => {
    renderWithProviders(
      <>
        <SetupPage />
        <FolderPickerHost />
      </>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'Choose a folder' }));
    await waitFor(() => expect(screen.getByRole('dialog', { hidden: true })).toBeTruthy());
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Choose this folder' })).toBeTruthy(),
    );
    await waitFor(() =>
      expect(
        (screen.getByRole('button', { name: 'Choose this folder' }) as HTMLButtonElement).disabled,
      ).toBe(false),
    );
    fireEvent.click(screen.getByRole('button', { name: 'Choose this folder' }));
    await waitFor(() => expect(screen.getByText('Chosen: /home/mock')).toBeTruthy());
  });
});
