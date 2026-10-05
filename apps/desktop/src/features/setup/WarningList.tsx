import { Banner } from 'rimstudio-ui';
import type { DetectionWarningDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { WARNING_KEYS } from './codes';

/** Detection warnings with a plain explanation by code, and the paths they are about. */
export function WarningList({ warnings }: { warnings: readonly DetectionWarningDto[] }) {
  if (warnings.length === 0) return null;
  return (
    <ul class="m-0 flex list-none flex-col gap-2 p-0" aria-label={t('setup.warnings')}>
      {warnings.map((warning) => {
        const key = WARNING_KEYS[warning.code];
        return (
          <li key={`${warning.code}:${warning.paths.join('|')}`}>
            <Banner tone="warning" title={warning.code}>
              <p class="m-0">{key ? t(key) : warning.message}</p>
              {warning.paths.map((path) => (
                <p key={path} class="m-0 mt-1 font-mono text-mono break-all text-muted">
                  {path}
                </p>
              ))}
            </Banner>
          </li>
        );
      })}
    </ul>
  );
}
