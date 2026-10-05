import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { currentProject, projectRevision, setCurrentProject } from '~/shared/project';
import { devLink } from './devLinks';
import { NewModDialog } from './NewModDialog';
import { OpenCard } from './OpenCard';
import { ProjectHeader } from './ProjectHeader';
import { RecentList } from './RecentList';
import { Workbench } from './Workbench';
import { loadError, loadProject, loadState, openFolder, view } from './store';

/** Project: choose or create the mod, see its annotated folders, check the layout, read its files. */
export default function ProjectPage() {
  const [creatingMod, setCreatingMod] = useState(() => devLink('new') === '1');
  const current = currentProject.value;
  const revision = projectRevision.value;

  // load when the project changes, when another page changed its files, and when the page is shown again
  useEffect(() => {
    const now = currentProject.peek();
    if (now) void loadProject(now);
  }, [current?.path, revision]);
  useEffect(() => {
    const onHash = (): void => {
      const now = currentProject.peek();
      if (now && window.location.hash.startsWith('#/project')) void loadProject(now);
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  useEffect(() => {
    const path = devLink('open');
    if (path) void openFolder(path);
  }, []);

  const chooseFolder = async (): Promise<void> => {
    const picked = await pickFolder();
    if (picked) await openFolder(picked);
  };

  const shown = view.value;
  return (
    <div class="flex h-full min-h-0 max-w-7xl flex-col gap-4 p-6">
      <div class="flex flex-wrap items-center gap-3">
        <h1 class="m-0 flex-1 font-display text-display font-semibold tracking-display">
          {t('project.title')}
        </h1>
        {current ? null : (
          <Button icon="plus" onClick={() => setCreatingMod(true)}>
            {t('project.new.action')}
          </Button>
        )}
      </div>
      {!current ? (
        <div class="grid grid-cols-1 items-start gap-4 xl:grid-cols-2">
          <OpenCard />
          <RecentList />
        </div>
      ) : null}
      {current && loadState.value === 'error' ? (
        <Banner
          tone="error"
          title={t('project.load.error')}
          action={
            <Button size="sm" variant="secondary" onClick={() => setCurrentProject(undefined)}>
              {t('project.close')}
            </Button>
          }
        >
          {loadError.value?.message}
        </Banner>
      ) : null}
      {current && !shown && loadState.value !== 'error' ? (
        <Spinner label={t('project.loading')} />
      ) : null}
      {current && shown ? (
        <>
          <ProjectHeader
            view={shown}
            refreshing={loadState.value === 'loading'}
            onRefresh={() => void loadProject(current)}
            onOpen={() => void chooseFolder()}
            onNew={() => setCreatingMod(true)}
            onClose={() => setCurrentProject(undefined)}
          />
          <Workbench key={shown.summary.projectId} view={shown} />
        </>
      ) : null}
      <NewModDialog
        open={creatingMod}
        startParent={devLink('parent')}
        onClose={() => setCreatingMod(false)}
      />
    </div>
  );
}
