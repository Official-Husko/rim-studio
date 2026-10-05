import { Banner, Chip } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { useFieldEnv } from './fieldEnv';

/** What the draft remembers from its source and does not edit: missing fields, attributes and resets. */
export function CarriedInfo() {
  const { spec } = useFieldEnv();
  const missing = spec.acceptedMissing ?? [];
  const reset = spec.inheritReset ?? [];
  const omitted = spec.omitDefaults ?? [];
  const attrs = Object.entries(spec.extraAttrs ?? {});
  if (missing.length + reset.length + omitted.length + attrs.length === 0) return null;
  return (
    <div class="flex flex-col gap-3">
      {missing.length > 0 ? (
        <Banner tone="info" title={t('designer.carried.missingTitle')}>
          {t('designer.carried.missing', { fields: missing.join(', ') })}
        </Banner>
      ) : null}
      {attrs.length > 0 ? (
        <div class="flex flex-col gap-1">
          <h3 class="font-display text-label tracking-label text-muted uppercase">
            {t('designer.carried.attrs')}
          </h3>
          <ul class="flex flex-wrap gap-1.5" aria-label={t('designer.carried.attrs')}>
            {attrs.map(([name, value]) => (
              <li key={name}>
                <Chip>{`${name}="${value}"`}</Chip>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {reset.length > 0 ? (
        <div class="flex flex-col gap-1">
          <h3 class="font-display text-label tracking-label text-muted uppercase">
            {t('designer.carried.reset')}
          </h3>
          <p class="text-small text-faint">{t('designer.carried.resetHelp')}</p>
          <ul class="flex flex-wrap gap-1.5" aria-label={t('designer.carried.reset')}>
            {reset.map((name) => (
              <li key={name}>
                <Chip>{name}</Chip>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {omitted.length > 0 ? (
        <div class="flex flex-col gap-1">
          <h3 class="font-display text-label tracking-label text-muted uppercase">
            {t('designer.carried.omitted')}
          </h3>
          <p class="text-small text-faint">{t('designer.carried.omittedHelp')}</p>
          <ul class="flex flex-wrap gap-1.5" aria-label={t('designer.carried.omitted')}>
            {omitted.map((name) => (
              <li key={name}>
                <Chip>{name}</Chip>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
