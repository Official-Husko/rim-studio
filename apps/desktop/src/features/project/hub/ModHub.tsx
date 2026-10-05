import { OpenCard } from '../OpenCard';
import { RecentList } from '../RecentList';
import { CreateCard } from './CreateCard';
import { t } from '~/shared/i18n';

export interface ModHubProps {
  onCreate: () => void;
}

/** With no mod open: two big cards (create a new mod, open an existing one) and the recent mods. */
export function ModHub({ onCreate }: ModHubProps) {
  return (
    <div class="flex flex-col gap-4">
      <h1 class="m-0 font-display text-display font-semibold tracking-display">
        {t('project.title')}
      </h1>
      <p class="m-0 max-w-3xl text-body text-muted">{t('project.hub.intro')}</p>
      <div class="grid grid-cols-1 items-start gap-4 xl:grid-cols-2">
        <CreateCard onCreate={onCreate} />
        <OpenCard />
      </div>
      <RecentList />
    </div>
  );
}
