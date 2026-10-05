import { Banner, Button, Panel, ProgressBar } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { jobs } from '~/shared/ipc';
import { ScanResults } from './ScanResults';
import { runScan, scanError, scanResult, scanning } from './scanStore';

/** Card 3: scan the library and read the result. Progress also shows in the task centre. */
export function ScanCard() {
  const busy = scanning.value;
  const job = jobs.value.find((j) => j.command === 'library_scan' && j.state === 'running');
  const result = scanResult.value;
  const fraction = job && job.total ? job.done / job.total : undefined;
  return (
    <Panel
      title={t('setup.scan.title')}
      framed
      actions={
        <>
          <Button
            size="sm"
            icon="play"
            loading={busy === 'scan'}
            disabled={busy === 'full'}
            onClick={() => void runScan(false)}
          >
            {t('setup.scan.run')}
          </Button>
          <Button
            size="sm"
            variant="secondary"
            icon="refresh"
            loading={busy === 'full'}
            disabled={busy === 'scan'}
            onClick={() => void runScan(true)}
          >
            {t('setup.scan.full')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-4">
        <p class="m-0 text-small text-muted">{t('setup.scan.hint')}</p>
        {busy ? (
          <ProgressBar
            label={t('setup.scan.title')}
            value={fraction}
            caption={job?.message || t('setup.scan.running')}
          />
        ) : null}
        {scanError.value ? (
          <Banner tone="error" title={t('setup.scan.error')}>
            {scanError.value.message}
          </Banner>
        ) : null}
        {result?.cancelled ? <Banner tone="warning">{t('setup.scan.cancelled')}</Banner> : null}
        {result ? (
          <ScanResults result={result} />
        ) : busy ? null : (
          <p class="m-0 text-muted">{t('setup.scan.none')}</p>
        )}
      </div>
    </Panel>
  );
}
