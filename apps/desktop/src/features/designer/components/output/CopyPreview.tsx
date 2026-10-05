import type { PlannedFileDto } from 'rimstudio-ipc-types';
import { Banner, KeyValueList, type KeyValueItem } from 'rimstudio-ui';
import { formatBytes } from '~/shared/format';
import { t } from '~/shared/i18n';
import { baseName, shortHash } from '../../model/assets';

export interface CopyPreviewProps {
  file: PlannedFileDto;
  /** A `data:image/png` URL of the source, for a texture. */
  thumbnail?: string | undefined;
}

/** A copied file has no text to show: its source, where it goes, its size and its hash. */
export function CopyPreview({ file, thumbnail }: CopyPreviewProps) {
  const copy = file.copy;
  if (!copy) return null;
  const rows: KeyValueItem[] = [
    { key: t('designer.output.copy.source'), value: copy.source, mono: true },
    { key: t('designer.output.copy.target'), value: file.path, mono: true },
    { key: t('designer.output.copy.size'), value: formatBytes(copy.bytes), mono: true },
  ];
  if (copy.width !== undefined && copy.height !== undefined) {
    rows.push({
      key: t('designer.output.copy.dimensions'),
      value: t('designer.assets.fact.pixels', { w: copy.width, h: copy.height }),
      mono: true,
    });
  }
  rows.push({ key: t('designer.output.copy.hash'), value: copy.sha256, mono: true });
  return (
    <div class="flex min-w-0 flex-col gap-3">
      {thumbnail ? (
        <img
          src={thumbnail}
          alt={t('designer.assets.thumbnail', { name: baseName(copy.source) })}
          class="size-32 border border-line bg-bg object-contain"
        />
      ) : null}
      <KeyValueList label={t('designer.output.copy.facts')} items={rows} />
      {copy.existingSha256 ? (
        <Banner tone="warning">
          {t('designer.output.copy.replaces', { hash: shortHash(copy.existingSha256) })}
        </Banner>
      ) : null}
    </div>
  );
}
