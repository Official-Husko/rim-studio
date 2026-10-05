import { Badge, Banner, Button, KeyValueList, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { CE_PACKAGE_ID, SOURCE_KIND_KEYS } from './model';
import { runScan, scanResult, scanning } from './scanStore';

/**
 * Card 4: whether Combat Extended is in the scanned library, read from the scan result, and what that
 * means for the patch generator.
 */
export function CeCard() {
  const found = scanResult.value?.ceInLibrary;
  return (
    <Panel title={t('setup.ce.title')} framed>
      <div class="flex flex-col gap-3">
        <p class="m-0 text-small text-muted">{t('setup.ce.hint')}</p>
        {found === undefined ? (
          <div class="flex flex-wrap items-center gap-3">
            <p class="m-0 flex-1 text-muted">{t('setup.ce.unknown')}</p>
            <Button
              size="sm"
              variant="secondary"
              icon="play"
              loading={scanning.value !== undefined}
              onClick={() => void runScan(false)}
            >
              {t('setup.ce.scan')}
            </Button>
          </div>
        ) : null}
        {found?.present ? (
          <KeyValueList
            label={t('setup.ce.title')}
            items={[
              {
                key: t('setup.ce.status'),
                value: <Badge tone="success">{t('setup.ce.found')}</Badge>,
              },
              ...(found.name ? [{ key: t('setup.ce.name'), value: found.name }] : []),
              ...(found.version
                ? [{ key: t('setup.ce.version'), value: found.version, mono: true }]
                : []),
              { key: t('setup.ce.package'), value: found.packageId ?? CE_PACKAGE_ID, mono: true },
              ...(found.sourceKind
                ? [{ key: t('setup.ce.source'), value: t(SOURCE_KIND_KEYS[found.sourceKind]) }]
                : []),
              ...(found.path ? [{ key: t('setup.ce.path'), value: found.path, mono: true }] : []),
              ...(found.loadable === false
                ? [{ key: t('setup.ce.loadable'), value: t('setup.ce.not-loadable') }]
                : []),
            ]}
          />
        ) : null}
        {found && !found.present ? (
          <Banner tone="info" title={t('setup.ce.missing')}>
            {t('setup.ce.missing.hint', { id: CE_PACKAGE_ID })}
          </Banner>
        ) : null}
      </div>
    </Panel>
  );
}
