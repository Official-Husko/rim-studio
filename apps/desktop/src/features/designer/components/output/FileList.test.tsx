import { fireEvent, screen } from '@testing-library/preact';
import type { WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it, vi } from 'vitest';
import { fixture } from '../../testSupport';
import { FileList } from './FileList';

const plan = () => fixture<WritePlanDto>('designer-output-plan-ce-ready');

describe('FileList', () => {
  it('shows the path split into folder and name, the action, the role and the size', () => {
    renderWithProviders(<FileList files={plan().files} selected={undefined} onSelect={() => {}} />);
    expect(screen.getByText('Gun_OutRifle.xml')).toBeTruthy();
    expect(screen.getByText('Defs/ThingDefs_Misc/Weapons/RangedIndustrial')).toBeTruthy();
    expect(screen.getByText('Weapon definitions')).toBeTruthy();
    expect(screen.getByText('Combat Extended patch')).toBeTruthy();
    expect(screen.getByText('Load folders')).toBeTruthy();
    expect(screen.getByText('Project root')).toBeTruthy();
    expect(screen.getAllByText('New')).toHaveLength(3);
  });

  it('marks the selected file and reports a click', () => {
    const onSelect = vi.fn();
    const files = plan().files;
    renderWithProviders(<FileList files={files} selected={files[1]?.path} onSelect={onSelect} />);
    const buttons = screen.getAllByRole('button');
    expect(buttons[1]?.getAttribute('aria-pressed')).toBe('true');
    expect(buttons[0]?.getAttribute('aria-pressed')).toBe('false');
    fireEvent.click(buttons[2] as HTMLElement);
    expect(onSelect).toHaveBeenCalledWith('LoadFolders.xml');
  });

  it('says what happens to an unchanged and to an updated file', () => {
    const files = fixture<WritePlanDto>('designer-output-plan-ce-update').files;
    renderWithProviders(<FileList files={files} selected={undefined} onSelect={() => {}} />);
    expect(screen.getByText('Update')).toBeTruthy();
    expect(screen.getAllByText('Unchanged')).toHaveLength(2);
  });
});

describe('FileList with copied files', () => {
  const files = () => fixture<WritePlanDto>('designer-assets-plan-imports').files;

  it('names a texture, a clip and the sound definitions, and shows the source with a hash start', () => {
    renderWithProviders(<FileList files={files()} selected={undefined} onSelect={() => {}} />);
    expect(screen.getAllByText('Texture')).toHaveLength(2);
    expect(screen.getAllByText('Sound clip')).toHaveLength(2);
    expect(screen.getByText('Sound definitions')).toBeTruthy();
    expect(
      screen.getByText('From /home/user/Art/TLWWP_Eagle_Carbine.png (SHA-256 6865c3284585)'),
    ).toBeTruthy();
  });

  it('shows the thumbnail of a texture copy when it is known', () => {
    renderWithProviders(
      <FileList
        files={files()}
        selected={undefined}
        onSelect={() => {}}
        thumbnailOf={(source) =>
          source.endsWith('Eagle_Carbine.png') ? 'data:image/png;base64,AAAA' : undefined
        }
      />,
    );
    expect(screen.getAllByRole('img')).toHaveLength(1);
  });

  it('words a replaced file', () => {
    const list = files().map((f) => (f.copy ? { ...f, action: 'replace' as const } : f));
    renderWithProviders(<FileList files={list} selected={undefined} onSelect={() => {}} />);
    expect(screen.getAllByText('Replace')).toHaveLength(4);
  });
});
