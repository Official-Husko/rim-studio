import { useId, useState } from 'preact/hooks';
import type { DraftEntryDto, ProjectWeaponDto } from 'rimstudio-ipc-types';
import { Badge, Button, Chip, cx, Icon, KeyValueList } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { detailFacts, keyNumbers, weaponName } from '../model/project-weapons';

export interface ProjectWeaponRowProps {
  weapon: ProjectWeaponDto;
  /** The stored draft of the weapon that the session holds; the button then continues it. */
  draft: DraftEntryDto | undefined;
  /** True when that draft is the one open in the editor. */
  isOpen: boolean;
  /** This weapon is being opened right now. */
  busy: boolean;
  /** The list or another weapon is being read: opening waits. */
  disabled: boolean;
  onLoad: () => void;
}

/** One weapon of the mod: name, kind, the key numbers, what the project holds for it, the details. */
export function ProjectWeaponRow({
  weapon,
  draft,
  isOpen,
  busy,
  disabled,
  onLoad,
}: ProjectWeaponRowProps) {
  const [expanded, setExpanded] = useState(false);
  const detailsId = useId();
  const name = weaponName(weapon);
  const numbers = keyNumbers(weapon);
  return (
    <li
      class={cx('flex flex-col gap-2 border bg-bg p-2', isOpen ? 'border-accent' : 'border-line')}
      data-def={weapon.defName}
      aria-current={isOpen ? 'true' : undefined}
    >
      <div class="flex items-baseline justify-between gap-2">
        <span class="min-w-0 truncate text-body font-semibold text-fg">{name}</span>
        <Badge>
          {weapon.kind === 'ranged' ? t('designer.kind.ranged') : t('designer.kind.melee')}
        </Badge>
      </div>
      <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
        <span class="min-w-0 break-all font-mono text-mono-small text-faint">{weapon.defName}</span>
        {weapon.generatedPatch ? (
          <Badge tone="info">{t('designer.projectWeapons.patch')}</Badge>
        ) : null}
        {draft ? <Badge>{t('designer.projectWeapons.draft')}</Badge> : null}
      </div>
      {numbers.length > 0 ? (
        <ul
          class="flex flex-wrap gap-1"
          aria-label={t('designer.projectWeapons.numbers', { name })}
        >
          {numbers.map((fact) => (
            <li key={fact.id}>
              <Chip>{`${fact.label} ${fact.value}`}</Chip>
            </li>
          ))}
        </ul>
      ) : null}
      <div class="flex flex-wrap items-center justify-between gap-1">
        <Button
          size="sm"
          variant="ghost"
          aria-expanded={expanded}
          aria-controls={detailsId}
          aria-label={t('designer.projectWeapons.detailsOf', { name })}
          onClick={() => setExpanded(!expanded)}
        >
          <Icon name="chevron" turn={expanded ? 1 : 0} />
          {t('designer.projectWeapons.details')}
        </Button>
        <Button
          size="sm"
          loading={busy}
          disabled={disabled}
          aria-label={
            draft
              ? t('designer.projectWeapons.continueOf', { name })
              : t('designer.projectWeapons.loadOf', { name })
          }
          onClick={onLoad}
        >
          {draft ? t('designer.projectWeapons.continue') : t('designer.projectWeapons.load')}
        </Button>
      </div>
      {expanded ? (
        <div id={detailsId}>
          <KeyValueList
            label={t('designer.projectWeapons.detailsOf', { name })}
            items={detailFacts(weapon).map((fact) => ({
              key: fact.label,
              value: fact.value,
              mono: fact.mono,
            }))}
          />
        </div>
      ) : null}
    </li>
  );
}
