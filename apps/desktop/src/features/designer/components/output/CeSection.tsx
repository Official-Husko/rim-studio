import type { DesignSpecDto } from 'rimstudio-ipc-types';
import { Banner, Panel, Spinner, Switch } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { folderOf } from '../../output-model';
import type { OutputStore } from '../../output-store';
import { CeBody } from './CeBody';

export interface CeSectionProps {
  store: OutputStore;
  spec: DesignSpecDto;
  onGoTo: (pointer: string) => void;
}

/**
 * The optional Combat Extended patch. It is off until the user turns it on for this draft; the
 * weapon is always written as a vanilla definition either way. When Combat Extended is not loaded
 * the plain reason is shown and the switch stays off.
 */
export function CeSection({ store, spec, onGoTo }: CeSectionProps) {
  const on = store.ceOn.value;
  const suggestion = store.suggestion.value;
  const unavailable = suggestion !== undefined && !suggestion.available;
  const folder = store.plan.value?.files.find((f) => f.kind === 'ce-patch');
  return (
    <Panel title={t('designer.output.ce.title')} framed>
      <div class="flex flex-col gap-3">
        <Switch checked={on} onCheckedChange={store.setCeEnabled} disabled={unavailable && !on}>
          {t('designer.output.ce.switch')}
        </Switch>
        <p class="text-small text-muted">
          {folder
            ? t('designer.output.ce.explain', { folder: folderOf(folder.path) })
            : t('designer.output.ce.explainNoFolder')}
        </p>
        {unavailable ? (
          <Banner tone="warning" title={t('designer.output.ce.unavailable')}>
            {suggestion.reason ?? t('designer.output.ce.noData')}
          </Banner>
        ) : null}
        {store.suggestError.value ? (
          <Banner tone="error" title={store.suggestError.value.code}>
            {store.suggestError.value.message}
          </Banner>
        ) : null}
        {on && suggestion === undefined && !store.suggestError.value ? (
          <span class="inline-flex items-center gap-2 text-small text-muted">
            <Spinner label={t('designer.output.ce.loading')} />
            {t('designer.output.ce.loading')}
          </span>
        ) : null}
        {on && suggestion?.available ? (
          <CeBody store={store} suggestion={suggestion} spec={spec} onGoTo={onGoTo} />
        ) : null}
        {!on && suggestion?.available ? (
          <p class="text-small text-muted">{t('designer.output.ce.offHelp')}</p>
        ) : null}
      </div>
    </Panel>
  );
}
