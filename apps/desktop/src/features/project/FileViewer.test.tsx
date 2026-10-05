import { screen } from '@testing-library/preact';
import { loadFixture, renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import type { ProjectFileDto } from 'rimstudio-ipc-types';
import { FileViewer } from './FileViewer';

describe('FileViewer', () => {
  it('asks for a file when none is chosen', () => {
    renderWithProviders(<FileViewer file={undefined} />);
    expect(screen.getByText('No file selected')).toBeTruthy();
  });

  it('shows an xml file with its role and size', () => {
    const file = loadFixture<ProjectFileDto>('project-file-about');
    renderWithProviders(<FileViewer file={{ path: file.path, loading: false, file }} />);
    expect(screen.getByRole('region', { name: 'Contents of About/About.xml' })).toBeTruthy();
    expect(screen.getByText('About')).toBeTruthy();
    expect(screen.getByText('542 B')).toBeTruthy();
  });

  it('says when only the start of a file is shown', () => {
    const file = loadFixture<ProjectFileDto>('project-file-weapons');
    renderWithProviders(<FileViewer file={{ path: file.path, loading: false, file }} />);
    expect(screen.getByText(/Showing the first/)).toBeTruthy();
    expect(screen.getByText('Weapon defs')).toBeTruthy();
  });

  it('explains a binary file instead of showing it', () => {
    const file = loadFixture<ProjectFileDto>('project-file-binary');
    renderWithProviders(<FileViewer file={{ path: file.path, loading: false, file }} />);
    expect(screen.getByText('This file is not text')).toBeTruthy();
    expect(screen.queryByRole('region')).toBeNull();
  });

  it('shows the loading and the error states', () => {
    const { rerender } = renderWithProviders(
      <FileViewer file={{ path: 'a.xml', loading: true }} />,
    );
    expect(screen.getByRole('status', { name: 'Reading the file' })).toBeTruthy();
    rerender(
      <FileViewer
        file={{
          path: 'a.xml',
          loading: false,
          error: { code: 'io.not-found', message: 'missing', errorId: 'e-1' },
        }}
      />,
    );
    expect(screen.getByText('missing')).toBeTruthy();
  });
});
