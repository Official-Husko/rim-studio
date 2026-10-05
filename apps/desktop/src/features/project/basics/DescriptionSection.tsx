import { Panel } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import { TextRow } from './TextRow';
import { textOf } from './aboutModel';
import { aboutDraft, aboutModel } from './aboutStore';

/** The description: a multi line editor, its length, and how the game shows it. */
export function DescriptionSection() {
  const about = aboutModel.value;
  if (!about) return null;
  const length = textOf(about, aboutDraft.value, 'description').length;
  return (
    <Panel title={t('project.basics.description')} framed>
      <div class="flex flex-col gap-2 p-3">
        <TextRow
          field="description"
          label={t('project.basics.description')}
          multiline
          rows={10}
          help={t('project.basics.description.help')}
        />
        <p class="m-0 text-small text-muted" aria-live="polite">
          {t('project.basics.description.count', { n: formatNumber(length, 0) })}
        </p>
      </div>
    </Panel>
  );
}
