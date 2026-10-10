import { Banner, Button } from 'rimstudio-ui';
import { MissingList } from '~/shared/ce';
import { t, tn } from '~/shared/i18n';
import { ceLabels } from '../../output-model';
import type { OutputStore } from '../../output-store';

export interface ApplyBarProps {
  store: OutputStore;
  /** Moves the focus to the field a pointer names; absent hides the shortcuts. */
  onGoTo?: (pointer: string) => void;
}

/** The Apply button with the reason it is not available, and a note about the last write. */
export function ApplyBar({ store, onGoTo }: ApplyBarProps) {
  const reason = store.blockedBy.value;
  const errors = store.plan.value?.diagnostics.filter((d) => d.severity === 'error').length ?? 0;
  const last = store.lastReport.value;
  // answers the plan waits for are named; only the errors that are not answers are counted
  const waiting = store.pending.value;
  const labels = ceLabels(store.suggestion.value);
  const otherErrors = Math.max(0, errors - waiting.length);
  const missing = waiting.map((d) => ({
    key: d.field ?? d.message,
    label: labels.get(d.field ?? '') ?? d.message,
  }));
  return (
    <div class="flex flex-col gap-2">
      <Button
        variant="primary"
        icon="check"
        disabled={reason !== undefined}
        onClick={store.openApply}
        {...(reason ? { 'aria-describedby': 'designer-apply-reason' } : {})}
      >
        {t('designer.output.apply.button')}
      </Button>
      {reason ? (
        <div id="designer-apply-reason" class="flex flex-col gap-2 text-small text-muted">
          {reason === 'designer.output.blocked.errors' && missing.length > 0 ? (
            <MissingList
              title={tn('designer.output.blocked.answers', missing.length)}
              items={missing}
              goLabel={t('designer.output.ce.goTo')}
              onGoTo={onGoTo}
              footer={
                otherErrors > 0
                  ? tn('designer.output.blocked.alsoErrors', otherErrors)
                  : t('designer.output.blocked.answersFooter')
              }
            />
          ) : (
            <p class="m-0">
              {reason === 'designer.output.blocked.errors'
                ? tn('designer.output.blocked.errorsCount', errors)
                : t(reason)}
            </p>
          )}
        </div>
      ) : null}
      {last ? (
        <Banner
          tone="success"
          action={
            <Button size="sm" variant="ghost" onClick={store.showReport}>
              {t('designer.output.apply.showResult')}
            </Button>
          }
        >
          {tn('designer.output.result.written', last.written.length)}
        </Banner>
      ) : null}
    </div>
  );
}
