import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Panel } from 'rimstudio-ui';
import type { SourceDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { listSources } from './api';
import { openError, openFolder, opening } from './store';

/** The sources a mod project may live in: the folders the user keeps their own mods in. */
function candidates(sources: readonly SourceDto[]): SourceDto[] {
  return sources.filter((s) => s.enabled && (s.kind === 'custom' || s.kind === 'game-mods'));
}

/** Open an existing mod: pick its folder, or start the picker inside one of the known mod folders. */
export function OpenCard() {
  const [sources, setSources] = useState<SourceDto[]>([]);
  useEffect(() => {
    let live = true;
    listSources().then(
      (list) => live && setSources(candidates(list)),
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);

  const choose = async (start?: string): Promise<void> => {
    const picked = await pickFolder(start ? { start } : {});
    if (picked) await openFolder(picked);
  };

  return (
    <Panel title={t('project.open.title')} framed>
      <div class="flex flex-col gap-3 p-3">
        <p class="m-0 text-small text-muted">{t('project.open.hint')}</p>
        <div>
          <Button icon="folder" loading={opening.value} onClick={() => void choose()}>
            {t('project.open.choose')}
          </Button>
        </div>
        {openError.value ? (
          <Banner tone="error" title={t('project.open.error')}>
            {openError.value.code === 'io.not-found'
              ? t('project.open.notamod')
              : openError.value.message}
          </Banner>
        ) : null}
        {sources.length > 0 ? (
          <div class="flex flex-col gap-1">
            <h3 class="m-0 font-display text-small tracking-display text-muted uppercase">
              {t('project.open.sources')}
            </h3>
            <ul class="m-0 list-none divide-y divide-line-subtle p-0">
              {sources.map((source) => (
                <li key={source.id} class="flex items-center gap-3 py-1">
                  <span class="min-w-0 flex-1">
                    <span class="block truncate">{source.label}</span>
                    <span class="block truncate font-mono text-mono-small text-faint">
                      {source.path}
                    </span>
                  </span>
                  <Button
                    size="sm"
                    variant="secondary"
                    aria-label={t('project.open.in', { name: source.label })}
                    onClick={() => void choose(source.path)}
                  >
                    {t('project.open.browse')}
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
    </Panel>
  );
}
