import type { DesignerStructureDefaultsResponse } from 'rimstudio-ipc-types';
import { Banner, Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface StructureBannerProps {
  suggestion: DesignerStructureDefaultsResponse | undefined;
  onApply: () => void;
}

/** Offers the parent, projectile, cost list and stuff of the nearest reference weapon. */
export function StructureBanner({ suggestion, onApply }: StructureBannerProps) {
  const filled = suggestion?.filled ?? [];
  if (!suggestion || filled.length === 0) return null;
  return (
    <Banner
      tone="info"
      title={t('designer.structure.title')}
      action={
        <Button size="sm" variant="primary" onClick={onApply}>
          {t('designer.structure.apply')}
        </Button>
      }
    >
      <span>
        {suggestion.reference
          ? t('designer.structure.from', { label: suggestion.reference.label })
          : null}{' '}
        {t('designer.structure.fills', { fields: filled.join(', ') })}
      </span>
      {(suggestion.notes ?? []).map((note) => (
        <span key={note} class="mt-1 block text-small text-muted">
          {note}
        </span>
      ))}
    </Banner>
  );
}
