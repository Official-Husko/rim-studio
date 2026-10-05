import { useEffect, useState } from 'preact/hooks';
import type { ApiError, ProjectFileDto } from 'rimstudio-ipc-types';
import { Banner, CodeView, Spinner } from 'rimstudio-ui';
import { normalizeError } from '~/shared/ipc';
import { t } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { readProjectFile } from './api';
import { withOpenProject } from './session';

export interface DefinitionViewProps {
  project: ProjectRef;
  /** Project relative path of the file that holds the weapon. */
  file: string | undefined;
}

/** The definition file of the weapon as it is in the project (read only). */
export function DefinitionView({ project, file }: DefinitionViewProps) {
  const [loaded, setLoaded] = useState<ProjectFileDto | undefined>(undefined);
  const [error, setError] = useState<ApiError | undefined>(undefined);

  useEffect(() => {
    setLoaded(undefined);
    setError(undefined);
    if (!file) return;
    let live = true;
    withOpenProject(project, (id) => readProjectFile(id, file))
      .then((dto) => live && setLoaded(dto))
      .catch((thrown) => live && setError(normalizeError(thrown)));
    return () => {
      live = false;
    };
    // the project is read through its path; a new object for the same path must not reload the file
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [project.path, file]);

  if (!file) return <Banner tone="info">{t('patches.definition.none')}</Banner>;
  if (error)
    return (
      <Banner tone="error" title={error.code}>
        {error.message}
      </Banner>
    );
  if (!loaded) return <Spinner label={t('patches.definition.loading')} />;
  return (
    <div class="flex flex-col gap-2">
      <code class="font-mono text-mono text-muted">{loaded.path}</code>
      {loaded.truncated ? (
        <Banner tone="warning">{t('patches.definition.truncated')}</Banner>
      ) : null}
      <CodeView
        language="xml"
        code={loaded.text}
        label={t('patches.definition.label', { path: loaded.path })}
        lineNumbers
        heightClass="max-h-96"
      />
    </div>
  );
}
