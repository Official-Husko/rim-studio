import { useEffect } from 'preact/hooks';
import { Banner, Button, Panel, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { devLink } from '../devLinks';
import { LinkCommand } from './LinkCommand';
import { LinkConfirmDialog } from './LinkConfirmDialog';
import { LinkStatusLine } from './LinkStatusLine';
import { LinkSteps } from './LinkSteps';
import { activeLabel, wantsManualCommand } from './linkLabels';
import {
  askToLink,
  cancelLink,
  confirmLink,
  linkBusy,
  linkConfirming,
  linkError,
  linkNotice,
  linkStatus,
  loadLinkStatus,
  resetLinkStore,
  unlink,
} from './linkStore';

export interface LinkCardProps {
  projectId: string;
  /** The project folder, shown in the confirmation. */
  projectPath: string;
  /** The mod's display name. */
  projectName: string;
}

/** "Test in RimWorld": makes the project visible to the game and says how to find it there. */
export function LinkCard({ projectId, projectPath, projectName }: LinkCardProps) {
  useEffect(() => {
    resetLinkStore();
    void loadLinkStatus(projectId).then(() => {
      if (devLink('link') === 'confirm' && linkStatus.peek()?.canCreate) askToLink();
    });
    return resetLinkStore;
  }, [projectId]);

  const status = linkStatus.value;
  const busy = linkBusy.value;
  const notice = linkNotice.value;
  const error = linkError.value;
  const refused = notice?.kind === 'refused';

  return (
    <Panel
      title={t('project.link.title')}
      framed
      collapsible
      actions={
        <Button
          size="sm"
          variant="ghost"
          icon="refresh"
          loading={busy === 'loading'}
          onClick={() => void loadLinkStatus(projectId)}
        >
          {t('project.link.refresh')}
        </Button>
      }
    >
      <div class="flex flex-col gap-3 p-3">
        {error ? (
          <Banner tone="error" title={t('project.link.error')}>
            {error.message}
          </Banner>
        ) : null}
        {!status && !error ? <Spinner label={t('project.link.loading')} /> : null}
        {status ? (
          <>
            <div class="flex flex-wrap items-start gap-3">
              <LinkStatusLine status={status} />
              <div class="flex shrink-0 items-center gap-2">
                {status.canCreate ? (
                  <Button
                    variant="primary"
                    icon="link"
                    loading={busy === 'creating'}
                    disabled={busy !== 'idle'}
                    onClick={askToLink}
                  >
                    {t('project.link.action.link')}
                  </Button>
                ) : null}
                {status.canRemove ? (
                  <Button
                    variant="secondary"
                    loading={busy === 'removing'}
                    disabled={busy !== 'idle'}
                    onClick={() => void unlink(projectId)}
                  >
                    {status.state === 'copy'
                      ? t('project.link.action.removeCopy')
                      : t('project.link.action.remove')}
                  </Button>
                ) : null}
              </div>
            </div>
            {notice?.kind === 'created' ? (
              <Banner tone="success">{t('project.link.created')}</Banner>
            ) : null}
            {notice?.kind === 'removed' ? (
              <Banner tone="success">{t('project.link.removed')}</Banner>
            ) : null}
            {notice?.kind === 'refused' ? (
              <Banner tone="warning" title={t('project.link.refused')}>
                {notice.refusal?.message}
              </Banner>
            ) : null}
            {status.gameRunning === 'running' && status.canCreate ? (
              <Banner tone="info">{t('project.link.running')}</Banner>
            ) : null}
            {wantsManualCommand(status, refused) && status.manualCommand ? (
              <LinkCommand command={status.manualCommand} />
            ) : null}
            <dl class="m-0 flex flex-wrap gap-x-6 gap-y-1 text-small">
              <div class="flex gap-2">
                <dt class="text-muted">{t('project.link.active.label')}</dt>
                <dd class="m-0 text-fg">{activeLabel(status.activeInGame)}</dd>
              </div>
              {status.hasCePatch ? (
                <div class="flex gap-2">
                  <dt class="text-muted">{t('project.link.ce.label')}</dt>
                  <dd class="m-0 text-fg">{activeLabel(status.ceActiveInGame)}</dd>
                </div>
              ) : null}
            </dl>
            <LinkSteps name={projectName} hasCePatch={status.hasCePatch} />
            <LinkConfirmDialog
              open={linkConfirming.value}
              status={status}
              projectPath={projectPath}
              onCancel={cancelLink}
              onConfirm={(mode, running) => void confirmLink(projectId, mode, running)}
            />
          </>
        ) : null}
      </div>
    </Panel>
  );
}
