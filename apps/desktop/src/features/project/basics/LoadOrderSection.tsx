import { Panel } from 'rimstudio-ui';
import type { AboutListFieldDto } from 'rimstudio-ipc-types';
import { t, type MessageKey } from '~/shared/i18n';
import { ChipListEditor } from './ChipListEditor';
import { findingsFor, listOf } from './aboutModel';
import { aboutDraft, aboutModel, findings, setList } from './aboutStore';

const LISTS: { field: AboutListFieldDto; label: MessageKey; help: MessageKey }[] = [
  {
    field: 'loadAfter',
    label: 'project.basics.order.after',
    help: 'project.basics.order.after.help',
  },
  {
    field: 'loadBefore',
    label: 'project.basics.order.before',
    help: 'project.basics.order.before.help',
  },
  {
    field: 'forceLoadAfter',
    label: 'project.basics.order.forceAfter',
    help: 'project.basics.order.forceAfter.help',
  },
  {
    field: 'forceLoadBefore',
    label: 'project.basics.order.forceBefore',
    help: 'project.basics.order.forceBefore.help',
  },
  {
    field: 'incompatibleWith',
    label: 'project.basics.order.incompatible',
    help: 'project.basics.order.incompatible.help',
  },
];

/** Load order and conflicts: lists of package ids, filled from the library or typed. */
export function LoadOrderSection() {
  const about = aboutModel.value;
  if (!about) return null;
  return (
    <Panel title={t('project.basics.order')} framed>
      <div class="grid grid-cols-1 gap-5 p-3 xl:grid-cols-2">
        {LISTS.map((list) => (
          <ChipListEditor
            key={list.field}
            library
            label={t(list.label)}
            help={t(list.help)}
            items={listOf(about, aboutDraft.value, list.field)}
            onChange={(items) => setList(list.field, items)}
            disabled={!about.editable}
            findings={findingsFor(findings.value, list.field)}
          />
        ))}
      </div>
    </Panel>
  );
}
