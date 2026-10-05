import { useEffect, useRef, useState } from 'preact/hooks';
import type { DraftEntryDto, ItemKindDto, ReferenceItemDto } from 'rimstudio-ipc-types';
import { EmptyState } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { DefinitionDialog } from './components/DefinitionDialog';
import { DraftList } from './components/DraftList';
import { Editor } from './components/editor/Editor';
import { NewDraftDialog } from './components/NewDraftDialog';
import { OutputPanel } from './components/output/OutputPanel';
import { ProjectPrompt } from './components/ProjectPrompt';
import { ReferenceBrowser } from './components/ReferenceBrowser';
import { applyDevLinks, readDevLinks } from './dev-links';
import { currentProject } from './project-source';
import { designer, type Designer } from './stores';

type Dialog =
  { type: 'new'; kind: ItemKindDto } | { type: 'clone'; item: ReferenceItemDto } | undefined;

export interface DesignerPageProps {
  /** The stores to use; the running app uses the shared instance. */
  stores?: Designer;
}

/**
 * The Weapons page. Left: the drafts of the project and the reference weapons of the install.
 * Centre: the editor of the open draft with its live results. Right: the output of the draft.
 */
export default function DesignerPage({ stores = designer }: DesignerPageProps) {
  const project = currentProject();
  const projectId = project?.projectId;
  const [dialog, setDialog] = useState<Dialog>(undefined);
  const [definition, setDefinition] = useState<string | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const { drafts, editor, reference } = stores;

  useEffect(() => {
    void reference.load();
  }, [reference]);

  useEffect(() => {
    if (!import.meta.env.DEV) return;
    const links = readDevLinks(location.hash);
    if (!links.project && !links.def) return;
    void applyDevLinks(stores, links).then((def) => def && setDefinition(def));
  }, [stores]);

  const previous = useRef<string | undefined>(undefined);
  useEffect(() => {
    // switching from one project to another drops the open draft; the first project keeps it
    if (previous.current !== undefined && previous.current !== projectId) stores.reset();
    previous.current = projectId;
    if (projectId) void drafts.load();
  }, [projectId, stores, drafts]);

  const finish = async (entry: DraftEntryDto | undefined): Promise<void> => {
    setBusy(false);
    if (!entry) return;
    setDialog(undefined);
    await stores.select(entry);
  };

  const create = async (kind: ItemKindDto, defName: string, label: string): Promise<void> => {
    setBusy(true);
    await finish(await drafts.create(kind, defName, label));
  };

  const clone = async (
    source: string,
    defName: string,
    label: string,
    ownProjectile: boolean,
  ): Promise<void> => {
    setBusy(true);
    await finish(
      await drafts.clone(source, defName, label === '' ? undefined : label, ownProjectile),
    );
  };

  const addAnchor = (item: ReferenceItemDto): void => {
    editor.update((d) => {
      const anchors = d.anchors ?? [];
      if (anchors.some((a) => a.defName === item.defName)) return d;
      return { ...d, anchors: [...anchors, { defName: item.defName, label: item.label }] };
    });
  };

  const openDraft = editor.draft.value;
  return (
    <div class="flex h-full min-h-0">
      <h1 class="sr-only">{t('nav.weapons')}</h1>
      <aside
        class="flex w-96 shrink-0 flex-col gap-3 overflow-x-hidden overflow-y-auto border-r border-line bg-bg p-3"
        aria-label={t('designer.region.left')}
      >
        {project ? (
          <DraftList
            store={drafts}
            openId={editor.entryId.value}
            openState={editor.saveState.value}
            onOpen={(entry) => void stores.select(entry)}
            onNew={(kind) => setDialog({ type: 'new', kind })}
            onDelete={(entry) => void stores.remove(entry.id)}
          />
        ) : (
          <ProjectPrompt onOpened={() => undefined} />
        )}
        <ReferenceBrowser
          store={reference}
          canClone={project !== undefined}
          canAnchor={openDraft !== undefined}
          onClone={(item) => setDialog({ type: 'clone', item })}
          onAnchor={addAnchor}
          onShowDefinition={(item) => setDefinition(item.defName)}
        />
      </aside>
      <div class="flex min-w-0 flex-1 flex-col overflow-y-auto xl:flex-row xl:overflow-hidden">
        <section
          class="min-w-0 shrink-0 overflow-x-hidden xl:flex-1 xl:shrink xl:overflow-y-auto"
          aria-label={t('designer.region.centre')}
        >
          {openDraft ? (
            <Editor key={editor.entryId.value} designer={stores} />
          ) : (
            <div class="p-6">
              <EmptyState
                icon="crosshair"
                title={t('designer.editor.emptyTitle')}
                description={t('designer.editor.emptyBody')}
              />
            </div>
          )}
        </section>
        <section
          class="shrink-0 border-t border-line p-3 xl:w-96 xl:overflow-y-auto xl:border-t-0 xl:border-l"
          aria-label={t('designer.region.right')}
        >
          <OutputPanel
            draftId={editor.entryId.value}
            projectPath={project?.path}
            store={stores.output}
            editor={editor}
          />
        </section>
      </div>
      {dialog?.type === 'new' ? (
        <NewDraftDialog
          key={`new-${dialog.kind}`}
          open
          title={
            dialog.kind === 'ranged'
              ? t('designer.dialog.newRanged')
              : t('designer.dialog.newMelee')
          }
          defName=""
          label=""
          busy={busy}
          error={drafts.error.value}
          onClose={() => setDialog(undefined)}
          onSubmit={(defName, label) => void create(dialog.kind, defName, label)}
        />
      ) : null}
      {dialog?.type === 'clone' ? (
        <NewDraftDialog
          key={`clone-${dialog.item.defName}`}
          open
          title={t('designer.dialog.clone', { name: dialog.item.label })}
          defName={`${dialog.item.defName}_Copy`}
          label={`${dialog.item.label} copy`}
          busy={busy}
          error={drafts.error.value}
          onClose={() => setDialog(undefined)}
          offerOwnProjectile={reference.kind.value === 'ranged'}
          onSubmit={(defName, label, own) => void clone(dialog.item.defName, defName, label, own)}
        />
      ) : null}
      <DefinitionDialog defName={definition} onClose={() => setDefinition(undefined)} />
    </div>
  );
}
