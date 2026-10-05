import { describe, expect, it } from 'vitest';
import {
  isWindowsCommand,
  stateLabel,
  stateLine,
  stateTone,
  wantsManualCommand,
} from './linkLabels';
import { statusFixture } from './testSupport';

describe('link labels', () => {
  it('names every state in words', () => {
    expect(stateLabel('not-linked')).toBe('Not visible to the game');
    expect(stateLabel('linked')).toBe('Linked into the game');
    expect(stateLabel('foreign-folder')).toBe('Name taken by a folder');
  });

  it('gives a tone to every state', () => {
    expect(stateTone('linked')).toBe('success');
    expect(stateTone('stale')).toBe('warning');
    expect(stateTone('unavailable')).toBe('danger');
    expect(stateTone('not-linked')).toBe('neutral');
  });

  it('explains the not linked state with the name and the Mods folder', () => {
    const line = stateLine(statusFixture('link-status-not-linked'));
    expect(line).toContain('Link it as RS_Arms');
    expect(line).toContain('/Mods');
  });

  it('says why nothing can be linked', () => {
    expect(stateLine(statusFixture('link-status-no-mods'))).toMatch(/does not exist/);
    expect(stateLine(statusFixture('link-status-no-mods', { gameFound: false }))).toMatch(
      /No RimWorld install is selected/,
    );
  });

  it('offers the manual command only where it could work', () => {
    const free = statusFixture('link-status-not-linked');
    expect(wantsManualCommand(free, false)).toBe(false);
    expect(wantsManualCommand(free, true)).toBe(true);
    expect(wantsManualCommand({ ...free, modsReadOnly: true }, false)).toBe(true);
    expect(
      wantsManualCommand(
        { ...free, support: { symlink: false, junction: false, needsPrivilege: true } },
        false,
      ),
    ).toBe(true);
    expect(wantsManualCommand(statusFixture('link-status-no-mods'), false)).toBe(true);
    expect(wantsManualCommand(statusFixture('link-status-linked'), true)).toBe(false);
    expect(wantsManualCommand({ ...free, state: 'foreign-folder' }, true)).toBe(false);
    expect(wantsManualCommand({ ...free, gameFound: false }, true)).toBe(false);
  });

  it('tells a Windows command from a shell command', () => {
    expect(isWindowsCommand('mklink /J "a" "b"')).toBe(true);
    expect(isWindowsCommand("ln -s 'a' 'b'")).toBe(false);
  });
});
