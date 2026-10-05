import type { DesignerAssetInfoResponse } from 'rimstudio-ipc-types';
import { Banner, KeyValueList, Spinner, type KeyValueItem } from 'rimstudio-ui';
import { formatBytes, formatDuration, formatNumber } from '~/shared/format';
import { t } from '~/shared/i18n';
import type { FactState } from '../../asset-store';
import { baseName, shortHash } from '../../model/assets';

export interface AssetFactsProps {
  path: string;
  state: FactState | undefined;
}

function kindText(kind: DesignerAssetInfoResponse['kind']): string {
  switch (kind) {
    case 'png':
      return t('designer.assets.kind.png');
    case 'wav':
      return t('designer.assets.kind.wav');
    case 'ogg':
      return t('designer.assets.kind.ogg');
    default:
      return t('designer.assets.kind.unknown');
  }
}

/** The key facts of a read file, as rows. */
export function factRows(info: DesignerAssetInfoResponse): KeyValueItem[] {
  const rows: KeyValueItem[] = [
    { key: t('designer.assets.fact.type'), value: kindText(info.kind) },
  ];
  if (info.bytes !== undefined) {
    rows.push({ key: t('designer.assets.fact.size'), value: formatBytes(info.bytes), mono: true });
  }
  if (info.width !== undefined && info.height !== undefined) {
    rows.push({
      key: t('designer.assets.fact.dimensions'),
      value: t('designer.assets.fact.pixels', { w: info.width, h: info.height }),
      mono: true,
    });
  }
  if (info.durationMs !== undefined) {
    rows.push({
      key: t('designer.assets.fact.length'),
      value: formatDuration(info.durationMs),
      mono: true,
    });
  }
  if (info.channels !== undefined) {
    rows.push({
      key: t('designer.assets.fact.channels'),
      value:
        info.sampleRate !== undefined
          ? t('designer.assets.fact.channelsRate', {
              n: info.channels,
              rate: formatNumber(info.sampleRate, 0),
            })
          : String(info.channels),
      mono: true,
    });
  }
  if (info.sha256) {
    rows.push({ key: t('designer.assets.fact.hash'), value: shortHash(info.sha256), mono: true });
  }
  return rows;
}

/** What the backend found at a file: a thumbnail for a small PNG and the facts of the header. */
export function AssetFacts({ path, state }: AssetFactsProps) {
  if (!state || state.status === 'loading') {
    return (
      <p class="flex items-center gap-2 text-small text-muted">
        <Spinner label={t('designer.assets.reading')} />
        {t('designer.assets.reading')}
      </p>
    );
  }
  if (state.status === 'failed') {
    return (
      <Banner tone="error" title={state.error.code}>
        {state.error.message}
      </Banner>
    );
  }
  const { info } = state;
  if (info.status !== 'found') {
    const reason =
      info.status === 'missing'
        ? t('designer.assets.status.missing')
        : info.status === 'too-large'
          ? t('designer.assets.status.tooLarge')
          : t('designer.assets.status.refused');
    return <p class="text-small text-danger">{reason}</p>;
  }
  return (
    <div class="flex min-w-0 items-start gap-3">
      {info.preview ? (
        <img
          src={info.preview}
          alt={t('designer.assets.thumbnail', { name: baseName(path) })}
          class="size-24 shrink-0 border border-line bg-bg object-contain"
        />
      ) : null}
      <KeyValueList
        label={t('designer.assets.facts', { name: baseName(path) })}
        items={factRows(info)}
      />
    </div>
  );
}
