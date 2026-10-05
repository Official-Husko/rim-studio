import { Badge, Banner, CodeView, EmptyState, Spinner } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t } from '~/shared/i18n';
import { extensionOf, ROLE_LABEL } from './model';
import type { FileView } from './store';

export interface FileViewerProps {
  file: FileView | undefined;
}

/** The text of one project file, with xml colouring, or the reason it cannot be shown. */
export function FileViewer({ file }: FileViewerProps) {
  if (!file) {
    return (
      <EmptyState
        compact
        icon="file"
        title={t('project.file.empty.title')}
        description={t('project.file.empty.hint')}
      />
    );
  }
  const dto = file.file;
  return (
    <div class="flex min-h-0 flex-col gap-3 p-3">
      <div class="flex flex-wrap items-center gap-2">
        <span class="min-w-0 flex-1 truncate font-mono text-mono" title={file.path}>
          {file.path}
        </span>
        {dto ? (
          <>
            <Badge tone="info">{t(ROLE_LABEL[dto.role])}</Badge>
            <Badge>{formatBytes(dto.bytes)}</Badge>
          </>
        ) : null}
      </div>
      {file.loading ? <Spinner label={t('project.file.loading')} /> : null}
      {file.error ? (
        <Banner tone="error" title={t('project.file.error')}>
          {file.error.message}
        </Banner>
      ) : null}
      {dto?.binary ? (
        <EmptyState
          compact
          icon="image"
          title={t('project.file.binary.title')}
          description={t('project.file.binary.hint', { size: formatBytes(dto.bytes) })}
        />
      ) : null}
      {dto && !dto.binary ? (
        <>
          {dto.truncated ? (
            <Banner tone="info">
              {t('project.file.truncated', {
                shown: formatBytes(new TextEncoder().encode(dto.text).length),
                size: formatBytes(dto.bytes),
              })}
            </Banner>
          ) : null}
          <CodeView
            code={dto.text}
            language={extensionOf(file.path) === 'xml' ? 'xml' : 'text'}
            label={t('project.file.code', { path: file.path })}
            copyLabel={t('project.file.copy')}
            copiedLabel={t('project.file.copied')}
            heightClass="max-h-[60vh]"
          />
        </>
      ) : null}
    </div>
  );
}
