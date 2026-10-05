import { useEffect, useState } from 'preact/hooks';
import type { ApiError } from 'rimstudio-ipc-types';
import { Banner, CodeView, Dialog, Spinner } from 'rimstudio-ui';
import { normalizeError } from '~/shared/ipc';
import { t } from '~/shared/i18n';
import { resolveDefinition } from '../api';
import { treeToXml } from '../model/xml';

export interface DefinitionDialogProps {
  /** The def name to show; undefined keeps the dialog closed. */
  defName: string | undefined;
  onClose: () => void;
}

/** The definition of a reference weapon as the game resolves it, as XML text for comparison. */
export function DefinitionDialog({ defName, onClose }: DefinitionDialogProps) {
  const [xml, setXml] = useState<string | undefined>(undefined);
  const [file, setFile] = useState('');
  const [error, setError] = useState<ApiError | undefined>(undefined);
  useEffect(() => {
    setXml(undefined);
    setError(undefined);
    if (!defName) return;
    let current = true;
    resolveDefinition(defName).then(
      (def) => {
        if (!current) return;
        setXml(treeToXml(def.tree));
        setFile(`${def.modId}: ${def.file}`);
      },
      (thrown: unknown) => current && setError(normalizeError(thrown)),
    );
    return () => {
      current = false;
    };
  }, [defName]);
  return (
    <Dialog
      open={defName !== undefined}
      title={t('designer.definition.title', { name: defName ?? '' })}
      onClose={onClose}
      closeLabel={t('designer.dialog.close')}
      size="lg"
    >
      {error ? (
        <Banner tone="error" title={error.code}>
          {error.message}
        </Banner>
      ) : xml === undefined ? (
        <div class="flex justify-center py-6">
          <Spinner size="md" label={t('app.loading')} />
        </div>
      ) : (
        <div class="flex flex-col gap-2">
          <p class="font-mono text-mono-small text-faint">{file}</p>
          <CodeView
            code={xml}
            language="xml"
            label={t('designer.definition.code', { name: defName ?? '' })}
            lineNumbers
            heightClass="max-h-96"
          />
        </div>
      )}
    </Dialog>
  );
}
