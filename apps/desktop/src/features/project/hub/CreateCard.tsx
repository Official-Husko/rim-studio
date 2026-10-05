import { Button, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface CreateCardProps {
  onCreate: () => void;
}

/** The first of the two big cards: start a new mod with the recommended structure. */
export function CreateCard({ onCreate }: CreateCardProps) {
  return (
    <Panel title={t('project.hub.create.title')} framed>
      <div class="flex flex-col gap-3 p-4">
        <p class="m-0 text-body">{t('project.hub.create.body')}</p>
        <ul class="m-0 flex list-disc flex-col gap-1 pl-5 text-small text-muted">
          <li>{t('project.hub.create.point.about')}</li>
          <li>{t('project.hub.create.point.folders')}</li>
          <li>{t('project.hub.create.point.optional')}</li>
        </ul>
        <div>
          <Button icon="plus" onClick={onCreate}>
            {t('project.hub.create.action')}
          </Button>
        </div>
      </div>
    </Panel>
  );
}
