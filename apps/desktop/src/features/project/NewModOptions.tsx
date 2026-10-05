import { Checkbox } from 'rimstudio-ui';
import { t, type MessageKey } from '~/shared/i18n';
import type { NewModForm } from './scaffold';

type Flag = Exclude<
  keyof NewModForm,
  'name' | 'author' | 'packageId' | 'versions' | 'description' | 'folderName'
>;

interface Group {
  title: MessageKey;
  /** Short labels sit in two columns. */
  paired?: boolean;
  flags: { key: Flag; label: MessageKey }[];
}

const GROUPS: Group[] = [
  {
    title: 'project.new.group.folders',
    paired: true,
    flags: [
      { key: 'patchesFolder', label: 'project.new.opt.patches' },
      { key: 'texturesFolder', label: 'project.new.opt.textures' },
      { key: 'soundsFolder', label: 'project.new.opt.sounds' },
      { key: 'languagesFolder', label: 'project.new.opt.languages' },
      { key: 'assembliesFolder', label: 'project.new.opt.assemblies' },
      { key: 'sourceFolder', label: 'project.new.opt.source' },
    ],
  },
  {
    title: 'project.new.group.layout',
    flags: [
      { key: 'versionedFolders', label: 'project.new.opt.versioned' },
      { key: 'cePatchFolder', label: 'project.new.opt.ce' },
      { key: 'placeholderFiles', label: 'project.new.opt.placeholders' },
    ],
  },
  {
    title: 'project.new.group.files',
    flags: [
      { key: 'gitignore', label: 'project.new.opt.gitignore' },
      { key: 'ignoreSourceArt', label: 'project.new.opt.ignoreSource' },
      { key: 'readme', label: 'project.new.opt.readme' },
      { key: 'credits', label: 'project.new.opt.credits' },
    ],
  },
];

export interface NewModOptionsProps {
  form: NewModForm;
  onChange: (patch: Partial<NewModForm>) => void;
}

/** The switches of the scaffold: which folders and files the new mod gets besides About.xml. */
export function NewModOptions({ form, onChange }: NewModOptionsProps) {
  return (
    <div class="flex flex-col gap-3">
      {GROUPS.map((group) => (
        <fieldset key={group.title} class="m-0 min-w-0 border-0 p-0">
          <legend class="mb-1 p-0 font-display text-small tracking-display text-muted uppercase">
            {t(group.title)}
          </legend>
          <div class={group.paired ? 'grid grid-cols-2 gap-x-4 gap-y-1' : 'flex flex-col gap-1'}>
            {group.flags.map((flag) => (
              <Checkbox
                key={flag.key}
                checked={form[flag.key]}
                disabled={flag.key === 'ignoreSourceArt' && !form.sourceFolder}
                onCheckedChange={(checked) => onChange({ [flag.key]: checked })}
              >
                {t(flag.label)}
              </Checkbox>
            ))}
          </div>
        </fieldset>
      ))}
    </div>
  );
}
