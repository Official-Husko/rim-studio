import { useState } from 'preact/hooks';
import type { ApiError } from 'rimstudio-ipc-types';
import { Banner, Button, FormField, Panel, TextField } from 'rimstudio-ui';
import { normalizeError } from '~/shared/ipc';
import { pickFolder } from '~/shared/platform';
import { t } from '~/shared/i18n';
import { selectProject } from '../project-source';

export interface ProjectPromptProps {
  /** Called after a project was opened. */
  onOpened: () => void;
}

/** Shown while no project is open: the drafts live in a project folder. */
export function ProjectPrompt({ onOpened }: ProjectPromptProps) {
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ApiError | undefined>(undefined);

  const open = async (target: string): Promise<void> => {
    setBusy(true);
    try {
      await selectProject(target);
      setError(undefined);
      onOpened();
    } catch (thrown) {
      setError(normalizeError(thrown));
    } finally {
      setBusy(false);
    }
  };

  const browse = async (): Promise<void> => {
    const picked = await pickFolder();
    if (picked) {
      setPath(picked);
      await open(picked);
    }
  };

  return (
    <Panel title={t('designer.panel.drafts')}>
      <div class="flex flex-col gap-3">
        <p class="text-small text-muted">{t('designer.project.none')}</p>
        {error ? (
          <Banner tone="error" title={error.code}>
            {error.message}
          </Banner>
        ) : null}
        <FormField label={t('designer.project.path')}>
          <TextField value={path} onValueChange={setPath} />
        </FormField>
        <div class="flex gap-2">
          <Button
            variant="primary"
            loading={busy}
            disabled={path.trim() === ''}
            onClick={() => void open(path.trim())}
          >
            {t('designer.project.open')}
          </Button>
          <Button icon="folder" onClick={() => void browse()}>
            {t('designer.project.browse')}
          </Button>
        </div>
      </div>
    </Panel>
  );
}
