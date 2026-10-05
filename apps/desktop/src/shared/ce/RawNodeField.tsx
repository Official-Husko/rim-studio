import { useEffect, useState } from 'preact/hooks';
import type { RawNodeDto } from 'rimstudio-ipc-types';
import { FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { parseTree, treeProblem } from './blockModel';

export interface RawNodeFieldProps {
  label: string;
  node: RawNodeDto | undefined;
  /** Called with the new element, or undefined when the text is emptied. */
  onChange: (node: RawNodeDto | undefined) => void;
  help?: string;
}

function show(node: RawNodeDto | undefined): string {
  return node ? JSON.stringify(node, null, 2) : '';
}

/**
 * One element of the patch as an editable tree. The webview never reads XML: the tree is JSON with a tag,
 * attributes and children, and the backend checks it when the plan is built.
 */
export function RawNodeField({ label, node, onChange, help }: RawNodeFieldProps) {
  const [text, setText] = useState(() => show(node));
  const [problem, setProblem] = useState<'json' | 'shape' | undefined>(undefined);
  const shown = show(node);
  // follow the block when it changes from outside (a suggestion taken, another weapon opened)
  useEffect(() => {
    setText(shown);
    setProblem(undefined);
  }, [shown]);
  const commit = (): void => {
    if (text.trim() === '') {
      setProblem(undefined);
      onChange(undefined);
      return;
    }
    const found = treeProblem(text);
    setProblem(found);
    if (!found) onChange(parseTree(text));
  };
  return (
    <FormField
      label={label}
      {...(help ? { help } : {})}
      {...(problem
        ? { error: problem === 'json' ? t('ceblock.raw.badJson') : t('ceblock.raw.badShape') }
        : {})}
    >
      <TextField
        aria-label={label}
        multiline
        rows={Math.min(10, Math.max(3, text.split('\n').length))}
        value={text}
        invalid={problem !== undefined}
        onValueChange={setText}
        onBlur={commit}
      />
    </FormField>
  );
}
