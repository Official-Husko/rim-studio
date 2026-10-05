import { useState } from 'preact/hooks';
import { Button, EmptyState, Panel } from 'rimstudio-ui';
import type { LibraryModHitDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { DependencyRow } from './DependencyRow';
import { ModPicker } from './ModPicker';
import { depsOf, findingsFor, listOf, moveItem, type DraftDependency } from './aboutModel';
import { aboutDraft, aboutModel, findings, setDependencies, setList } from './aboutStore';
import { CE_PACKAGE_ID, useCombatExtended } from './useCombatExtended';

function fromHit(hit: LibraryModHitDto): DraftDependency {
  return {
    packageId: hit.packageId,
    displayName: hit.name,
    ...(hit.workshopUrl ? { steamWorkshopUrl: hit.workshopUrl } : {}),
  };
}

/** The mods this mod needs, each with a display name and a Workshop link; added from the library or by hand. */
export function DependenciesSection() {
  const about = aboutModel.value;
  const [picking, setPicking] = useState(false);
  const ce = useCombatExtended();
  if (!about) return null;
  const rows = depsOf(about, aboutDraft.value);
  const locked = !about.editable;
  const hasCe = rows.some((r) => r.packageId.toLowerCase() === CE_PACKAGE_ID);

  const change = (index: number, patch: Partial<DraftDependency>): void =>
    setDependencies(rows.map((row, i) => (i === index ? { ...row, ...patch } : row)));
  const add = (row: DraftDependency): void => {
    if (
      row.packageId &&
      rows.some((r) => r.packageId.toLowerCase() === row.packageId.toLowerCase())
    )
      return;
    setDependencies([...rows, row]);
  };
  const addCe = (hit: LibraryModHitDto): void => {
    add(fromHit(hit));
    const after = listOf(about, aboutDraft.value, 'loadAfter');
    if (!after.some((id) => id.toLowerCase() === hit.packageId.toLowerCase()))
      setList('loadAfter', [...after, hit.packageId]);
  };

  return (
    <Panel title={t('project.basics.deps')} framed>
      <div class="flex flex-col gap-3 p-3">
        <p class="m-0 text-small text-muted">{t('project.basics.deps.help')}</p>
        {rows.length === 0 ? (
          <EmptyState compact icon="link" title={t('project.basics.deps.empty')} />
        ) : (
          <ul class="m-0 flex list-none flex-col gap-2 p-0" aria-label={t('project.basics.deps')}>
            {rows.map((row, index) => (
              <DependencyRow
                key={`${row.origId ?? 'new'}:${index}`}
                row={row}
                index={index}
                count={rows.length}
                disabled={locked}
                findings={findingsFor(findings.value, `modDependencies/${index}`)}
                onChange={(patch) => change(index, patch)}
                onRemove={() => setDependencies(rows.filter((_, i) => i !== index))}
                onMove={(to) => setDependencies(moveItem(rows, index, to))}
              />
            ))}
          </ul>
        )}
        <div class="flex flex-wrap gap-2">
          <Button
            size="sm"
            icon="search"
            aria-expanded={picking}
            disabled={locked}
            onClick={() => setPicking(!picking)}
          >
            {t('project.basics.deps.library')}
          </Button>
          <Button
            size="sm"
            icon="plus"
            disabled={locked}
            onClick={() => add({ packageId: '', displayName: '' })}
          >
            {t('project.basics.deps.manual')}
          </Button>
          {ce && !hasCe ? (
            <Button size="sm" icon="crosshair" disabled={locked} onClick={() => addCe(ce)}>
              {t('project.basics.deps.ce')}
            </Button>
          ) : null}
        </div>
        {picking ? (
          <ModPicker
            label={t('project.basics.deps.search')}
            taken={rows.map((r) => r.packageId)}
            onPick={(hit) => add(fromHit(hit))}
          />
        ) : null}
      </div>
    </Panel>
  );
}
