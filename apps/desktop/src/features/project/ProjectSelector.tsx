import { useEffect, useState } from 'preact/hooks';
import { Icon, Select, type SelectOption } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { normalizeError, transportKind } from '~/shared/ipc';
import {
  currentProject,
  openProjectAt,
  recentProjects,
  restoreCurrentProject,
} from '~/shared/project';

/**
 * The project selector of the top bar: shows the current project and switches between the recent
 * ones. A link leads to the Project page for choosing a folder or creating a mod.
 */
export function ProjectSelector() {
  const [problem, setProblem] = useState<string | undefined>(undefined);
  // The top bar is drawn before the transport is chosen; opening the last project needs it.
  const ready = transportKind.value !== 'none';
  useEffect(() => {
    if (ready) void restoreCurrentProject();
  }, [ready]);

  const current = currentProject.value;
  const options: SelectOption[] = [];
  if (current && !recentProjects.value.some((r) => r.path === current.path)) {
    options.push({ value: current.path, label: current.name });
  }
  for (const item of recentProjects.value) options.push({ value: item.path, label: item.name });

  const switchTo = async (path: string): Promise<void> => {
    setProblem(undefined);
    try {
      await openProjectAt(path);
    } catch (thrown) {
      setProblem(normalizeError(thrown).message);
    }
  };

  return (
    <div class="flex min-w-0 items-center gap-3">
      <span class="text-muted">
        <Icon name="folder" />
      </span>
      {options.length > 0 ? (
        <Select
          aria-label={t('project.selector.label')}
          value={current?.path}
          placeholder={t('project.slot.none')}
          options={options}
          onValueChange={(path) => void switchTo(path)}
        />
      ) : (
        <span class="text-muted">{t('project.slot.none')}</span>
      )}
      <a
        href="#/project"
        class="rounded-sm text-small text-accent underline-offset-2 hover:underline"
      >
        {t('project.selector.manage')}
      </a>
      {problem ? (
        <span role="alert" class="min-w-0 truncate text-small text-danger" title={problem}>
          {problem}
        </span>
      ) : null}
    </div>
  );
}
