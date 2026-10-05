import { useEffect } from 'preact/hooks';
import { Banner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { connection } from '~/shared/ipc';
import { CeCard } from './CeCard';
import { loadReport } from './detectStore';
import { devLink } from './devLinks';
import { GameCard } from './GameCard';
import { ScanCard } from './ScanCard';
import { runScan } from './scanStore';
import { loadSettings } from './settingsStore';
import { SettingsCard } from './SettingsCard';
import { SourcesCard } from './SourcesCard';
import { loadSources, startAdd } from './sourcesStore';

/** Setup: detection, mod folders, library scan, Combat Extended and a settings summary. */
export default function SetupPage() {
  useEffect(() => {
    void (async () => {
      await loadReport();
      await Promise.all([loadSources(), loadSettings()]);
      const addPath = devLink('add');
      if (addPath) await startAdd(addPath);
      if (devLink('scan') === 'run') await runScan(false);
    })();
  }, []);
  return (
    <div class="flex max-w-7xl flex-col gap-4 p-6">
      <h1 class="font-display text-display font-semibold tracking-display">{t('setup.title')}</h1>
      {connection.value === 'mock' ? <Banner tone="info">{t('setup.mock.note')}</Banner> : null}
      <div class="grid grid-cols-1 items-start gap-4 xl:grid-cols-2">
        <div class="flex min-w-0 flex-col gap-4">
          <GameCard />
          <CeCard />
          <SettingsCard />
        </div>
        <div class="flex min-w-0 flex-col gap-4">
          <SourcesCard />
          <ScanCard />
        </div>
      </div>
    </div>
  );
}
