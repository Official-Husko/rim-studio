import type { WritePlanDto } from 'rimstudio-ipc-types';
import { Badge, KeyValueList } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import { clipCopies, isSoundDefFile, plannedSoundName } from '../../model/assets';
import { actionText, actionTone } from '../output/labels';

export interface SoundPlanSummaryProps {
  plan: WritePlanDto | undefined;
  /** The name typed for the sound definition, shown when the plan has none yet. */
  typedName: string | undefined;
}

/** What the plan will write for the custom shot sound: the sound definition and the copied clips. */
export function SoundPlanSummary({ plan, typedName }: SoundPlanSummaryProps) {
  const copies = clipCopies(plan);
  const file = plan?.files.find(isSoundDefFile);
  if (copies.length === 0 || !file) {
    return <p class="text-small text-faint">{t('designer.sounds.plan.none')}</p>;
  }
  const name = plannedSoundName(plan) ?? typedName ?? '';
  return (
    <div class="flex min-w-0 flex-col gap-2 border border-line bg-bg p-2">
      <h4 class="text-small font-semibold text-muted">{t('designer.sounds.plan.title')}</h4>
      <KeyValueList
        label={t('designer.sounds.plan.title')}
        items={[
          { key: t('designer.sounds.plan.def'), value: name, mono: true },
          {
            key: t('designer.sounds.plan.file'),
            value: (
              <span class="flex flex-wrap items-center gap-2">
                <span class="break-all">{file.path}</span>
                <Badge tone={actionTone(file.action)}>{actionText(file.action)}</Badge>
              </span>
            ),
            mono: true,
          },
        ]}
      />
      <p class="text-small text-muted">{tn('designer.sounds.plan.copies', copies.length)}</p>
      <ul aria-label={t('designer.sounds.plan.copiesLabel')} class="flex flex-col gap-1">
        {copies.map((copy) => (
          <li key={copy.path} class="flex flex-wrap items-center gap-2">
            <span class="break-all font-mono text-mono-small text-fg">{copy.path}</span>
            <Badge tone={actionTone(copy.action)}>{actionText(copy.action)}</Badge>
            <span class="font-mono text-mono-small text-faint">{formatBytes(copy.bytes)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
