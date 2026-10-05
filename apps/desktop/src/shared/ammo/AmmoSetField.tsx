import { useMemo, useState } from 'preact/hooks';
import type { CustomAmmoDto, DraftDto, WritePlanDto } from 'rimstudio-ipc-types';
import { Banner, Button, Chip, FormField } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { AmmoBrowser } from './AmmoBrowser';
import { createCatalogStore } from './catalogStore';
import { CustomAmmoDialog } from './custom/CustomAmmoDialog';
import { typeTitle } from './custom/model';
import { useAmmoSummary } from './useAmmoSummary';

/** One of the ranked suggestions offered as a quick pick. */
export interface QuickPick {
  name: string;
  /** A short note such as how many converted weapons use the set. */
  hint?: string | undefined;
}

export interface AmmoSetFieldProps {
  /** The visible label of the field. */
  label: string;
  /** The ammo set chosen now, by def name. */
  value: string | undefined;
  /** The custom ammo of the weapon, when it has some; it stands in for a chosen set. */
  custom?: CustomAmmoDto | undefined;
  /** The ranked suggestions, best first. */
  quickPicks?: readonly QuickPick[];
  required?: boolean;
  /** Why the question is open, when the backend says. */
  reason?: string | undefined;
  /** The current design, so the browser marks the sets it would suggest. */
  draft?: (() => DraftDto | undefined) | undefined;
  /** Choose an existing set; undefined clears the choice. */
  onSelect: (defName: string | undefined) => void;
  /** Save custom ammo, or remove it with undefined. */
  onCustomChange: (custom: CustomAmmoDto | undefined) => void;
  /** Plans the weapon with a custom ammo spec so the backend can check it; absent without an open project. */
  plan?: ((custom: CustomAmmoDto) => Promise<WritePlanDto>) | undefined;
  /** Custom ammunition is offered only where the plan writes files into a project. */
  canCreate?: boolean;
  /** Why creating is not offered, shown in the browser instead of the button. */
  createHint?: string | undefined;
}

/**
 * The choice of the ammo set of a Combat Extended conversion: the chosen set with its ammo types, the ranked
 * suggestions as quick picks, a browser of every ammo set of the install and the window that creates a custom
 * caliber. The weapon is never given an ammo set unless the user picks one.
 */
export function AmmoSetField(props: AmmoSetFieldProps) {
  const { value, custom, quickPicks = [], canCreate = true } = props;
  const [browsing, setBrowsing] = useState(false);
  const [creating, setCreating] = useState(false);
  const catalog = useMemo(
    () => createCatalogStore(props.draft ? { draft: props.draft } : {}),
    // the browser keeps its filters while the field stays on screen
    // oxlint-disable-next-line react-hooks/exhaustive-deps
    [],
  );
  const summary = useAmmoSummary(custom ? undefined : value);
  const startCreate = (): void => {
    setBrowsing(false);
    setCreating(true);
  };
  return (
    <div class="flex flex-col gap-2" data-ammo-set-field>
      <FormField
        label={props.label}
        {...(props.required && !value && !custom ? { required: true } : {})}
      >
        <div class="flex flex-col gap-2">
          {custom ? (
            <div
              class="flex flex-col gap-2 border border-accent bg-accent-tint p-3"
              data-ammo-custom
            >
              <div class="flex flex-wrap items-baseline gap-2">
                <span class="text-body font-semibold">
                  {custom.name || t('ammo.custom.unnamed')}
                </span>
                <Chip kind="derived">{t('ammo.field.custom-chip')}</Chip>
                <span class="text-small text-muted">{custom.caliber}</span>
              </div>
              <ul
                class="m-0 flex list-none flex-wrap gap-1 p-0"
                aria-label={t('ammo.detail.types')}
              >
                {custom.types.map((type, index) => (
                  <li key={`${type.key}-${index}`}>
                    <Chip>{typeTitle(type, index)}</Chip>
                  </li>
                ))}
              </ul>
              <div class="flex flex-wrap gap-2">
                <Button size="sm" onClick={() => setCreating(true)}>
                  {t('ammo.custom.edit')}
                </Button>
                <Button size="sm" variant="danger" onClick={() => props.onCustomChange(undefined)}>
                  {t('ammo.custom.remove')}
                </Button>
              </div>
            </div>
          ) : value ? (
            <div class="flex flex-col gap-1 border border-line bg-surface p-3" data-ammo-chosen>
              <div class="flex flex-wrap items-baseline gap-2">
                <span class="text-body font-semibold">{summary?.label ?? value}</span>
                {summary ? <span class="text-small text-muted">{summary.caliber}</span> : null}
                <span class="font-mono text-mono-small text-faint">{value}</span>
              </div>
              {summary ? (
                <ul
                  class="m-0 flex list-none flex-wrap gap-1 p-0"
                  aria-label={t('ammo.detail.types')}
                >
                  {summary.types.map((type) => (
                    <li key={type.ammoDef}>
                      <Chip>{type.ammoClassLabel}</Chip>
                    </li>
                  ))}
                </ul>
              ) : null}
              {summary && summary.weaponCount > 0 ? (
                <p class="m-0 text-small text-muted">
                  {tn('ammo.row.weapons', summary.weaponCount)}
                </p>
              ) : null}
            </div>
          ) : (
            <p class="m-0 text-small text-muted">{props.reason ?? t('ammo.field.none')}</p>
          )}
          <div class="flex flex-wrap gap-2">
            <Button icon="search" onClick={() => setBrowsing(true)}>
              {t('ammo.browse')}
            </Button>
            {canCreate && !custom ? (
              <Button icon="plus" onClick={startCreate}>
                {t('ammo.create')}
              </Button>
            ) : null}
            {value && !custom ? (
              <Button variant="ghost" onClick={() => props.onSelect(undefined)}>
                {t('ammo.field.clear')}
              </Button>
            ) : null}
          </div>
        </div>
      </FormField>
      {quickPicks.length > 0 && !custom ? (
        <div class="flex flex-col gap-1" role="group" aria-label={t('ammo.quick.label')}>
          <span class="text-small text-muted">{t('ammo.quick.label')}</span>
          <div class="flex flex-wrap gap-1">
            {quickPicks.slice(0, 5).map((pick) => (
              <Button
                key={pick.name}
                size="sm"
                variant={pick.name === value ? 'primary' : 'secondary'}
                aria-pressed={pick.name === value}
                onClick={() => props.onSelect(pick.name)}
              >
                {pick.hint ? `${pick.name} (${pick.hint})` : pick.name}
              </Button>
            ))}
          </div>
        </div>
      ) : null}
      {custom && props.canCreate === false ? (
        <Banner tone="info">{t('ammo.custom.needs-project')}</Banner>
      ) : null}
      <AmmoBrowser
        open={browsing}
        store={catalog}
        current={custom ? undefined : value}
        onClose={() => setBrowsing(false)}
        onSelect={(def) => {
          props.onSelect(def);
          setBrowsing(false);
        }}
        onCreateCustom={canCreate ? startCreate : undefined}
        createHint={props.createHint}
      />
      <CustomAmmoDialog
        open={creating}
        initial={custom}
        plan={props.plan}
        onClose={() => setCreating(false)}
        onSave={(next) => {
          props.onCustomChange(next);
          setCreating(false);
        }}
        onRemove={
          custom
            ? () => {
                props.onCustomChange(undefined);
                setCreating(false);
              }
            : undefined
        }
      />
    </div>
  );
}
