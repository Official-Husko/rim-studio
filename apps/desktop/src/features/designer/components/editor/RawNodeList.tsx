import { useState } from 'preact/hooks';
import type { RawNodeDto } from 'rimstudio-ipc-types';
import { Button, CodeView, IconButton } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { diagnosticsFor } from '../../model/draft';
import { getAt, type Pointer } from '../../model/pointer';
import { treeToXml } from '../../model/xml';
import { severityText } from './DiagnosticNotes';
import { useFieldEnv } from './fieldEnv';
import { RawNodeDialog } from './RawNodeDialog';

export interface RawNodeListProps {
  /** Where the list of nodes lives in the spec, for example /extraFields. */
  pointer: Pointer;
  /** Heading of the list. */
  label: string;
  help?: string;
  /** The tag a new entry starts with (li for the entries of a list field). */
  defaultTag?: string;
}

type Editing = { index: number } | 'new' | undefined;

/**
 * Carried XML fields: each one shown as XML with Edit and Remove. The list is stored as JSON node
 * trees; the backend validates it and its diagnostics are shown under the entry they name.
 */
export function RawNodeList({ pointer, label, help, defaultTag = '' }: RawNodeListProps) {
  const env = useFieldEnv();
  const raw = getAt(env.spec, pointer);
  const nodes = Array.isArray(raw) ? (raw as RawNodeDto[]) : [];
  const [editing, setEditing] = useState<Editing>(undefined);
  const listDiagnostics = diagnosticsFor(env.diagnostics, pointer);

  const save = (node: RawNodeDto): void => {
    const index = editing === 'new' || editing === undefined ? nodes.length : editing.index;
    env.setField(`${pointer}/${index}`, node);
    setEditing(undefined);
  };
  const remove = (index: number): void => {
    env.setField(nodes.length === 1 ? pointer : `${pointer}/${index}`, undefined);
  };
  const current = typeof editing === 'object' ? nodes[editing.index] : undefined;

  return (
    <div data-field={pointer} class="flex min-w-0 flex-col gap-2">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <h3 class="font-display text-label tracking-label text-muted uppercase">{label}</h3>
        <Button size="sm" icon="plus" onClick={() => setEditing('new')}>
          {t('designer.raw.add')}
        </Button>
      </div>
      {help ? <p class="text-small text-faint">{help}</p> : null}
      {nodes.length === 0 ? <p class="text-small text-muted">{t('designer.raw.none')}</p> : null}
      <ul class="flex flex-col gap-2">
        {nodes.map((node, index) => {
          const own = listDiagnostics.filter(
            (d) => d.field === `${pointer}/${index}` || d.field?.startsWith(`${pointer}/${index}/`),
          );
          return (
            <li
              key={`${index}-${node.tag}`}
              data-field={`${pointer}/${index}`}
              class="flex flex-col gap-1.5 border border-line-subtle p-2"
            >
              <div class="flex items-center justify-between gap-2">
                <span class="font-mono text-mono-small text-muted">{node.tag}</span>
                <span class="flex gap-1">
                  <Button
                    size="sm"
                    variant="ghost"
                    aria-label={t('designer.raw.edit', { name: node.tag })}
                    onClick={() => setEditing({ index })}
                  >
                    {t('designer.raw.editButton')}
                  </Button>
                  <IconButton
                    icon="trash"
                    variant="danger"
                    label={t('designer.raw.remove', { name: node.tag })}
                    onClick={() => remove(index)}
                  />
                </span>
              </div>
              <CodeView
                code={treeToXml(node)}
                label={t('designer.raw.code', { name: node.tag })}
                lineNumbers={false}
                heightClass="max-h-48"
              />
              {own.map((d, i) => (
                <p key={i} class="text-small text-fg">
                  <span class="font-semibold">{severityText(d.severity)}.</span> {d.message}
                </p>
              ))}
            </li>
          );
        })}
      </ul>
      {editing !== undefined ? (
        <RawNodeDialog
          key={typeof editing === 'object' ? `edit-${editing.index}` : 'new'}
          open
          title={
            current
              ? t('designer.raw.editTitle', { name: current.tag })
              : t('designer.raw.addTitle')
          }
          initial={current}
          defaultTag={defaultTag}
          onSave={save}
          onClose={() => setEditing(undefined)}
        />
      ) : null}
    </div>
  );
}
