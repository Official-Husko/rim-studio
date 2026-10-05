import { useState } from 'preact/hooks';
import type { PlannedFileDto } from 'rimstudio-ipc-types';
import { Badge, CodeView, DiffView, SegmentedControl } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t } from '~/shared/i18n';

const ACTION = {
  create: 'patches.action.create',
  'update-region': 'patches.action.update-region',
  unchanged: 'patches.action.unchanged',
} as const;

const KIND = {
  'vanilla-defs': 'patches.file-kind.vanilla-defs',
  'ce-patch': 'patches.file-kind.ce-patch',
  'load-folders': 'patches.file-kind.load-folders',
  about: 'patches.file-kind.about',
} as const;

export interface PlanFileProps {
  file: PlannedFileDto;
}

/** One planned file: its path, what the plan does to it and the text or the diff. */
export function PlanFile({ file }: PlanFileProps) {
  const [view, setView] = useState<'text' | 'diff'>(file.diff ? 'diff' : 'text');
  return (
    <section class="flex flex-col gap-2 rounded-sm border border-line p-3" aria-label={file.path}>
      <div class="flex flex-wrap items-center gap-2">
        <code class="min-w-0 flex-1 truncate font-mono text-mono" title={file.path}>
          {file.path}
        </code>
        <Badge tone="neutral">{t(KIND[file.kind])}</Badge>
        <Badge tone={file.action === 'create' ? 'success' : 'info'}>{t(ACTION[file.action])}</Badge>
        <span class="font-mono text-mono-small text-muted">{formatBytes(file.bytes)}</span>
      </div>
      {file.diff ? (
        <SegmentedControl
          label={t('patches.file.view', { path: file.path })}
          value={view}
          onValueChange={(v) => setView(v === 'diff' ? 'diff' : 'text')}
          options={[
            { value: 'diff', label: t('patches.file.diff') },
            { value: 'text', label: t('patches.file.text') },
          ]}
        />
      ) : null}
      {view === 'diff' && file.diff ? (
        <DiffView diff={file.diff} label={t('patches.file.diff-label', { path: file.path })} />
      ) : (
        <CodeView
          language="xml"
          code={file.rendered}
          label={t('patches.file.text-label', { path: file.path })}
          lineNumbers
          heightClass="max-h-80"
        />
      )}
    </section>
  );
}
