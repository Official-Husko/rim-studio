import type { ProjectCountsDto } from 'rimstudio-ipc-types';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';

/** The definition and asset counts of a project as small tiles. */
export function ProjectCounts({ counts }: { counts: ProjectCountsDto }) {
  const tiles = [
    { key: 'weapons', label: t('project.counts.weapons'), value: counts.weaponDefs },
    { key: 'projectiles', label: t('project.counts.projectiles'), value: counts.projectileDefs },
    { key: 'defFiles', label: t('project.counts.defFiles'), value: counts.defFiles },
    { key: 'patches', label: t('project.counts.patches'), value: counts.patchFiles },
    { key: 'textures', label: t('project.counts.textures'), value: counts.textures },
    { key: 'sounds', label: t('project.counts.sounds'), value: counts.sounds },
  ];
  return (
    <ul
      class="m-0 grid list-none grid-cols-3 gap-px md:grid-cols-6 xl:grid-cols-3 border border-line bg-line p-0"
      aria-label={t('project.counts.label')}
    >
      {tiles.map((tile) => (
        <li key={tile.key} class="flex flex-col gap-0.5 bg-surface px-3 py-2">
          <span class="font-mono text-title leading-none">{formatNumber(tile.value, 0)}</span>
          <span class="text-small text-muted">{tile.label}</span>
        </li>
      ))}
    </ul>
  );
}
