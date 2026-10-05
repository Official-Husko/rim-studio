import { Switch } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { getAt } from '../../model/pointer';
import { ExtraDamageRows } from './ExtraDamageRows';
import { useFieldEnv } from './fieldEnv';
import { MoreFields } from './MoreFields';
import { RawNodeList } from './RawNodeList';

export interface ToolExtrasProps {
  /** Position of the tool in the tools list. */
  index: number;
}

/** What a tool carries beyond its numbers: extra damages, the surprise attack and other XML fields. */
export function ToolExtras({ index }: ToolExtrasProps) {
  const env = useFieldEnv();
  const base = `/tools/${index}`;
  const tool = env.spec.tools?.[index];
  const moreCount = (tool?.extraMeleeDamages?.length ?? 0) + (tool?.extra?.length ?? 0);
  const surprise = getAt(env.spec, `${base}/surpriseAttack`);
  return (
    <div class="flex flex-col gap-3 border-t border-line-subtle pt-3">
      <Switch
        checked={surprise !== undefined}
        onCheckedChange={(on) => env.setField(`${base}/surpriseAttack`, on ? {} : undefined)}
      >
        {t('designer.tool.surprise')}
      </Switch>
      {surprise !== undefined ? (
        <ExtraDamageRows
          pointer={`${base}/surpriseAttack/extraMeleeDamages`}
          label={t('designer.tool.surpriseDamages')}
          addLabel={t('designer.tool.addSurpriseDamage')}
        />
      ) : null}
      <MoreFields title={t('designer.tool.more')} count={moreCount}>
        <ExtraDamageRows
          pointer={`${base}/extraMeleeDamages`}
          label={t('designer.tool.extraDamages')}
          addLabel={t('designer.tool.addExtraDamage')}
        />
        <RawNodeList
          pointer={`${base}/extra`}
          label={t('designer.tool.otherFields')}
          help={t('designer.tool.otherFieldsHelp')}
        />
      </MoreFields>
    </div>
  );
}
