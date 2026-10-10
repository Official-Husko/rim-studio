import type { DiagnosticDto, DraftEntryDto, ProjectWeaponDto } from 'rimstudio-ipc-types';
import { Banner, Button, EmptyState, Panel, Spinner } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import type { ProjectWeaponsStore } from '../project-weapons-store';
import { ProjectWeaponRow } from './ProjectWeaponRow';

export interface ProjectWeaponsProps {
  store: ProjectWeaponsStore;
  /** The id of the draft open in the editor: the weapon it edits is marked. */
  openId: string | undefined;
  /** Called with the stored draft after a weapon was loaded; the page makes it the open draft. */
  onOpened: (entry: DraftEntryDto) => void;
}

/** The worst severity decides the tone of the banner of reading problems. */
function toneOf(diagnostics: readonly DiagnosticDto[]): 'error' | 'warning' | 'info' {
  if (diagnostics.some((d) => d.severity === 'error')) return 'error';
  if (diagnostics.some((d) => d.severity === 'warning')) return 'warning';
  return 'info';
}

function Problems({ diagnostics }: { diagnostics: readonly DiagnosticDto[] }) {
  return (
    <Banner
      tone={toneOf(diagnostics)}
      title={tn('designer.projectWeapons.problems', diagnostics.length)}
    >
      <ul class="mt-1 flex max-h-32 flex-col gap-1 overflow-y-auto text-small">
        {diagnostics.map((d, i) => (
          <li key={`${d.code}-${i}`}>
            {d.message} <span class="font-mono text-mono-small text-faint">{d.code}</span>
          </li>
        ))}
      </ul>
    </Banner>
  );
}

/**
 * The weapons the open mod defines in its own files, as the base game loads them. A row loads the
 * weapon into the editor, where it is edited where it stands, or continues the draft that already
 * edits it.
 */
export function ProjectWeapons({ store, openId, onOpened }: ProjectWeaponsProps) {
  const weapons = store.weapons.value;
  const error = store.error.value;
  const problems = store.diagnostics.value;
  const loading = store.loading.value;
  const loaded = store.loaded.value;
  const opening = store.opening.value;

  const load = async (weapon: ProjectWeaponDto): Promise<void> => {
    const entry = await store.open(weapon.defName);
    if (entry) onOpened(entry);
  };

  return (
    <Panel
      title={t('designer.projectWeapons.title')}
      actions={
        <>
          {loading ? <Spinner size="sm" label={t('app.loading')} /> : null}
          {loaded ? (
            <span class="font-mono text-mono-small text-muted">
              {tn('designer.projectWeapons.count', weapons.length)}
            </span>
          ) : null}
        </>
      }
    >
      <div class="flex flex-col gap-2">
        {error ? (
          <Banner
            tone="error"
            title={error.code}
            action={
              <Button size="sm" onClick={() => void store.load()}>
                {t('designer.retry')}
              </Button>
            }
          >
            {error.message}
          </Banner>
        ) : null}
        {problems.length > 0 ? <Problems diagnostics={problems} /> : null}
        {!loaded && loading ? (
          <p class="text-small text-muted">{t('designer.projectWeapons.loading')}</p>
        ) : null}
        {loaded && weapons.length === 0 && !error ? (
          <EmptyState
            compact
            icon="crosshair"
            title={t('designer.projectWeapons.emptyTitle')}
            description={t('designer.projectWeapons.emptyBody')}
          />
        ) : null}
        {weapons.length > 0 ? (
          <ul class="flex flex-col gap-2" aria-label={t('designer.projectWeapons.title')}>
            {weapons.map((weapon) => {
              const draft = store.draftOf(weapon);
              return (
                <ProjectWeaponRow
                  key={weapon.defName}
                  weapon={weapon}
                  draft={draft}
                  isOpen={draft !== undefined && draft.id === openId}
                  busy={opening === weapon.defName}
                  disabled={loading || opening !== undefined}
                  onLoad={() => void load(weapon)}
                />
              );
            })}
          </ul>
        ) : null}
      </div>
    </Panel>
  );
}
