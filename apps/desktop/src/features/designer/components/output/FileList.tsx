import type { PlannedFileDto } from 'rimstudio-ipc-types';
import { Badge, Card } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t } from '~/shared/i18n';
import { folderOf, nameOf } from '../../output-model';
import { actionText, actionTone, kindText } from './labels';

export interface FileListProps {
  files: readonly PlannedFileDto[];
  /** The path of the file whose preview is open. */
  selected: string | undefined;
  onSelect: (path: string) => void;
}

/** The files of the plan: path relative to the project, what happens to each, its role and size. */
export function FileList({ files, selected, onSelect }: FileListProps) {
  return (
    <ul aria-label={t('designer.output.files')} class="flex flex-col gap-2">
      {files.map((file) => (
        <li key={file.path}>
          <Card
            selected={file.path === selected}
            onSelect={() => onSelect(file.path)}
            label={t('designer.output.fileLabel', { path: file.path })}
          >
            <span class="flex min-w-0 flex-col gap-1">
              <span class="truncate font-mono text-mono font-semibold text-fg">
                {nameOf(file.path)}
              </span>
              <span class="break-all font-mono text-mono-small text-faint">
                {folderOf(file.path) || t('designer.output.projectRoot')}
              </span>
              <span class="flex flex-wrap items-center gap-2">
                <Badge tone={actionTone(file.action)}>{actionText(file.action)}</Badge>
                <Badge>{kindText(file.kind)}</Badge>
                <span class="font-mono text-mono-small text-muted">{formatBytes(file.bytes)}</span>
              </span>
            </span>
          </Card>
        </li>
      ))}
    </ul>
  );
}
