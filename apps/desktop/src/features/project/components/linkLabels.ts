import type { BadgeTone } from 'rimstudio-ui';
import type { ProjectLinkStateDto, ProjectLinkStatusDto } from 'rimstudio-ipc-types';
import { t, type MessageKey } from '~/shared/i18n';

/** The badge tone of a state: the words carry the meaning, the tone only helps the eye. */
export function stateTone(state: ProjectLinkStateDto): BadgeTone {
  switch (state) {
    case 'linked':
    case 'copy':
    case 'in-mods':
      return 'success';
    case 'linked-by-hand':
      return 'info';
    case 'stale':
    case 'foreign-link':
    case 'foreign-folder':
      return 'warning';
    case 'unavailable':
      return 'danger';
    case 'not-linked':
      return 'neutral';
  }
}

// The keys are written out so that the catalog check can see that each one is used.
const STATE_KEYS: Record<ProjectLinkStateDto, MessageKey> = {
  'not-linked': 'project.link.state.not-linked',
  linked: 'project.link.state.linked',
  'linked-by-hand': 'project.link.state.linked-by-hand',
  stale: 'project.link.state.stale',
  'foreign-link': 'project.link.state.foreign-link',
  'foreign-folder': 'project.link.state.foreign-folder',
  copy: 'project.link.state.copy',
  'in-mods': 'project.link.state.in-mods',
  unavailable: 'project.link.state.unavailable',
};

const LINE_KEYS: Record<Exclude<ProjectLinkStateDto, 'unavailable'>, MessageKey> = {
  'not-linked': 'project.link.line.not-linked',
  linked: 'project.link.line.linked',
  'linked-by-hand': 'project.link.line.linked-by-hand',
  stale: 'project.link.line.stale',
  'foreign-link': 'project.link.line.foreign-link',
  'foreign-folder': 'project.link.line.foreign-folder',
  copy: 'project.link.line.copy',
  'in-mods': 'project.link.line.in-mods',
};

const ACTIVE_KEYS: Record<ProjectLinkStatusDto['activeInGame'], MessageKey> = {
  active: 'project.link.active.active',
  inactive: 'project.link.active.inactive',
  unknown: 'project.link.active.unknown',
};

/** The short name of a state. */
export function stateLabel(state: ProjectLinkStateDto): string {
  return t(STATE_KEYS[state]);
}

/** The sentence under the badge. */
export function stateLine(status: ProjectLinkStatusDto): string {
  const args = {
    name: status.linkName ?? '',
    mods: status.modsFolder ?? '',
    entry: status.entryPath ?? '',
    target: status.pointsTo ?? '',
  };
  if (status.state === 'unavailable') {
    if (!status.gameFound) return t('project.link.line.noGame');
    if (!status.modsExists) return t('project.link.line.noMods', args);
    return t('project.link.line.unavailable');
  }
  return t(LINE_KEYS[status.state], args);
}

/** The answer of the active list hint. */
export function activeLabel(state: ProjectLinkStatusDto['activeInGame']): string {
  return t(ACTIVE_KEYS[state]);
}

/**
 * The manual command is offered when the automatic way is refused or impossible and the command could
 * work: the name is free but the folder is read only or links are not available, or the Mods folder is
 * missing (the person creates it first).
 */
export function wantsManualCommand(status: ProjectLinkStatusDto, refused: boolean): boolean {
  if (!status.manualCommand || !status.gameFound) return false;
  if (status.state === 'unavailable') return true;
  if (status.state !== 'not-linked') return false;
  return refused || status.modsReadOnly || (!status.support.symlink && !status.support.junction);
}

/** True when the manual command is for a Windows command prompt. */
export function isWindowsCommand(command: string): boolean {
  return command.startsWith('mklink');
}
