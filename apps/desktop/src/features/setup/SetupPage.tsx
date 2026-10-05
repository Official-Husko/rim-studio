import { useState } from 'preact/hooks';
import { Banner, Button, KeyValueList, Panel } from 'rimstudio-ui';
import { connection, devInfo, useQuery } from '~/shared/ipc';
import { pickFolder } from '~/shared/platform';
import { t } from '~/shared/i18n';

/** Placeholder setup page: shows what the bridge reports and proves the folder picker. */
export default function SetupPage() {
  const info = useQuery('dev-info', devInfo);
  const [picked, setPicked] = useState<string | null>(null);
  const data = info.data.value;
  return (
    <div class="flex max-w-3xl flex-col gap-4 p-6">
      <h1 class="font-display text-display font-semibold tracking-display">{t('setup.title')}</h1>
      {connection.value === 'mock' ? <Banner tone="info">{t('setup.mock.note')}</Banner> : null}
      <Panel title={t('setup.connection')} framed>
        {info.error.value ? (
          <Banner tone="error" title={t('setup.error')}>
            {info.error.value.message}
          </Banner>
        ) : data ? (
          <KeyValueList
            items={[
              { key: t('setup.info.version'), value: data.bridgeVersion, mono: true },
              { key: t('setup.info.platform'), value: data.platform, mono: true },
              { key: t('setup.info.home'), value: data.home, mono: true },
              { key: t('setup.info.data'), value: data.dataDir, mono: true },
              { key: t('setup.info.commands'), value: String(data.commandCount), mono: true },
            ]}
          />
        ) : null}
      </Panel>
      <div class="flex items-center gap-3">
        <Button icon="folder" onClick={() => void pickFolder().then(setPicked)}>
          {t('setup.pick')}
        </Button>
        {picked ? (
          <span class="font-mono text-mono text-muted">{t('setup.picked', { path: picked })}</span>
        ) : null}
      </div>
    </div>
  );
}
