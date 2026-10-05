import { KeyValueList, Panel } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { CarriedInfo } from './CarriedInfo';
import { useFieldEnv } from './fieldEnv';
import { MoreFields } from './MoreFields';
import { NumberFieldRow } from './NumberFieldRow';
import { NumberMapField } from './NumberMapField';
import { RawNodeList } from './RawNodeList';
import { TextFieldRow } from './TextFieldRow';

/** Fields copied from the source that have no group of their own: extra stats, inherited stats. */
export function OtherFieldsPanel() {
  const env = useFieldEnv();
  const extra = Object.keys(env.spec.extraStats ?? {}).sort();
  const inherited = Object.entries(env.spec.parent?.inheritedStats ?? {}).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  const ranged = env.spec.ranged;
  const spec = env.spec;
  const moreCount =
    (spec.comps?.length ?? 0) +
    (spec.otherVerbs?.length ?? 0) +
    (spec.ranged?.verbExtra?.length ?? 0);
  const carries =
    (spec.extraFields?.length ?? 0) +
      (spec.comps?.length ?? 0) +
      (spec.otherVerbs?.length ?? 0) +
      (spec.ranged?.verbExtra?.length ?? 0) +
      Object.keys(spec.equippedStatOffsets ?? {}).length >
    0;
  return (
    <Panel title={t('designer.panel.other')} collapsible defaultCollapsed={!carries}>
      <div class="flex flex-col gap-4">
        {ranged ? (
          <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
            <TextFieldRow pointer="/ranged/verbClass" label={t('designer.field.verbClass')} />
            <NumberFieldRow
              def={{
                pointer: '/ranged/muzzleFlashScale',
                label: 'designer.field.muzzle',
                step: 1,
                plain: true,
              }}
            />
            <NumberFieldRow
              def={{
                pointer: '/ranged/forcedMissRadius',
                label: 'designer.field.forcedMiss',
                unit: 'tiles',
                step: 0.1,
                plain: true,
              }}
            />
          </div>
        ) : null}
        {extra.length > 0 ? (
          <div class="grid grid-cols-2 gap-3 xl:grid-cols-3">
            {extra.map((name) => (
              <NumberFieldRow
                key={name}
                labelText={name}
                def={{ pointer: `/extraStats/${name}`, label: 'designer.field.extra', step: 1 }}
              />
            ))}
          </div>
        ) : null}
        {inherited.length > 0 ? (
          <div class="flex flex-col gap-2">
            <h3 class="font-display text-label tracking-label text-muted uppercase">
              {t('designer.other.inherited')}
            </h3>
            <p class="text-small text-faint">{t('designer.other.inheritedHelp')}</p>
            <KeyValueList
              label={t('designer.other.inherited')}
              items={inherited.map(([key, value]) => ({
                key,
                value: formatNumber(value, 3),
                mono: true,
              }))}
            />
          </div>
        ) : null}
        <NumberMapField
          pointer="/equippedStatOffsets"
          label={t('designer.other.offsets')}
          help={t('designer.other.offsetsHelp')}
          keyLabel={t('designer.other.offsetStat')}
          valueLabel={t('designer.other.offsetValue')}
          addLabel={t('designer.other.addOffset')}
          removeLabel={(name) => t('designer.other.removeOffset', { name })}
          step={0.05}
        />
        <RawNodeList
          pointer="/extraFields"
          label={t('designer.other.fields')}
          help={t('designer.other.fieldsHelp')}
        />
        <MoreFields title={t('designer.other.more')} count={moreCount}>
          <RawNodeList
            pointer="/comps"
            label={t('designer.other.comps')}
            help={t('designer.other.compsHelp')}
            defaultTag="li"
          />
          <RawNodeList
            pointer="/otherVerbs"
            label={t('designer.other.verbs')}
            help={t('designer.other.verbsHelp')}
            defaultTag="li"
          />
          {ranged ? (
            <RawNodeList
              pointer="/ranged/verbExtra"
              label={t('designer.other.verbExtra')}
              help={t('designer.other.verbExtraHelp')}
            />
          ) : null}
        </MoreFields>
        <CarriedInfo />
      </div>
    </Panel>
  );
}
