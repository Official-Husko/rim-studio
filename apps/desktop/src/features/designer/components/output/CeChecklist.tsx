import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Banner, Button } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';

export interface CeChecklistProps {
  /** One diagnostic per answer the plan still waits for. */
  pending: readonly DiagnosticDto[];
  /** The question of each field pointer, as the suggestion words it. */
  labels: ReadonlyMap<string, string>;
  onGoTo: (pointer: string) => void;
}

/** What is left to answer before the Combat Extended files can be written, one line each. */
export function CeChecklist({ pending, labels, onGoTo }: CeChecklistProps) {
  if (pending.length === 0) {
    return <Banner tone="success">{t('designer.output.ce.allAnswered')}</Banner>;
  }
  return (
    <div class="flex flex-col gap-2 border border-warning p-2">
      <h3 class="text-body font-semibold text-fg">
        {tn('designer.output.ce.needed', pending.length)}
      </h3>
      <ul class="flex flex-col gap-1">
        {pending.map((d) => (
          <li key={d.field} class="flex items-center gap-2 text-body">
            <span aria-hidden="true" class="inline-block size-3.5 shrink-0 border border-warning" />
            <span class="min-w-0 flex-1 break-words">{labels.get(d.field ?? '') ?? d.message}</span>
            {d.field ? (
              <Button size="sm" variant="ghost" onClick={() => onGoTo(d.field as string)}>
                {t('designer.output.ce.goTo')}
              </Button>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}
