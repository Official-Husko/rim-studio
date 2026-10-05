import { useEffect, useState } from 'preact/hooks';
import { Badge, Banner, Button, Dialog, FormField, Spinner, TextField } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { PROBE_KEYS } from './codes';
import { adding, cancelAdd, confirmAdd, pending } from './sourcesStore';

const KIND_KEYS = {
  missing: 'setup.probe.kind.missing',
  'not-directory': 'setup.probe.kind.not-directory',
  'game-data': 'setup.probe.kind.game-data',
  'game-mods': 'setup.probe.kind.game-mods',
  workshop: 'setup.probe.kind.workshop',
  'single-mod': 'setup.probe.kind.single-mod',
  'mods-root': 'setup.probe.kind.mods-root',
  empty: 'setup.probe.kind.empty',
} as const;

const WARNING_KEYS = {
  'no-mods-found': 'setup.probe.warn.no-mods-found',
  'mods-found-deeper': 'setup.probe.warn.mods-found-deeper',
  'ambiguous-layout': 'setup.probe.warn.ambiguous-layout',
  'path-is-link': 'setup.probe.warn.path-is-link',
  unreachable: 'setup.probe.warn.unreachable',
} as const;

/** The add folder dialog: shows what the probe found, then adds on confirmation. */
export function AddFolderDialog() {
  const current = pending.value;
  const [label, setLabel] = useState('');
  useEffect(() => {
    if (current?.probing) setLabel('');
  }, [current?.path, current?.probing]);
  const probe = current?.probe;
  return (
    <Dialog
      open={current !== undefined}
      title={t('setup.add.title')}
      size="lg"
      onClose={cancelAdd}
      closeLabel={t('setup.dialog.close')}
      footer={
        <>
          <Button variant="secondary" onClick={cancelAdd}>
            {t('setup.dialog.cancel')}
          </Button>
          <Button
            disabled={!probe?.canSave}
            loading={adding.value}
            onClick={() => void confirmAdd(label)}
          >
            {t('setup.add.confirm')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-3">
        <p class="m-0 font-mono text-mono break-all">{current?.path}</p>
        {current?.probing ? <Spinner label={t('setup.add.probing')} /> : null}
        {current?.error ? (
          <Banner tone="error" title={t('setup.add.error')}>
            {current.error.message}
          </Banner>
        ) : null}
        {probe ? (
          <>
            <div class="flex flex-wrap items-center gap-2">
              <Badge tone={probe.canSave ? 'success' : 'danger'}>{t(KIND_KEYS[probe.kind])}</Badge>
              <span class="text-muted">{tn('setup.add.count', probe.modCount)}</span>
              <span class="text-muted">{t('setup.add.depth', { n: probe.suggestedDepth })}</span>
            </div>
            {probe.warnings.map((warning) => (
              <Banner key={warning} tone="warning">
                {t(WARNING_KEYS[warning])}
              </Banner>
            ))}
            {probe.diagnostics.map((diag) => {
              const key = PROBE_KEYS[diag.code];
              return (
                <Banner key={diag.code} tone={diag.severity === 'error' ? 'error' : 'warning'}>
                  {key ? t(key) : diag.message}
                </Banner>
              );
            })}
            {probe.canSave ? (
              <FormField label={t('setup.add.label')} help={t('setup.add.label.hint')}>
                <TextField value={label} onValueChange={setLabel} />
              </FormField>
            ) : (
              <Banner tone="error">{t('setup.add.blocked')}</Banner>
            )}
          </>
        ) : null}
      </div>
    </Dialog>
  );
}
