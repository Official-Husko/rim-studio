import { Checkbox } from 'rimstudio-ui';
import { t, type MessageKey } from '~/shared/i18n';
import type { NewModForm } from '../scaffold';
import { StructureTree } from './StructureTree';
import { changeForm, createForm, createPreview, createTarget } from './createStore';

type Flag = Exclude<
  keyof NewModForm,
  'name' | 'author' | 'packageId' | 'versions' | 'description' | 'folderName'
>;

interface Group {
  title: MessageKey;
  flags: { key: Flag; label: MessageKey; help: MessageKey }[];
}

const GROUPS: Group[] = [
  {
    title: 'project.create.group.standard',
    flags: [
      {
        key: 'patchesFolder',
        label: 'project.new.opt.patches',
        help: 'project.create.opt.patches.help',
      },
      {
        key: 'texturesFolder',
        label: 'project.new.opt.textures',
        help: 'project.create.opt.textures.help',
      },
      {
        key: 'soundsFolder',
        label: 'project.new.opt.sounds',
        help: 'project.create.opt.sounds.help',
      },
    ],
  },
  {
    title: 'project.create.group.layout',
    flags: [
      {
        key: 'versionedFolders',
        label: 'project.new.opt.versioned',
        help: 'project.create.opt.versioned.help',
      },
      { key: 'cePatchFolder', label: 'project.new.opt.ce', help: 'project.create.opt.ce.help' },
    ],
  },
  {
    title: 'project.create.group.optional',
    flags: [
      {
        key: 'languagesFolder',
        label: 'project.new.opt.languages',
        help: 'project.create.opt.languages.help',
      },
      {
        key: 'assembliesFolder',
        label: 'project.new.opt.assemblies',
        help: 'project.create.opt.assemblies.help',
      },
      {
        key: 'sourceFolder',
        label: 'project.new.opt.source',
        help: 'project.create.opt.source.help',
      },
    ],
  },
  {
    title: 'project.create.group.files',
    flags: [
      {
        key: 'gitignore',
        label: 'project.new.opt.gitignore',
        help: 'project.create.opt.gitignore.help',
      },
      {
        key: 'ignoreSourceArt',
        label: 'project.new.opt.ignoreSource',
        help: 'project.create.opt.ignoreSource.help',
      },
      { key: 'readme', label: 'project.new.opt.readme', help: 'project.create.opt.readme.help' },
      { key: 'credits', label: 'project.new.opt.credits', help: 'project.create.opt.credits.help' },
      {
        key: 'placeholderFiles',
        label: 'project.new.opt.placeholders',
        help: 'project.create.opt.placeholders.help',
      },
    ],
  },
];

/** Step 2: the recommended structure as a tree, with the optional parts as switches. */
export function StructureStep() {
  const form = createForm.value;
  const preview = createPreview.value;
  return (
    <div class="grid grid-cols-1 gap-6 lg:grid-cols-[minmax(0,22rem)_minmax(0,1fr)]">
      <div class="flex flex-col gap-4">
        {GROUPS.map((group) => (
          <fieldset key={group.title} class="m-0 min-w-0 border-0 p-0">
            <legend class="mb-1 p-0 font-display text-small tracking-display text-muted uppercase">
              {t(group.title)}
            </legend>
            <div class="flex flex-col gap-2">
              {group.flags.map((flag) => (
                <div key={flag.key} class="flex flex-col">
                  <Checkbox
                    checked={form[flag.key]}
                    disabled={flag.key === 'ignoreSourceArt' && !form.sourceFolder}
                    onCheckedChange={(checked) => changeForm({ [flag.key]: checked })}
                  >
                    {t(flag.label)}
                  </Checkbox>
                  <span class="pl-6 text-small text-faint">{t(flag.help)}</span>
                </div>
              ))}
            </div>
          </fieldset>
        ))}
      </div>
      <div class="min-w-0">
        <h3 class="m-0 mb-2 font-display text-small tracking-display text-muted uppercase">
          {t('project.create.tree.title')}
        </h3>
        {preview && preview.entries.length > 0 ? (
          <StructureTree root={createTarget.value} entries={preview.entries} />
        ) : (
          <p class="m-0 text-small text-muted">{t('project.new.preview.empty')}</p>
        )}
      </div>
    </div>
  );
}
