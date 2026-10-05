import { useState } from 'preact/hooks';
import type { DiagnosticDto, PlannedFileDto } from 'rimstudio-ipc-types';
import { Badge, Banner, Button, CodeView, EmptyState, Spinner } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import type { CustomAmmoStore } from './store';

export interface ReviewSectionProps {
  store: CustomAmmoStore;
}

function severityBadge(d: DiagnosticDto) {
  const tone = d.severity === 'error' ? 'danger' : d.severity === 'warning' ? 'warning' : 'neutral';
  const text =
    d.severity === 'error'
      ? t('ammo.badge.error')
      : d.severity === 'warning'
        ? t('ammo.badge.warning')
        : t('ammo.badge.note');
  return <Badge tone={tone}>{text}</Badge>;
}

function actionText(file: PlannedFileDto): string {
  switch (file.action) {
    case 'create':
      return t('ammo.review.create');
    case 'update-region':
      return t('ammo.review.update');
    case 'unchanged':
      return t('ammo.review.unchanged');
    default:
      return t('ammo.review.replace');
  }
}

/** What will be written and everything the backend found: the files with their XML, and the checks with links to the fields. */
export function ReviewSection({ store }: ReviewSectionProps) {
  const plan = store.plan.value;
  const [picked, setPicked] = useState<string | undefined>(undefined);
  if (!store.hasPlan) {
    return <Banner tone="info">{t('ammo.review.no-project')}</Banner>;
  }
  if (!plan && store.planning.value) {
    return (
      <p class="m-0 flex items-center gap-2 text-small text-muted">
        <Spinner size="sm" label={t('ammo.review.checking')} />
        {t('ammo.review.checking')}
      </p>
    );
  }
  if (store.planError.value) {
    return (
      <Banner tone="error" title={store.planError.value.code}>
        {store.planError.value.message}
      </Banner>
    );
  }
  if (!plan) return <EmptyState compact title={t('ammo.review.not-yet')} />;
  const diagnostics = [...store.diagnostics.value].sort(
    (a, b) => order(a.severity) - order(b.severity),
  );
  const shown = plan.files.find((f) => f.path === picked) ?? store.ammoFile.value ?? plan.files[0];
  return (
    <div class="flex flex-col gap-4">
      <section class="flex flex-col gap-2" aria-label={t('ammo.review.checks')}>
        <h3 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
          {t('ammo.review.checks')}
        </h3>
        {diagnostics.length === 0 ? (
          <p class="m-0 text-small text-success">{t('ammo.review.clean')}</p>
        ) : (
          <ul class="m-0 flex list-none flex-col gap-1 p-0">
            {diagnostics.map((d, i) => (
              <li key={`${d.code}-${i}`} class="flex flex-wrap items-center gap-2 text-small">
                {severityBadge(d)}
                <span class="min-w-0 flex-1">{d.message}</span>
                {d.field?.startsWith('/ce/customAmmo') ? (
                  <Button size="sm" variant="ghost" onClick={() => store.goTo(d.field)}>
                    {t('ammo.review.go')}
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </section>
      {store.ammoFile.value === undefined && store.otherErrors.value.length > 0 ? (
        <Banner tone="warning" title={t('ammo.review.blocked')}>
          {t('ammo.review.blocked-body')}
          <ul class="m-0 mt-1 list-disc pl-5">
            {store.otherErrors.value.slice(0, 4).map((d, i) => (
              <li key={`${d.code}-${i}`}>{d.message}</li>
            ))}
          </ul>
        </Banner>
      ) : null}
      <section class="flex flex-col gap-2" aria-label={t('ammo.review.files')}>
        <h3 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
          {tn('ammo.review.files-count', plan.files.length)}
        </h3>
        <ul class="m-0 flex list-none flex-col gap-1 p-0" aria-label={t('ammo.review.files')}>
          {plan.files.map((file) => (
            <li key={file.path}>
              <button
                type="button"
                aria-pressed={shown?.path === file.path}
                onClick={() => setPicked(file.path)}
                class={
                  shown?.path === file.path
                    ? 'flex w-full flex-wrap items-center gap-2 border border-accent bg-accent-tint px-2 py-1 text-left'
                    : 'flex w-full flex-wrap items-center gap-2 border border-line px-2 py-1 text-left hover:bg-hover'
                }
              >
                <span class="min-w-0 flex-1 truncate font-mono text-mono-small">{file.path}</span>
                {file.kind === 'ce-defs' ? (
                  <Badge tone="info">{t('ammo.review.ammo-file')}</Badge>
                ) : null}
                <Badge>{actionText(file)}</Badge>
                <span class="text-small text-faint">{formatBytes(file.bytes)}</span>
              </button>
            </li>
          ))}
        </ul>
        {shown && shown.rendered !== '' ? (
          <CodeView code={shown.rendered} label={shown.path} heightClass="max-h-96" />
        ) : null}
      </section>
    </div>
  );
}

function order(severity: DiagnosticDto['severity']): number {
  return ['error', 'warning', 'info', 'hint'].indexOf(severity);
}
