import { Badge, Icon } from 'rimstudio-ui';
import type { NodeRoleDto } from 'rimstudio-ipc-types';
import type { MessageKey } from '~/shared/i18n';
import { t } from '~/shared/i18n';
import { ROLE_ICON, ROLE_LABEL } from './model';

interface GuideRow {
  role: NodeRoleDto;
  path: string;
  note: MessageKey;
}

// The roles of the RimStudio mod layout (docs/features/mod-layout.md section 4). The role ids and
// the badges of the tree come from the backend; this table only explains them.
const ROWS: readonly GuideRow[] = [
  { role: 'about', path: 'About/', note: 'project.guide.about' },
  { role: 'load-folders', path: 'LoadFolders.xml', note: 'project.guide.load-folders' },
  { role: 'content-root', path: '1.6/ or Common/', note: 'project.guide.content-root' },
  { role: 'defs', path: 'Defs/', note: 'project.guide.defs' },
  {
    role: 'defs-weapons',
    path: 'Defs/ThingDefs_Misc/Weapons/<Category>/<DefName>.xml',
    note: 'project.guide.defs-weapons',
  },
  { role: 'defs-sounds', path: 'Defs/SoundDefs/', note: 'project.guide.defs-sounds' },
  { role: 'patches', path: 'Patches/', note: 'project.guide.patches' },
  {
    role: 'ce-compat',
    path: 'Compat/CombatExtended/Patches/',
    note: 'project.guide.ce-compat',
  },
  {
    role: 'textures',
    path: 'Textures/Things/Item/Equipment/WeaponRanged/',
    note: 'project.guide.textures',
  },
  { role: 'sounds', path: 'Sounds/Weapons/', note: 'project.guide.sounds' },
  { role: 'languages', path: 'Languages/English/Keyed/', note: 'project.guide.languages' },
  { role: 'assemblies', path: 'Assemblies/', note: 'project.guide.assemblies' },
  { role: 'source', path: 'Source/Art/', note: 'project.guide.source' },
];

/** A short legend of the RimStudio mod layout: every role and where it lives. */
export function LayoutGuide() {
  return (
    <div class="flex flex-col gap-3 p-3">
      <p class="m-0 text-small text-muted">{t('project.guide.intro')}</p>
      <ul
        class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0"
        aria-label={t('project.guide.label')}
      >
        {ROWS.map((row) => (
          <li
            key={row.role}
            class="grid grid-cols-1 gap-x-4 gap-y-1 px-3 py-2 sm:grid-cols-[11rem_1fr]"
          >
            <span class="flex items-center gap-2">
              <span class="text-muted">
                <Icon name={ROLE_ICON[row.role]} />
              </span>
              <Badge tone="info">{t(ROLE_LABEL[row.role])}</Badge>
            </span>
            <span class="flex min-w-0 flex-col gap-0.5">
              <span class="font-mono text-mono break-all">{row.path}</span>
              <span class="text-small text-muted">{t(row.note)}</span>
            </span>
          </li>
        ))}
      </ul>
      <ul class="m-0 flex list-disc flex-col gap-1 pl-5 text-small text-muted">
        <li>{t('project.guide.rule.perWeapon')}</li>
        <li>{t('project.guide.rule.vanilla')}</li>
        <li>{t('project.guide.rule.keep')}</li>
      </ul>
    </div>
  );
}
