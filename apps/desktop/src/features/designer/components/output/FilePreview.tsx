import { useState } from 'preact/hooks';
import type { PlannedFileDto } from 'rimstudio-ipc-types';
import { Button, CodeView, cx, DiffView, Dialog, SegmentedControl } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { nameOf } from '../../output-model';
import { CopyPreview } from './CopyPreview';

export interface FilePreviewProps {
  file: PlannedFileDto;
  /** A thumbnail of the source of a copied texture. */
  thumbnail?: string | undefined;
}

type View = 'changes' | 'file';

function Body({
  file,
  view,
  heightClass,
}: {
  file: PlannedFileDto;
  view: View;
  heightClass: string;
}) {
  if (view === 'changes' && file.diff !== undefined) {
    return (
      <div class={cx('overflow-auto', heightClass)}>
        <DiffView
          diff={file.diff}
          label={t('designer.output.diffLabel', { name: nameOf(file.path) })}
          emptyText={t('designer.output.noChanges')}
        />
      </div>
    );
  }
  return (
    <CodeView
      code={file.rendered}
      label={t('designer.output.codeLabel', { name: nameOf(file.path) })}
      copyLabel={t('designer.output.copy')}
      copiedLabel={t('designer.output.copied')}
      heightClass={heightClass}
    />
  );
}

/**
 * The text of one planned file: the XML as it will be written, or for an update the changes against
 * the file on disk. A wide dialog shows the same text with room to read it.
 */
export function FilePreview({ file, thumbnail }: FilePreviewProps) {
  const [chosen, setChosen] = useState<View>('changes');
  const [wide, setWide] = useState(false);
  if (file.kind === 'copy') return <CopyPreview file={file} thumbnail={thumbnail} />;
  const hasDiff = file.diff !== undefined && file.action === 'update-region';
  const view: View = hasDiff ? chosen : 'file';
  return (
    <div class="flex flex-col gap-2">
      <div class="flex flex-wrap items-center justify-between gap-2">
        {hasDiff ? (
          <SegmentedControl
            label={t('designer.output.viewLabel')}
            value={view}
            onValueChange={(v) => setChosen(v === 'file' ? 'file' : 'changes')}
            options={[
              { value: 'changes', label: t('designer.output.view.changes') },
              { value: 'file', label: t('designer.output.view.file') },
            ]}
          />
        ) : (
          <span class="text-small text-muted">{t('designer.output.view.asWritten')}</span>
        )}
        <Button size="sm" variant="ghost" onClick={() => setWide(true)}>
          {t('designer.output.openWide')}
        </Button>
      </div>
      <Body file={file} view={view} heightClass="max-h-72" />
      <Dialog
        open={wide}
        size="lg"
        title={file.path}
        closeLabel={t('designer.dialog.close')}
        onClose={() => setWide(false)}
      >
        <Body file={file} view={view} heightClass="max-h-[60vh]" />
      </Dialog>
    </div>
  );
}
