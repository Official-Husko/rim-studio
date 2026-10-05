import type { ComponentChildren } from 'preact';
import { useRef, useState } from 'preact/hooks';
import { Spinner } from 'rimstudio-ui';
import { transportKind } from '~/shared/ipc';
import { FolderPickerHost } from '~/shared/platform';
import { t } from '~/shared/i18n';
import { ErrorBoundary } from './ErrorBoundary';
import { NavRail } from './NavRail';
import { TaskCentre } from './TaskCentre';
import { ToastHost } from './ToastHost';
import { TopBar } from './TopBar';
import { route, routeParams } from './route';
import { visibleTools } from './tools';

export interface AppShellProps {
  /** Fills the project selector slot of the top bar. */
  projectSlot?: ComponentChildren;
}

/**
 * The application frame: top bar, tool rail, the visible page and the overlay layer. A page is
 * mounted on first visit and stays mounted (hidden) so scroll and form state survive switching.
 */
export function AppShell({ projectSlot }: AppShellProps) {
  const tools = visibleTools();
  const active = tools.some((tool) => tool.id === route.value)
    ? route.value
    : (tools[0]?.id ?? 'setup');
  const visited = useRef(new Set<string>());
  visited.current.add(active);
  const [tasksOpen, setTasksOpen] = useState(() => routeParams.value.get('tasks') === 'open');
  const ready = transportKind.value !== 'none';

  return (
    <div class="flex h-full flex-col bg-bg text-fg">
      <a
        href="#main"
        class="sr-only focus:not-sr-only focus:absolute focus:z-(--rs-z-toast) focus:bg-raised focus:p-2"
        onClick={(e) => {
          e.preventDefault();
          document.getElementById('main')?.focus();
        }}
      >
        {t('app.skip')}
      </a>
      <TopBar
        projectSlot={projectSlot}
        tasksOpen={tasksOpen}
        onToggleTasks={() => setTasksOpen((open) => !open)}
      />
      <div class="flex min-h-0 flex-1">
        <NavRail tools={tools} active={active} />
        <main id="main" tabIndex={-1} class="relative min-w-0 flex-1 overflow-hidden outline-none">
          {!ready ? (
            <div class="flex h-full items-center justify-center text-muted">
              <Spinner size="md" label={t('app.loading')} />
            </div>
          ) : (
            tools
              .filter((tool) => visited.current.has(tool.id))
              .map((tool) => (
                <div key={tool.id} hidden={tool.id !== active} class="h-full overflow-auto">
                  <ErrorBoundary region={t(tool.titleKey)}>
                    <tool.Page />
                  </ErrorBoundary>
                </div>
              ))
          )}
        </main>
        {tasksOpen ? <TaskCentre onClose={() => setTasksOpen(false)} /> : null}
      </div>
      <FolderPickerHost />
      <ToastHost />
    </div>
  );
}
