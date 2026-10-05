import type { MessageKey } from '~/shared/i18n';

/** Plain explanations of detection warnings by code; unknown codes show the backend message. */
export const WARNING_KEYS: Readonly<Record<string, MessageKey>> = {
  'steam.manifest-leftover': 'setup.warning.steam.manifest-leftover',
  'steam.userdir-version-mismatch': 'setup.warning.steam.userdir-version-mismatch',
  'steam.library-offline': 'setup.warning.steam.library-offline',
  'steam.library-timeout': 'setup.warning.steam.library-timeout',
  'steam.install-update-pending': 'setup.warning.steam.install-update-pending',
  'steam.install-needs-verify': 'setup.warning.steam.install-needs-verify',
  'steam.install-is-symlink': 'setup.warning.steam.install-is-symlink',
  'steam.multiple-installs': 'setup.warning.steam.multiple-installs',
  'steam.proton-and-native-userdirs': 'setup.warning.steam.proton-and-native-userdirs',
  'steam.workshop-different-library': 'setup.warning.steam.workshop-different-library',
};

/** Plain explanations of scan diagnostics by code. */
export const DIAGNOSTIC_KEYS: Readonly<Record<string, MessageKey>> = {
  'about.no-supported-versions': 'setup.diag.about.no-supported-versions',
  'about.field-dropped': 'setup.diag.about.field-dropped',
  'loadfolders.ignored-attribute': 'setup.diag.loadfolders.ignored-attribute',
  'scan.no-about': 'setup.diag.scan.no-about',
  'scan.dir-unreadable': 'setup.diag.scan.dir-unreadable',
  'scan.def-file-broken': 'setup.diag.scan.def-file-broken',
  'scan.package-id-missing': 'setup.diag.scan.package-id-missing',
};

/** Plain explanations of folder probe diagnostics by code. */
export const PROBE_KEYS: Readonly<Record<string, MessageKey>> = {
  'deploy.source-overlap': 'setup.probe.overlap',
  'deploy.source-offline': 'setup.probe.offline',
};
