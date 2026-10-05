import { EmptyState, IconButton, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { forgetRecent, recentProjects } from '~/shared/project';
import { openFolder } from './store';

/** The recent projects, newest first; a row opens the project, the small button forgets it. */
export function RecentList() {
  const list = recentProjects.value;
  return (
    <Panel title={t('project.recent.title')} framed>
      {list.length === 0 ? (
        <EmptyState
          compact
          icon="folder"
          title={t('project.recent.empty.title')}
          description={t('project.recent.empty.hint')}
        />
      ) : (
        <ul
          class="m-0 list-none divide-y divide-line-subtle p-0"
          aria-label={t('project.recent.list')}
        >
          {list.map((item) => (
            <li key={item.path} class="flex items-center gap-1 pr-2">
              <button
                type="button"
                class="flex min-w-0 flex-1 cursor-pointer flex-col items-start gap-0.5 bg-transparent px-3 py-2 text-left text-fg hover:bg-hover"
                onClick={() => void openFolder(item.path)}
              >
                <span class="max-w-full truncate font-semibold">{item.name}</span>
                <span class="max-w-full truncate font-mono text-mono-small text-faint">
                  {item.packageId ? `${item.packageId}  ${item.path}` : item.path}
                </span>
              </button>
              <IconButton
                icon="close"
                label={t('project.recent.forget', { name: item.name })}
                onClick={() => forgetRecent(item.path)}
              />
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}
