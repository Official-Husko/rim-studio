import { screen } from '@testing-library/preact';
import type { PlannedFileDto, WritePlanDto } from 'rimstudio-ipc-types';
import { renderWithProviders } from 'rimstudio-testkit';
import { describe, expect, it } from 'vitest';
import { fixture } from '../../testSupport';
import { CopyPreview } from './CopyPreview';

const files = () => fixture<WritePlanDto>('designer-assets-plan-imports').files;
const texture = (): PlannedFileDto =>
  files().find((f) => f.path.endsWith('DM_Carbine.png')) as PlannedFileDto;

describe('CopyPreview', () => {
  it('shows the source, the target, the size, the dimensions, the hash and the thumbnail', () => {
    renderWithProviders(<CopyPreview file={texture()} thumbnail="data:image/png;base64,AAAA" />);
    expect(screen.getByText('/home/user/Art/TLWWP_Eagle_Carbine.png')).toBeTruthy();
    expect(
      screen.getByText('Textures/Things/Item/Equipment/WeaponRanged/DM_Carbine.png'),
    ).toBeTruthy();
    expect(screen.getByText('7.7 KiB')).toBeTruthy();
    expect(screen.getByText('512 by 512 pixels')).toBeTruthy();
    expect(screen.getByText(/^6865c32845858ead/)).toBeTruthy();
    expect(screen.getByAltText('Thumbnail of TLWWP_Eagle_Carbine.png')).toBeTruthy();
  });

  it('says that a different file at the target is backed up first', () => {
    const file = texture();
    if (file.copy) file.copy.existingSha256 = 'f'.repeat(64);
    renderWithProviders(<CopyPreview file={file} />);
    expect(screen.getByText(/backed up before it is replaced/)).toBeTruthy();
    expect(screen.queryByRole('img')).toBeNull();
  });

  it('shows nothing for a file that is not a copy', () => {
    const def = files().find((f) => f.kind === 'vanilla-defs') as PlannedFileDto;
    const { container } = renderWithProviders(<CopyPreview file={def} />);
    expect(container.textContent).toBe('');
  });
});
