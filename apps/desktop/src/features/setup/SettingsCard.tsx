import { Banner, KeyValueList, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { devInfo, useQuery } from '~/shared/ipc';
import { settings, settingsError } from './settingsStore';

/** Card 5: where the settings live and a read only summary of them. */
export function SettingsCard() {
  const info = useQuery('setup-dev-info', devInfo);
  const current = settings.value;
  const dataDir = info.data.value?.dataDir;
  return (
    <Panel title={t('setup.settings.title')} framed>
      <div class="flex flex-col gap-3">
        <p class="m-0 text-small text-muted">{t('setup.settings.hint')}</p>
        {settingsError.value ? (
          <Banner tone="error" title={t('setup.settings.error')}>
            {settingsError.value.message}
          </Banner>
        ) : null}
        <KeyValueList
          label={t('setup.settings.title')}
          items={[
            {
              key: t('setup.settings.file'),
              value: dataDir
                ? `${dataDir}/config/settings.jsonc`
                : t('setup.settings.file.unknown'),
              mono: true,
            },
            ...(current
              ? [
                  { key: t('setup.settings.theme'), value: current.appearance.theme },
                  { key: t('setup.settings.density'), value: current.appearance.density },
                  {
                    key: t('setup.settings.language'),
                    value: current.language ?? t('setup.settings.language.system'),
                  },
                  {
                    key: t('setup.settings.threads'),
                    value:
                      current.library.scanThreads === 0
                        ? t('setup.settings.threads.auto')
                        : String(current.library.scanThreads),
                  },
                  { key: t('setup.settings.watch'), value: current.library.watch.mode },
                  { key: t('setup.settings.designer'), value: current.designer.mode },
                  { key: t('setup.settings.log'), value: current.logLevel },
                  {
                    key: t('setup.settings.folders'),
                    value: String(current.customFolders.length),
                    mono: true,
                  },
                ]
              : []),
          ]}
        />
      </div>
    </Panel>
  );
}
