import { Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { ChipListEditor } from './ChipListEditor';
import { TextRow } from './TextRow';
import { findingsFor, listOf } from './aboutModel';
import { aboutDraft, aboutModel, findings, setList } from './aboutStore';

/** Name, authors, package id, mod version and the project address. */
export function IdentitySection() {
  const about = aboutModel.value;
  if (!about) return null;
  return (
    <Panel title={t('project.basics.identity')} framed>
      <div class="grid grid-cols-1 gap-4 p-3 md:grid-cols-2">
        <TextRow field="name" label={t('project.basics.name')} required />
        <TextRow
          field="shortName"
          label={t('project.basics.shortName')}
          help={t('project.basics.shortName.help')}
        />
        <TextRow
          field="author"
          label={t('project.basics.author')}
          help={t('project.basics.author.help')}
        />
        <ChipListEditor
          label={t('project.basics.authors')}
          help={t('project.basics.authors.help')}
          items={listOf(about, aboutDraft.value, 'authors')}
          onChange={(items) => setList('authors', items)}
          disabled={!about.editable}
          findings={findingsFor(findings.value, 'authors')}
        />
        <TextRow
          field="packageId"
          label={t('project.basics.packageId')}
          help={t('project.basics.packageId.help')}
          required
        />
        <TextRow
          field="modVersion"
          label={t('project.basics.modVersion')}
          help={t('project.basics.modVersion.help')}
        />
        <div class="md:col-span-2">
          <TextRow
            field="url"
            label={t('project.basics.url')}
            help={t('project.basics.url.help')}
          />
        </div>
      </div>
    </Panel>
  );
}
