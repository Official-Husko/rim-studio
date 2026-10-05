import type { ProjectileChoiceDto } from 'rimstudio-ipc-types';
import { Banner, Panel, Spinner, Switch } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { NumberFieldDef } from '../../model/fields';
import { getAt } from '../../model/pointer';
import { useFieldEnv } from './fieldEnv';
import { MoreFields } from './MoreFields';
import { NumberFieldRow } from './NumberFieldRow';
import { RawNodeList } from './RawNodeList';
import { TextFieldRow } from './TextFieldRow';

const BASE = '/ranged/projectile/def';

const INLINE_NUMBERS: readonly NumberFieldDef[] = [
  { pointer: `${BASE}/speed`, label: 'designer.field.speed', unit: 'tiles/s', step: 1 },
  { pointer: `${BASE}/stoppingPower`, label: 'designer.field.stopping', step: 0.1 },
];

/** The projectile of a ranged weapon: shared with the source, or a projectile of its own. */
export function ProjectilePanel() {
  const env = useFieldEnv();
  const choice = getAt(env.spec, '/ranged/projectile') as ProjectileChoiceDto | undefined;
  const own = choice?.mode === 'inline';
  const inline = choice?.mode === 'inline' ? choice.def : undefined;
  const moreCount =
    (inline?.extra?.length ?? 0) +
    (inline?.graphicExtra?.length ?? 0) +
    (inline?.thingExtra?.length ?? 0);
  const copiedFrom = choice?.mode === 'inline' ? choice.def.copiedFrom : undefined;

  return (
    <Panel title={t('designer.panel.projectile')} collapsible>
      <div class="flex flex-col gap-3">
        <div class="flex flex-wrap items-center gap-3">
          <Switch checked={own} disabled={env.ownBusy} onCheckedChange={env.setOwnProjectile}>
            {t('designer.projectile.own')}
          </Switch>
          {env.ownBusy ? <Spinner label={t('designer.projectile.busy')} /> : null}
        </div>
        <p class="text-small text-muted">
          {own ? t('designer.projectile.ownHelp') : t('designer.projectile.sharedHelp')}
        </p>
        {env.projectileNotes.map((note) => (
          <Banner key={note} tone="info">
            {note}
          </Banner>
        ))}
        {own ? (
          <>
            {copiedFrom ? (
              <p class="font-mono text-mono-small text-faint">
                {t('designer.projectile.copiedFrom', { source: copiedFrom })}
              </p>
            ) : null}
            <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
              <TextFieldRow
                pointer={`${BASE}/defName`}
                label={t('designer.field.defName')}
                keepEmpty
              />
              <TextFieldRow pointer={`${BASE}/label`} label={t('designer.field.label')} keepEmpty />
              <TextFieldRow pointer={`${BASE}/parent`} label={t('designer.field.parent')} />
              <TextFieldRow pointer={`${BASE}/damageDef`} label={t('designer.field.damageDef')} />
              <TextFieldRow
                pointer={`${BASE}/graphicClass`}
                label={t('designer.field.graphicClass')}
              />
              <TextFieldRow pointer={`${BASE}/texturePath`} label={t('designer.field.texture')} />
              {INLINE_NUMBERS.map((def) => (
                <NumberFieldRow key={def.pointer} def={def} />
              ))}
            </div>
            <MoreFields title={t('designer.projectile.more')} count={moreCount}>
              <RawNodeList
                pointer={`${BASE}/extra`}
                label={t('designer.projectile.fields')}
                help={t('designer.projectile.fieldsHelp')}
              />
              <RawNodeList
                pointer={`${BASE}/graphicExtra`}
                label={t('designer.projectile.graphicFields')}
              />
              <RawNodeList
                pointer={`${BASE}/thingExtra`}
                label={t('designer.projectile.thingFields')}
              />
            </MoreFields>
          </>
        ) : (
          <>
            <TextFieldRow
              pointer="/ranged/projectile/def"
              diagnosticPointer="/ranged/projectile"
              label={t('designer.field.projectileDef')}
            />
            <Banner tone="info">{t('designer.projectile.sharedNote')}</Banner>
          </>
        )}
      </div>
    </Panel>
  );
}
