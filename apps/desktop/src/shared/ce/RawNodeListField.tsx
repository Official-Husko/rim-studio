import { useState } from 'preact/hooks';
import type { RawNodeDto } from 'rimstudio-ipc-types';
import { Button, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { leafNode, nodeSummary, replaceAt } from './blockModel';
import { RawNodeField } from './RawNodeField';

export interface RawNodeListFieldProps {
  label: string;
  help?: string;
  nodes: readonly RawNodeDto[];
  onChange: (nodes: RawNodeDto[]) => void;
}

/** A list of raw elements: each can be edited as a tree or removed, and a simple one can be added by name. */
export function RawNodeListField({ label, help, nodes, onChange }: RawNodeListFieldProps) {
  const [tag, setTag] = useState('');
  const [text, setText] = useState('');
  const add = (): void => {
    if (tag.trim() === '') return;
    onChange([...nodes, leafNode(tag.trim(), text)]);
    setTag('');
    setText('');
  };
  return (
    <div class="flex flex-col gap-2" role="group" aria-label={label}>
      <span class="text-small font-medium">{label}</span>
      {help ? <p class="m-0 text-small text-muted">{help}</p> : null}
      {nodes.map((node, index) => (
        <div key={index} class="flex flex-col gap-1 rounded-sm border border-line p-2">
          <RawNodeField
            label={t('ceblock.raw.element', { summary: nodeSummary(node) })}
            node={node}
            onChange={(next) => onChange(replaceAt(nodes, index, next))}
          />
          <div class="self-start">
            <Button
              size="sm"
              variant="secondary"
              onClick={() => onChange(replaceAt(nodes, index, undefined))}
            >
              {t('ceblock.raw.remove')}
            </Button>
          </div>
        </div>
      ))}
      <div class="flex flex-wrap items-end gap-2">
        <div class="w-40">
          <FormField label={t('ceblock.raw.tag')}>
            <TextField aria-label={t('ceblock.raw.tag')} value={tag} onValueChange={setTag} />
          </FormField>
        </div>
        <div class="min-w-0 flex-1">
          <FormField label={t('ceblock.raw.text')}>
            <TextField aria-label={t('ceblock.raw.text')} value={text} onValueChange={setText} />
          </FormField>
        </div>
        <Button size="sm" variant="secondary" disabled={tag.trim() === ''} onClick={add}>
          {t('ceblock.raw.add')}
        </Button>
      </div>
    </div>
  );
}
