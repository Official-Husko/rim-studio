import { Badge, KeyValueList, type KeyValueItem } from 'rimstudio-ui';
import type { DetectionReportDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { formatNumber } from '~/shared/format';
import { CONFIDENCE_KEYS, CONFIDENCE_TONES, HEALTH_KEYS, HOW_KEYS } from './labels';

/** The facts of the selected install, Steam and user folder. */
export function DetectionDetails({ report }: { report: DetectionReportDto }) {
  const install = report.installs.find((i) => i.id === report.selected.install);
  const userDir = report.userDirs.find((u) => u.path === report.selected.userDir);
  const items: KeyValueItem[] = [];
  if (install) {
    items.push(
      { key: t('setup.game.path'), value: install.path, mono: true },
      {
        key: t('setup.game.version'),
        value: install.version?.raw ?? t('setup.game.version.unknown'),
        mono: true,
      },
      {
        key: t('setup.game.found'),
        value: (
          <span class="inline-flex flex-wrap items-center gap-2">
            {t(HOW_KEYS[install.how])}
            <Badge tone={CONFIDENCE_TONES[install.confidence]}>
              {t(CONFIDENCE_KEYS[install.confidence])}
            </Badge>
            <Badge tone={install.health === 'installed' ? 'neutral' : 'warning'}>
              {t(HEALTH_KEYS[install.health])}
            </Badge>
          </span>
        ),
      },
      {
        key: t('setup.game.content'),
        value: install.dataDirs.join(', ') || t('setup.none'),
        mono: true,
      },
    );
    for (const workshop of install.workshop) {
      items.push({
        key: t('setup.game.workshop'),
        value: (
          <span>
            <span class="font-mono text-mono">{workshop.contentDir}</span>
            <span class="ml-2 text-muted">
              {tn('setup.game.workshop.items', workshop.itemsOnDisk, {
                total: formatNumber(workshop.itemsInAcf ?? workshop.itemsOnDisk, 0),
              })}
            </span>
          </span>
        ),
      });
    }
  }
  items.push({
    key: t('setup.game.steam'),
    value:
      report.steamRoots.length === 0 ? (
        t('setup.none')
      ) : (
        <ul class="m-0 list-none p-0">
          {report.steamRoots.map((root) => (
            <li key={root.path} class="font-mono text-mono">
              {root.path}
            </li>
          ))}
        </ul>
      ),
  });
  items.push({
    key: t('setup.game.libraries'),
    value:
      report.libraries.length === 0 ? (
        t('setup.none')
      ) : (
        <ul class="m-0 list-none p-0">
          {report.libraries.map((lib) => (
            <li key={lib.path} class="flex flex-wrap items-center gap-2">
              <span class="font-mono text-mono">{lib.path}</span>
              {lib.hasApp ? <Badge tone="success">{t('setup.game.libraries.has')}</Badge> : null}
              {lib.online ? null : (
                <Badge tone="warning">{t('setup.game.libraries.offline')}</Badge>
              )}
            </li>
          ))}
        </ul>
      ),
  });
  items.push({
    key: t('setup.game.userdir'),
    value: userDir ? (
      <span>
        <span class="font-mono text-mono">{userDir.path}</span>
        <span class="ml-2 text-muted">
          {userDir.modsConfigExists
            ? t('setup.game.modsconfig.yes')
            : t('setup.game.modsconfig.no')}
        </span>
      </span>
    ) : (
      t('setup.none')
    ),
  });
  return <KeyValueList items={items} label={t('setup.game.details')} />;
}
