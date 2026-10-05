import { render, screen } from '@testing-library/preact';
import { describe, expect, it } from 'vitest';
import { DefinitionView } from './DefinitionView';
import { installTransport, openFixtureProject } from './testSupport';

describe('DefinitionView', () => {
  it('reads the definition file of the weapon', async () => {
    const transport = installTransport();
    const p = openFixtureProject();
    render(
      <DefinitionView
        project={{ projectId: p.projectId, path: p.path, name: p.name }}
        file="Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml"
      />,
    );
    expect(await screen.findByRole('region', { name: /Text of Defs/ })).toBeTruthy();
    expect(transport.calls.find((c) => c.name === 'project_read_file')?.request).toMatchObject({
      path: 'Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml',
    });
  });

  it('says so when the file is not known', () => {
    installTransport();
    const p = openFixtureProject();
    render(
      <DefinitionView
        project={{ projectId: p.projectId, path: p.path, name: p.name }}
        file={undefined}
      />,
    );
    expect(screen.getByText('The file of this weapon is not known.')).toBeTruthy();
  });
});
