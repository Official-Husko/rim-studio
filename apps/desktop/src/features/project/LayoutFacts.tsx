import { Badge, KeyValueList, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { ProjectCounts } from './ProjectCounts';
import type { ProjectView } from './store';

const PROFILE_KEYS = {
  rimstudio: 'project.profile.rimstudio',
  'core-style': 'project.profile.core-style',
  flat: 'project.profile.flat',
} as const;

/** How the mod is laid out: the convention, the weapon and content folders, LoadFolders.xml, the Combat Extended folder and the counts. */
export function LayoutFacts({ view }: { view: ProjectView }) {
  const { summary, tree } = view;
  const loadFolders = !summary.hasLoadFolders
    ? t('project.info.loadfolders.none')
    : summary.hasCeGate
      ? t('project.info.loadfolders.gated')
      : t('project.info.loadfolders.present');
  const ce = !tree.ceFolderExists
    ? t('project.info.ce.none', { path: tree.ceFolder })
    : tree.ceFolderLegacy
      ? t('project.info.ce.legacy', { path: tree.ceFolder })
      : t('project.info.ce.standard', { path: tree.ceFolder });
  return (
    <Panel title={t('project.info.layout')} framed collapsible>
      <div class="grid grid-cols-1 gap-x-8 gap-y-4 p-3 md:grid-cols-2">
        <KeyValueList
          label={t('project.info.layout')}
          items={[
            {
              key: t('project.info.profile'),
              value: <Badge tone="info">{t(PROFILE_KEYS[tree.profile])}</Badge>,
            },
            { key: t('project.info.weapons'), value: tree.weaponsFolder, mono: true },
            ...(tree.contentFolder
              ? [{ key: t('project.info.content'), value: tree.contentFolder, mono: true }]
              : []),
            { key: t('project.info.loadfolders'), value: loadFolders },
            { key: t('project.info.ce'), value: ce },
          ]}
        />
        <ProjectCounts counts={tree.counts} />
      </div>
    </Panel>
  );
}
