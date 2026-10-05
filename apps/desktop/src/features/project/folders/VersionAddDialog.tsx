import { useEffect, useState } from 'preact/hooks';
import { Banner, Button, Checkbox, Dialog, FormField, TextField } from 'rimstudio-ui';
import type { ApiError, ProjectVersionAddDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { normalizeError } from '~/shared/ipc';
import { bumpProjectRevision } from '~/shared/project';
import { FieldFindings } from '../basics/FieldFindings';
import { versionAdd } from '../modApi';
import { loadFolders } from './folderStore';

export interface VersionAddDialogProps {
  open: boolean;
  projectId: string;
  onClose: () => void;
}

/** Create the folder of one more game version, with its standard folders and its block. */
export function VersionAddDialog({ open, projectId, onClose }: VersionAddDialogProps) {
  const [version, setVersion] = useState('');
  const [standard, setStandard] = useState(true);
  const [block, setBlock] = useState(true);
  const [plan, setPlan] = useState<ProjectVersionAddDto | undefined>(undefined);
  const [error, setError] = useState<ApiError | undefined>(undefined);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setVersion('');
      setPlan(undefined);
      setError(undefined);
    }
  }, [open]);

  // the dry run says what the call would do; it runs while the person types
  useEffect(() => {
    if (!open || version.trim() === '') {
      setPlan(undefined);
      return undefined;
    }
    let live = true;
    const timer = setTimeout(() => {
      versionAdd(projectId, version.trim(), {
        standardFolders: standard,
        addBlock: block,
        dryRun: true,
      }).then(
        (answer) => {
          if (!live) return;
          setPlan(answer);
          setError(undefined);
        },
        (thrown) => {
          if (!live) return;
          setPlan(undefined);
          setError(normalizeError(thrown));
        },
      );
    }, 250);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [open, projectId, version, standard, block]);

  const create = async (): Promise<void> => {
    setBusy(true);
    try {
      await versionAdd(projectId, version.trim(), {
        standardFolders: standard,
        addBlock: block,
        dryRun: false,
      });
      bumpProjectRevision();
      await loadFolders(projectId);
      onClose();
    } catch (thrown) {
      setError(normalizeError(thrown));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      title={t('project.folders.addVersion.title')}
      onClose={onClose}
      closeLabel={t('project.basics.close')}
      footer={
        <>
          <Button variant="secondary" onClick={onClose}>
            {t('project.basics.cancel')}
          </Button>
          <Button disabled={!plan} loading={busy} onClick={() => void create()}>
            {t('project.folders.addVersion.create')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-3">
        <FormField
          label={t('project.folders.addVersion.version')}
          help={t('project.folders.addVersion.help')}
        >
          <TextField value={version} onValueChange={setVersion} placeholder="1.6" />
        </FormField>
        <Checkbox checked={standard} onCheckedChange={setStandard}>
          {t('project.folders.addVersion.standard')}
        </Checkbox>
        <Checkbox checked={block} onCheckedChange={setBlock}>
          {t('project.folders.addVersion.block')}
        </Checkbox>
        {error ? <Banner tone="error">{error.message}</Banner> : null}
        {plan ? (
          <div
            class="flex flex-col gap-2"
            aria-label={t('project.folders.addVersion.plan')}
            role="region"
          >
            <p class="m-0 text-small text-muted">
              {plan.createdFolders.length > 0
                ? t('project.folders.addVersion.folders', {
                    n: plan.createdFolders.length,
                    folder: plan.folder,
                  })
                : t('project.folders.addVersion.foldersNone', { folder: plan.folder })}
            </p>
            {plan.createdFolders.length > 0 ? (
              <ul class="m-0 max-h-40 list-none overflow-auto p-0 font-mono text-mono-small">
                {plan.createdFolders.map((f) => (
                  <li key={f}>{f}</li>
                ))}
              </ul>
            ) : null}
            <p class="m-0 text-small text-muted">
              {plan.blockAdded
                ? t('project.folders.addVersion.blockAdded')
                : t('project.folders.addVersion.blockNone')}
            </p>
            {plan.notInSupportedVersions ? (
              <Banner tone="warning">
                {t('project.folders.addVersion.unsupported', { version: plan.version })}
              </Banner>
            ) : null}
            <FieldFindings items={plan.diagnostics} />
          </div>
        ) : null}
      </div>
    </Dialog>
  );
}
