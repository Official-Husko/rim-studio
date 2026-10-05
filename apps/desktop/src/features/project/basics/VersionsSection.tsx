import { Checkbox, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { FieldFindings } from './FieldFindings';
import { findingsFor, listOf, versionChoices } from './aboutModel';
import { aboutDraft, aboutModel, findings, setList } from './aboutStore';

/** The game versions the mod says it supports, as checkboxes. */
export function VersionsSection() {
  const about = aboutModel.value;
  if (!about) return null;
  const chosen = listOf(about, aboutDraft.value, 'supportedVersions');
  const choices = versionChoices(about.gameVersion, chosen);
  const toggle = (version: string, on: boolean): void => {
    const next = on ? [...chosen, version] : chosen.filter((v) => v !== version);
    setList(
      'supportedVersions',
      choices.filter((v) => next.includes(v)),
    );
  };
  return (
    <Panel title={t('project.basics.versions')} framed>
      <div class="flex flex-col gap-3 p-3">
        <p class="m-0 text-small text-muted">
          {about.gameVersion
            ? t('project.basics.versions.help', { version: about.gameVersion })
            : t('project.basics.versions.helpNoGame')}
        </p>
        <fieldset class="m-0 flex min-w-0 flex-wrap gap-x-6 gap-y-1 border-0 p-0">
          <legend class="sr-only">{t('project.basics.versions')}</legend>
          {choices.map((version) => (
            <Checkbox
              key={version}
              checked={chosen.includes(version)}
              disabled={!about.editable}
              onCheckedChange={(on) => toggle(version, on)}
            >
              {version}
            </Checkbox>
          ))}
        </fieldset>
        <FieldFindings items={findingsFor(findings.value, 'supportedVersions')} />
      </div>
    </Panel>
  );
}
