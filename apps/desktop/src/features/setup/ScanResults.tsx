import { KeyValueList, Table, type TableColumn } from 'rimstudio-ui';
import type { LibraryScanResult, SourceReportDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { formatBytes, formatDuration, formatNumber } from '~/shared/format';
import { DiagnosticGroups } from './DiagnosticGroups';
import { SOURCE_KIND_KEYS, SOURCE_STATUS_KEYS } from './model';

const COLUMNS: TableColumn<SourceReportDto>[] = [
  { key: 'kind', header: t('setup.scan.col.source'), render: (r) => t(SOURCE_KIND_KEYS[r.kind]) },
  {
    key: 'status',
    header: t('setup.scan.col.status'),
    render: (r) => t(SOURCE_STATUS_KEYS[r.status]),
  },
  {
    key: 'mods',
    header: t('setup.scan.col.mods'),
    render: (r) => formatNumber(r.mods, 0),
    align: 'right',
    mono: true,
  },
];

/** The result of a finished scan: counts, timings, sources and diagnostics. */
export function ScanResults({ result }: { result: LibraryScanResult }) {
  const { stats, timings } = result;
  return (
    <div class="flex flex-col gap-4">
      <KeyValueList
        label={t('setup.scan.counts')}
        items={[
          { key: t('setup.scan.mods'), value: formatNumber(stats.modsFound, 0), mono: true },
          { key: t('setup.scan.indexed'), value: formatNumber(stats.modsIndexed, 0), mono: true },
          { key: t('setup.scan.defs'), value: formatNumber(stats.defs, 0), mono: true },
          {
            key: t('setup.scan.deffiles'),
            value: t('setup.scan.parsed', {
              parsed: formatNumber(stats.defFilesParsed, 0),
              reused: formatNumber(stats.defFilesReused, 0),
            }),
            mono: true,
          },
          { key: t('setup.scan.read'), value: formatBytes(stats.bytesRead), mono: true },
          {
            key: t('setup.scan.time'),
            value: t('setup.scan.phases', {
              total: formatDuration(timings.totalMs),
              discover: formatDuration(timings.discoverMs),
              metadata: formatDuration(timings.metadataMs),
              defs: formatDuration(timings.definitionsMs),
            }),
            mono: true,
          },
        ]}
      />
      <div class="overflow-x-auto">
        <Table
          label={t('setup.scan.sources')}
          columns={COLUMNS}
          rows={result.sources}
          getKey={(r) => r.id}
          dense
        />
      </div>
      <div class="flex flex-col gap-2">
        <h3 class="m-0 text-small font-semibold text-muted">
          {t('setup.scan.diag.title')}
          <span class="ml-2 font-normal">
            {t('setup.scan.diag.totals', {
              errors: result.diagnostics.errors,
              warnings: result.diagnostics.warnings,
              infos: result.diagnostics.infos,
              hints: result.diagnostics.hints,
            })}
          </span>
        </h3>
        <DiagnosticGroups summary={result.diagnostics} />
      </div>
    </div>
  );
}
