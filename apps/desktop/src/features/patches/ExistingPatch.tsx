import type { ComponentChildren } from 'preact';
import type { ExistingPatchDto, PatchOriginDto } from 'rimstudio-ipc-types';
import { Banner, Button, Panel } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';

const ORIGIN = {
  rimstudio: 'patches.existing.origin.rimstudio',
  project: 'patches.existing.origin.project',
  unknown: 'patches.existing.origin.unknown',
} as const satisfies Partial<Record<PatchOriginDto, string>>;

/** Where the conversion comes from, in a sentence. The mod of another author is named when it is known. */
function originText(existing: ExistingPatchDto): string {
  if (existing.origin === 'foreign') {
    return existing.modName
      ? t('patches.existing.origin.foreign', { modName: existing.modName })
      : t('patches.existing.origin.foreign-unnamed');
  }
  return t(ORIGIN[existing.origin]);
}

/** A name over its value: the pane is narrow, so a long path gets the whole width. */
function Fact({ name, children }: { name: string; children: ComponentChildren }) {
  return (
    <div class="flex flex-col gap-0.5">
      <dt class="text-small text-muted">{name}</dt>
      <dd class="m-0 min-w-0 font-mono text-mono-small break-words">{children}</dd>
    </div>
  );
}

export interface ExistingPatchProps {
  existing: ExistingPatchDto;
  onRemove: () => void;
  onReplace: () => void;
}

/**
 * The conversion an already converted weapon carries: where it comes from, the files it lives in and, when
 * RimStudio generated it, the two things that can be done with it. A patch of another mod or one written by
 * hand is only described: it is never changed from here.
 */
export function ExistingPatch({ existing, onRemove, onReplace }: ExistingPatchProps) {
  const files = existing.files ?? [];
  // the sections of a conversion that is not RimStudio's own are overrides on top of it: the weapon stays
  // converted after a removal, so there is nothing to replace
  const generated = existing.origin === 'rimstudio';
  // a removal lists the file that holds the conversion, so it is not named a second time
  const named = existing.file !== undefined && !files.includes(existing.file);
  return (
    <Panel title={t('patches.existing.title')}>
      <div class="flex flex-col gap-3">
        <p class="m-0 text-body">{originText(existing)}</p>
        {named || files.length > 0 ? (
          <dl aria-label={t('patches.existing.facts')} class="m-0 flex flex-col gap-2">
            {named ? <Fact name={t('patches.existing.file')}>{existing.file}</Fact> : null}
            {files.length > 0 ? (
              <Fact name={tn('patches.existing.files', files.length)}>
                <ul class="m-0 flex list-none flex-col gap-0.5 p-0">
                  {files.map((file) => (
                    <li key={file}>{file}</li>
                  ))}
                </ul>
              </Fact>
            ) : null}
          </dl>
        ) : null}
        {existing.removable ? (
          <>
            <p class="m-0 text-small text-muted">
              {generated ? t('patches.existing.help') : t('patches.existing.keeps')}
            </p>
            <div class="flex flex-wrap items-center gap-2">
              <Button variant="danger" icon="trash" onClick={onRemove}>
                {t('patches.existing.remove')}
              </Button>
              {generated ? (
                <Button variant="secondary" icon="refresh" onClick={onReplace}>
                  {t('patches.existing.replace')}
                </Button>
              ) : null}
            </div>
          </>
        ) : (
          <Banner tone="info">{t('patches.existing.locked')}</Banner>
        )}
      </div>
    </Panel>
  );
}
