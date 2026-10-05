import { useState } from 'preact/hooks';
import type { RawNodeDto } from 'rimstudio-ipc-types';
import { Banner, Button, Dialog, FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { isTextOnly, leafNode, readTreeJson, textOf, treeJson } from '../../model/raw';

export interface RawNodeDialogProps {
  open: boolean;
  title: string;
  /** The node being edited; none adds a new one. */
  initial?: RawNodeDto | undefined;
  /** The tag a new node starts with (li for list entries). */
  defaultTag: string;
  onSave: (node: RawNodeDto) => void;
  onClose: () => void;
}

/**
 * Edits one carried XML field. A plain value element has a tag and a text; anything with
 * attributes or child elements is edited as its JSON tree. The browser never parses XML: the
 * backend checks the result and reports problems as diagnostics under the field.
 */
export function RawNodeDialog({
  open,
  title,
  initial,
  defaultTag,
  onSave,
  onClose,
}: RawNodeDialogProps) {
  const simple = initial === undefined || (isTextOnly(initial) && initial.attrs.length === 0);
  const [tree, setTree] = useState(!simple);
  const [tag, setTag] = useState(initial?.tag ?? defaultTag);
  const [value, setValue] = useState(initial ? textOf(initial) : '');
  const [json, setJson] = useState(treeJson(initial ?? leafNode(defaultTag, '')));
  const [problem, setProblem] = useState<'json' | 'shape' | undefined>(undefined);

  const switchToTree = (): void => {
    setJson(treeJson(leafNode(tag.trim(), value)));
    setProblem(undefined);
    setTree(true);
  };

  const save = (): void => {
    if (!tree) {
      onSave(leafNode(tag.trim(), value));
      return;
    }
    const reading = readTreeJson(json);
    if (reading.ok) onSave(reading.node);
    else setProblem(reading.reason === 'json' ? 'json' : 'shape');
  };

  return (
    <Dialog
      open={open}
      title={title}
      onClose={onClose}
      size="lg"
      closeLabel={t('designer.dialog.close')}
      footer={
        <>
          <Button onClick={onClose}>{t('designer.dialog.cancel')}</Button>
          <Button variant="primary" disabled={!tree && tag.trim() === ''} onClick={save}>
            {t('designer.raw.save')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-3">
        {tree ? (
          <>
            <p class="text-small text-muted">{t('designer.raw.treeHelp')}</p>
            <FormField label={t('designer.raw.tree')}>
              <TextField
                multiline
                rows={14}
                value={json}
                onValueChange={(next) => {
                  setJson(next);
                  setProblem(undefined);
                }}
              />
            </FormField>
            {problem ? (
              <Banner tone="error">
                {problem === 'json' ? t('designer.raw.badJson') : t('designer.raw.badShape')}
              </Banner>
            ) : null}
          </>
        ) : (
          <>
            <FormField label={t('designer.raw.tag')} required>
              <TextField value={tag} onValueChange={setTag} />
            </FormField>
            <FormField label={t('designer.raw.value')} help={t('designer.raw.valueHelp')}>
              <TextField value={value} onValueChange={setValue} multiline rows={3} />
            </FormField>
            <div>
              <Button size="sm" variant="ghost" onClick={switchToTree}>
                {t('designer.raw.asTree')}
              </Button>
            </div>
          </>
        )}
      </div>
    </Dialog>
  );
}
