import type { ApiError } from 'rimstudio-ipc-types';
import { Banner, Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

const CE_UNAVAILABLE = 'designer.reference-unavailable';

export interface ScanProblemProps {
  error: ApiError;
  onRetry: () => void;
}

/** A scan that failed. Combat Extended missing is explained in plain words; anything else shows its code. */
export function ScanProblem({ error, onRetry }: ScanProblemProps) {
  const retry = (
    <Button size="sm" variant="secondary" icon="refresh" onClick={onRetry}>
      {t('patches.retry')}
    </Button>
  );
  if (error.code === CE_UNAVAILABLE) {
    return (
      <Banner tone="warning" title={t('patches.ce.title')} action={retry}>
        <p class="m-0">{t('patches.ce.body')}</p>
        <p class="m-0 mt-1 text-muted">{error.message}</p>
        <p class="m-0 mt-1">
          <a class="text-accent underline" href="#/setup">
            {t('patches.ce.link')}
          </a>
        </p>
      </Banner>
    );
  }
  return (
    <Banner tone="error" title={t('patches.scan.failed')} action={retry}>
      <code class="font-mono text-mono">{error.code}</code> {error.message}
    </Banner>
  );
}
