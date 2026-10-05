import type { CeOptionDto } from 'rimstudio-ipc-types';
import { Badge, Button } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import {
  optionPatch,
  optionTaken,
  optionValueText,
  type CeBlock,
  type CeBlockPatch,
} from './blockModel';

export interface CeOptionsListProps {
  options: readonly CeOptionDto[];
  block: CeBlock;
  onChange: (patch: CeBlockPatch) => void;
}

/**
 * The optional additions the user's own conversions suggest, one card each. Nothing is written until the
 * user takes one; a taken suggestion becomes a normal member of the block that can be edited or removed.
 */
export function CeOptionsList({ options, block, onChange }: CeOptionsListProps) {
  if (options.length === 0) return null;
  return (
    <section class="mt-3 flex flex-col gap-2" aria-label={t('ceblock.options.title')}>
      <h4 class="m-0 font-display text-label font-semibold tracking-label text-muted uppercase">
        {t('ceblock.options.title')}
      </h4>
      <p class="m-0 text-small text-muted">{t('ceblock.options.help')}</p>
      <ul class="m-0 flex list-none flex-col gap-2 p-0">
        {options.map((option) => {
          const taken = optionTaken(option, block);
          return (
            <li
              key={option.id}
              data-option={option.id}
              class="flex flex-col gap-1 rounded-sm border border-line p-2"
            >
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-medium">{option.label}</span>
                <Badge tone="info">
                  {t('ceblock.options.share', { n: option.examples, of: option.of })}
                </Badge>
                <span class="min-w-0 flex-1 truncate font-mono text-small">
                  {optionValueText(option)}
                </span>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={taken}
                  aria-label={t('ceblock.options.takeLabel', { label: option.label })}
                  onClick={() => onChange(optionPatch(option, block))}
                >
                  {taken ? t('ceblock.options.taken') : t('ceblock.options.take')}
                </Button>
              </div>
              <p class="m-0 text-small text-muted">{option.why}</p>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
