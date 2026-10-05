import { Badge } from 'rimstudio-ui';
import type { LayoutFixItemDto } from 'rimstudio-ipc-types';
import { t, tn } from '~/shared/i18n';
import { FIX_KIND_LABEL, FIX_RISK_LABEL, FIX_RISK_TONE } from './fixLabels';

const SHOWN_REFERENCES = 5;

/** The items RimStudio does not carry out, each with the reason and the places that mention the old path. */
export function FixReviewList({ items }: { items: LayoutFixItemDto[] }) {
  if (items.length === 0) return null;
  return (
    <section class="flex flex-col gap-2" aria-label={t('project.fix.review.title')}>
      <h3 class="m-0 text-body font-semibold">{t('project.fix.review.title')}</h3>
      <p class="m-0 text-small text-muted">{t('project.fix.review.hint')}</p>
      <ul class="m-0 flex list-none flex-col divide-y divide-line-subtle border border-line p-0">
        {items.map((item) => (
          <li key={item.id} class="flex flex-col gap-1 px-3 py-2">
            <div class="flex flex-wrap items-center gap-2">
              <span class="font-semibold">{t(FIX_KIND_LABEL[item.kind])}</span>
              <Badge tone={FIX_RISK_TONE[item.risk]}>{t(FIX_RISK_LABEL[item.risk])}</Badge>
            </div>
            <div class="flex flex-col gap-0.5 font-mono text-mono">
              <span class="break-all text-muted">{item.from}</span>
              <span class="break-all">
                {'→ '}
                {item.to}
              </span>
            </div>
            <p class="m-0 text-small">{item.reviewReason ?? item.why}</p>
            {item.references.length > 0 ? (
              <div class="flex flex-col gap-0.5">
                <p class="m-0 text-small text-muted">
                  {tn('project.fix.references', item.references.length)}
                </p>
                <ul class="m-0 flex list-none flex-col gap-0.5 p-0 font-mono text-mono-small">
                  {item.references.slice(0, SHOWN_REFERENCES).map((ref) => (
                    <li key={`${ref.path}:${ref.line}`} class="break-all">
                      <span class="text-muted">
                        {ref.path}:{ref.line}
                      </span>{' '}
                      {ref.text}
                    </li>
                  ))}
                  {item.references.length > SHOWN_REFERENCES ? (
                    <li class="text-muted">
                      {t('project.fix.more', { n: item.references.length - SHOWN_REFERENCES })}
                    </li>
                  ) : null}
                </ul>
              </div>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
