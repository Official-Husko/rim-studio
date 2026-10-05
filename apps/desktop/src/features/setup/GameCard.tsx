import { Banner, Button, EmptyState, Panel, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { DetectionDetails } from './DetectionDetails';
import { detectAgain, detectError, detecting, report, reportLoaded } from './detectStore';
import { PathOverrides } from './PathOverrides';
import { WarningList } from './WarningList';

/** Card 1: what detection found about the game and Steam, with the overrides. */
export function GameCard() {
  const current = report.value;
  const busy = detecting.value;
  return (
    <Panel
      title={t('setup.game.title')}
      framed
      actions={
        <Button size="sm" icon="refresh" loading={busy} onClick={() => void detectAgain()}>
          {t('setup.game.detect')}
        </Button>
      }
    >
      <div class="flex flex-col gap-4">
        {detectError.value ? (
          <Banner tone="error" title={t('setup.game.error')}>
            {detectError.value.message}
          </Banner>
        ) : null}
        {current ? (
          <>
            {current.installs.length === 0 ? (
              <Banner tone="warning" title={t('setup.game.noinstall')}>
                {t('setup.game.noinstall.hint')}
              </Banner>
            ) : null}
            <DetectionDetails report={current} />
            <WarningList warnings={current.warnings} />
            <div class="flex flex-col gap-2 border-t border-line-subtle pt-3">
              <h3 class="m-0 text-small font-semibold text-muted">{t('setup.override.title')}</h3>
              <p class="m-0 text-small text-muted">{t('setup.override.hint')}</p>
              <PathOverrides report={current} />
            </div>
          </>
        ) : !reportLoaded.value ? (
          <Spinner label={t('app.loading')} />
        ) : (
          <EmptyState
            compact
            icon="search"
            title={t('setup.game.first.title')}
            description={t('setup.game.first.hint')}
          />
        )}
      </div>
    </Panel>
  );
}
