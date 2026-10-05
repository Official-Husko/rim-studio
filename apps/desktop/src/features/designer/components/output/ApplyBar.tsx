import { Banner, Button } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { OutputStore } from '../../output-store';

export interface ApplyBarProps {
  store: OutputStore;
}

/** The Apply button with the reason it is not available, and a note about the last write. */
export function ApplyBar({ store }: ApplyBarProps) {
  const reason = store.blockedBy.value;
  const errors = store.plan.value?.diagnostics.filter((d) => d.severity === 'error').length ?? 0;
  const last = store.lastReport.value;
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
        <p id="designer-apply-reason" class="text-small text-muted">
          {reason === 'designer.output.blocked.errors'
            ? tn('designer.output.blocked.errorsCount', errors)
            : t(reason)}
        </p>
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
