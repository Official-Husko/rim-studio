import { fireEvent, render, screen } from '@testing-library/preact';
import type { PlannedFileDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { PlanFile } from './PlanFile';

const created: PlannedFileDto = {
  path: 'Compat/CombatExtended/Patches/x.xml',
  kind: 'ce-patch',
  action: 'create',
  rendered: '<Patch>\n</Patch>\n',
  bytes: 17,
};

describe('PlanFile', () => {
  it('shows the path, what the plan does and the xml of a new file', () => {
    render(<PlanFile file={created} />);
    expect(screen.getByText('Compat/CombatExtended/Patches/x.xml')).toBeTruthy();
    expect(screen.getByText('Combat Extended patch')).toBeTruthy();
    expect(screen.getByText('New file')).toBeTruthy();
    expect(screen.getByRole('region', { name: /Text of Compat/ })).toBeTruthy();
    expect(screen.queryByRole('radiogroup')).toBeNull();
  });

  it('shows the diff of an edited file and switches to the whole file', () => {
    render(
      <PlanFile
        file={{
          ...created,
          action: 'update-region',
          diff: '--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new\n',
        }}
      />,
    );
    expect(screen.getByText('Existing file edited')).toBeTruthy();
    expect(screen.getByRole('region', { name: /Changes to Compat/ })).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Whole file' }));
    expect(screen.getByRole('region', { name: /Text of Compat/ })).toBeTruthy();
  });
});
