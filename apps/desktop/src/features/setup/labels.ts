import type { ConfidenceDto, HowDto, InstallDto } from 'rimstudio-ipc-types';
import type { MessageKey } from '~/shared/i18n';

export const HOW_KEYS: Record<HowDto, MessageKey> = {
  override: 'setup.how.override',
  'registry-hkcu': 'setup.how.registry-hkcu',
  'registry-hklm': 'setup.how.registry-hklm',
  'xdg-data-home': 'setup.how.xdg-data-home',
  'symlink-steam': 'setup.how.symlink-steam',
  flatpak: 'setup.how.flatpak',
  snap: 'setup.how.snap',
  'library-folders-vdf': 'setup.how.library-folders-vdf',
  appmanifest: 'setup.how.appmanifest',
  'directory-probe': 'setup.how.directory-probe',
};

export const CONFIDENCE_KEYS: Record<ConfidenceDto, MessageKey> = {
  high: 'setup.confidence.high',
  medium: 'setup.confidence.medium',
  low: 'setup.confidence.low',
};

export const CONFIDENCE_TONES = {
  high: 'success',
  medium: 'warning',
  low: 'danger',
} as const satisfies Record<ConfidenceDto, string>;

export const HEALTH_KEYS: Record<InstallDto['health'], MessageKey> = {
  installed: 'setup.health.installed',
  'update-pending': 'setup.health.update-pending',
  'needs-verify': 'setup.health.needs-verify',
};
