import { useState } from 'preact/hooks';
import { Banner, Button, Checkbox, Dialog, SegmentedControl } from 'rimstudio-ui';
import type { ProjectLinkModeDto, ProjectLinkStatusDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';

export interface LinkConfirmDialogProps {
  open: boolean;
  status: ProjectLinkStatusDto;
  /** The project folder the link points at, as shown to the person. */
  projectPath: string;
  onCancel: () => void;
  onConfirm: (mode: ProjectLinkModeDto, confirmGameRunning: boolean) => void;
}

/** Explains what is created and where, and asks for the go ahead. */
export function LinkConfirmDialog({
  open,
  status,
  projectPath,
  onCancel,
  onConfirm,
}: LinkConfirmDialogProps) {
  const [mode, setMode] = useState<ProjectLinkModeDto>(
    !status.support.symlink && status.support.junction ? 'junction' : 'symlink',
  );
  const [restart, setRestart] = useState(false);
  const running = status.gameRunning === 'running';
  const linkModes = [
    ...(status.support.symlink
      ? [{ value: 'symlink', label: t('project.link.confirm.mode.symlink') }]
      : []),
    ...(status.support.junction
      ? [{ value: 'junction', label: t('project.link.confirm.mode.junction') }]
      : []),
    { value: 'copy', label: t('project.link.confirm.mode.copy') },
  ];
  return (
    <Dialog
      open={open}
      title={t('project.link.confirm.title')}
      onClose={onCancel}
      footer={
        <>
          <Button variant="secondary" onClick={onCancel}>
            {t('project.link.confirm.cancel')}
          </Button>
          <Button
            variant="primary"
            disabled={running && !restart}
            onClick={() => onConfirm(mode, restart)}
          >
            {mode === 'copy'
              ? t('project.link.confirm.createCopy')
              : t('project.link.confirm.create')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-3">
        <p class="m-0 text-body">{t('project.link.confirm.intro')}</p>
        <dl class="m-0 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-body">
          <dt class="text-muted">{t('project.link.confirm.entry')}</dt>
          <dd class="m-0 break-all font-mono text-mono">{status.entryPath}</dd>
          <dt class="text-muted">{t('project.link.confirm.points')}</dt>
          <dd class="m-0 break-all font-mono text-mono">{projectPath}</dd>
        </dl>
        <p class="m-0 text-body">{t('project.link.confirm.safe')}</p>
        <p class="m-0 text-body">{t('project.link.confirm.restart')}</p>
        <div class="flex flex-col gap-1">
          <span class="text-small text-muted">{t('project.link.confirm.mode')}</span>
          <SegmentedControl
            label={t('project.link.confirm.mode')}
            options={linkModes}
            value={mode}
            onValueChange={(value) => setMode(value as ProjectLinkModeDto)}
          />
        </div>
        {mode === 'copy' ? (
          <Banner tone="warning">{t('project.link.confirm.copyWarning')}</Banner>
        ) : null}
        {running ? (
          <Banner tone="warning" title={t('project.link.confirm.runningTitle')}>
            <Checkbox checked={restart} onCheckedChange={setRestart}>
              {t('project.link.confirm.runningCheck')}
            </Checkbox>
          </Banner>
        ) : null}
      </div>
    </Dialog>
  );
}
