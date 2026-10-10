import { describe, expect, it } from 'vitest';
import { openWeapon, projectWeapons } from './api';
import { installTransport, openWeaponAnswer, projectWeaponsAnswer } from './testSupport';

describe('project weapon commands', () => {
  it('asks for the weapons of a project by its id', async () => {
    const transport = installTransport();
    const answer = await projectWeapons('p-1');
    expect(transport.calls).toEqual([
      { name: 'designer_project_weapons', request: { projectId: 'p-1' } },
    ]);
    expect(answer).toEqual(projectWeaponsAnswer());
  });

  it('opens a weapon by its definition name and leaves the reload member out', async () => {
    const transport = installTransport();
    const answer = await openWeapon('p-1', 'RS_Gun_Ranger');
    expect(transport.calls).toEqual([
      { name: 'designer_open_weapon', request: { projectId: 'p-1', defName: 'RS_Gun_Ranger' } },
    ]);
    expect(answer).toEqual(openWeaponAnswer());
  });

  it('sends the reload member when it is given', async () => {
    const transport = installTransport();
    await openWeapon('p-1', 'RS_Gun_Ranger', true);
    expect(transport.calls[0]?.request).toEqual({
      projectId: 'p-1',
      defName: 'RS_Gun_Ranger',
      reload: true,
    });
  });
});
