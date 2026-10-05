import { useState } from 'preact/hooks';
import type { ApiError } from 'rimstudio-ipc-types';
import { Banner, Button, EmptyState } from 'rimstudio-ui';
import { normalizeError } from '~/shared/ipc';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { openProjectAt } from '~/shared/project';

export interface NoProjectProps {
  /** Set when the project was remembered but its folder could not be opened. */
  missing?: { path: string; reason: string } | undefined;
}

/** Shown while no project is open, or while the open one cannot be found any more. */
export function NoProject({ missing }: NoProjectProps) {
  const [error, setError] = useState<ApiError | undefined>(undefined);
  const choose = async (): Promise<void> => {
    const picked = await pickFolder();
    if (!picked) return;
    try {
      await openProjectAt(picked);
      setError(undefined);
    } catch (thrown) {
      setError(normalizeError(thrown));
    }
  };
  return (
    <div class="flex flex-col gap-3">
      <EmptyState
        icon="folder"
        title={missing ? t('patches.missing.title') : t('patches.noproject.title')}
        description={
          missing ? t('patches.missing.body', { path: missing.path }) : t('patches.noproject.body')
        }
        action={
          <div class="flex flex-wrap items-center justify-center gap-2">
            <Button variant="primary" icon="folder" onClick={() => void choose()}>
              {t('patches.noproject.choose')}
            </Button>
            <a class="text-accent underline" href="#/project">
              {t('patches.noproject.link')}
            </a>
          </div>
        }
      />
      {missing ? <p class="m-0 text-center text-small text-muted">{missing.reason}</p> : null}
      {error ? (
        <Banner tone="error" title={error.code}>
          {error.message}
        </Banner>
      ) : null}
    </div>
  );
}
