import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { pickFolder } from '~/shared/platform';
import { currentProject, projectRevision, setCurrentProject } from '~/shared/project';
import { devLink } from './devLinks';
import { ModHub } from './hub/ModHub';
import { ModView } from './ModView';
import { NewModDialog } from './NewModDialog';
import { loadError, loadProject, loadState, openFolder, view } from './store';

/** The Mod page: a hub to create or open a mod, and for an open mod its basics, folders, files, layout and game link. */
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
      {!current ? <ModHub onCreate={() => setCreatingMod(true)} /> : null}
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
        <ModView
          key={shown.summary.projectId}
          view={shown}
          refreshing={loadState.value === 'loading'}
          onRefresh={() => void loadProject(current)}
          onOpen={() => void chooseFolder()}
          onNew={() => setCreatingMod(true)}
          onClose={() => setCurrentProject(undefined)}
        />
      ) : null}
      <NewModDialog
        open={creatingMod}
        startParent={devLink('parent')}
        onClose={() => setCreatingMod(false)}
      />
    </div>
  );
}
