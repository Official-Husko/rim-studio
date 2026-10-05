import { fireEvent, render, screen } from '@testing-library/preact';
import { loadFixture } from 'rimstudio-testkit';
import type { ConvertScanDto } from 'rimstudio-ipc-types';
import { describe, expect, it } from 'vitest';
import { DetailPane } from './DetailPane';
import { installTransport, openFixtureProject } from './testSupport';

const [first] = loadFixture<ConvertScanDto>('designer_convert_scan').candidates;
if (!first) throw new Error('fixture');

describe('DetailPane', () => {
  it('shows what the scan knows and opens on the questions of a convertible weapon', () => {
    installTransport();
    const p = openFixtureProject();
    render(
      <DetailPane
        project={{ projectId: p.projectId, path: p.path, name: p.name }}
        candidate={first}
        familySize={3}
      />,
    );
    expect(screen.getByRole('heading', { name: 'Gewehr 41 (m)' })).toBeTruthy();
    expect(screen.getByText('Bullet_MauserRifle')).toBeTruthy();
    expect(screen.getByRole('tab', { name: /Questions/ }).getAttribute('aria-selected')).toBe(
      'true',
    );
    expect(screen.getByText('0 of 9 answered')).toBeTruthy();
  });

  it('shows the definition file on its tab', async () => {
    installTransport();
    const p = openFixtureProject();
    render(
      <DetailPane
        project={{ projectId: p.projectId, path: p.path, name: p.name }}
        candidate={first}
        familySize={1}
      />,
    );
    fireEvent.click(screen.getByRole('tab', { name: 'Definition' }));
    expect(await screen.findByRole('region', { name: /Text of Defs/ })).toBeTruthy();
  });

  it('has no questions or plan for a weapon that cannot be converted', () => {
    installTransport();
    const p = openFixtureProject();
    render(
      <DetailPane
        project={{ projectId: p.projectId, path: p.path, name: p.name }}
        candidate={{
          ...first,
          status: 'unsupported-kind',
          reason: 'a weapon with a beam verb and no projectile',
          asks: [],
        }}
        familySize={1}
      />,
    );
    expect(screen.queryByRole('tab', { name: /Questions/ })).toBeNull();
    expect(screen.queryByRole('tab', { name: 'Plan' })).toBeNull();
    expect(screen.getByText('a weapon with a beam verb and no projectile')).toBeTruthy();
  });
});
