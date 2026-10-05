import { Badge, Banner, KeyValueList, Panel, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { ce } from './ceStore';
import { CE_PACKAGE_ID } from './model';

/** Card 4: whether Combat Extended is installed, and what that means for the patch generator. */
export function CeCard() {
  const state = ce.value;
  return (
    <Panel title={t('setup.ce.title')} framed>
      <div class="flex flex-col gap-3">
        <p class="m-0 text-small text-muted">{t('setup.ce.hint')}</p>
        {state.status === 'checking' ? <Spinner label={t('setup.ce.checking')} /> : null}
        {state.status === 'found' ? (
          <KeyValueList
            label={t('setup.ce.title')}
            items={[
              {
                key: t('setup.ce.status'),
                value: <Badge tone="success">{t('setup.ce.found')}</Badge>,
              },
              { key: t('setup.ce.package'), value: CE_PACKAGE_ID, mono: true },
              { key: t('setup.ce.path'), value: state.path, mono: true },
            ]}
          />
        ) : null}
        {state.status === 'missing' ? (
          <Banner tone="info" title={t('setup.ce.missing')}>
            {t('setup.ce.missing.hint', { id: CE_PACKAGE_ID })}
          </Banner>
        ) : null}
        {state.status === 'unknown' ? <p class="m-0 text-muted">{t('setup.ce.unknown')}</p> : null}
      </div>
    </Panel>
  );
}
