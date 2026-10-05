import type { FitLevelDto, FitReportDto, StatFitDto, SuggestionDto } from 'rimstudio-ipc-types';
import { Banner, Button, Meter, Panel } from 'rimstudio-ui';
import { formatNumber } from '~/shared/format';
import { t, type MessageKey } from '~/shared/i18n';
import { statLabel } from '../../model/labels';

export interface FitPanelProps {
  report: FitReportDto | undefined;
  /** Suggestions of the preview, to find the field behind a stat. */
  suggestions: readonly SuggestionDto[];
  calibrating: boolean;
  onCalibrate: () => void;
  /** Set a field to the predicted value of its stat. */
  onSetPredicted: (pointer: string, value: number) => void;
}

const LEVEL_TEXT: Record<FitLevelDto, MessageKey> = {
  typical: 'designer.fit.typical',
  plausible: 'designer.fit.plausible',
  unusual: 'designer.fit.unusual',
};

function MeterRow({ fit, onSet }: { fit: StatFitDto; onSet?: () => void }) {
  // the scale is rounded for display only, then widened so the value always sits inside it
  const round = (n: number): number => Number(n.toPrecision(3));
  const low = Math.min(round(Math.min(fit.p80.low, fit.value, fit.predicted)), fit.value);
  const high = Math.max(round(Math.max(fit.p80.high, fit.value, fit.predicted)), fit.value);
  return (
    <div class="flex flex-col gap-1">
      <Meter
        label={statLabel(fit.stat)}
        value={fit.value}
        min={low}
        max={high}
        p50={[fit.p50.low, fit.p50.high]}
        p80={[fit.p80.low, fit.p80.high]}
        prediction={fit.predicted}
        level={fit.level}
        valueText={formatNumber(fit.value, 3)}
        levelText={t(LEVEL_TEXT[fit.level])}
      />
      {fit.level === 'unusual' ? (
        <div class="flex items-center justify-between gap-2 text-small text-muted">
          <span>{t('designer.fit.nearest', { value: formatNumber(fit.nearestReference, 3) })}</span>
          {onSet ? (
            <Button size="sm" variant="ghost" onClick={onSet}>
              {t('designer.fit.setPredicted', { value: formatNumber(fit.predicted, 3) })}
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

/** The fit meter: where each typed number sits against the bands of the reference weapons. */
export function FitPanel({
  report,
  suggestions,
  calibrating,
  onCalibrate,
  onSetPredicted,
}: FitPanelProps) {
  const calibrate = (
    <Button size="sm" loading={calibrating} onClick={onCalibrate}>
      {t('designer.fit.calibrate')}
    </Button>
  );
  if (!report) {
    return (
      <Panel title={t('designer.panel.fit')} actions={calibrate}>
        <p class="text-small text-muted">{t('designer.fit.waiting')}</p>
      </Panel>
    );
  }
  const { summary, notices } = report;
  const defaultBands = report.perStat.length > 0 && report.perStat.every((s) => s.defaultBand);
  const pointerOf = (stat: string): string | undefined =>
    suggestions.find((s) => s.stat === stat)?.field;
  return (
    <Panel title={t('designer.panel.fit')} actions={calibrate}>
      <div class="flex flex-col gap-3">
        <div class="flex flex-wrap items-baseline gap-x-6 gap-y-1">
          {report.typicality !== undefined ? (
            <span
              class="font-mono text-mono-display tabular-nums"
              aria-label={t('designer.fit.typicality')}
            >
              {formatNumber(report.typicality, 0)}
              <span class="ml-1 text-mono-small text-faint">/ 100</span>
            </span>
          ) : null}
          <span class="text-small text-muted">
            {t('designer.fit.summary', {
              typical: summary.typical,
              plausible: summary.plausible,
              unusual: summary.unusual,
            })}
          </span>
        </div>
        <p class="text-small text-faint">{t('designer.fit.typicalityHelp')}</p>
        <p class="text-small text-muted">{notices.classLabel}</p>
        {defaultBands ? <Banner tone="info">{t('designer.fit.defaultBands')}</Banner> : null}
        {notices.rough ? <Banner tone="info">{t('designer.fit.rough')}</Banner> : null}
        {notices.optimistic ? <Banner tone="warning">{t('designer.fit.optimistic')}</Banner> : null}
        {report.perStat.length === 0 ? (
          <p class="text-small text-muted">{t('designer.fit.none')}</p>
        ) : (
          <div class="grid grid-cols-1 gap-x-6 gap-y-4 xl:grid-cols-2">
            {report.perStat.map((fit) => {
              const pointer = pointerOf(fit.stat);
              return (
                <MeterRow
                  key={fit.stat}
                  fit={fit}
                  {...(pointer ? { onSet: () => onSetPredicted(pointer, fit.predicted) } : {})}
                />
              );
            })}
          </div>
        )}
      </div>
    </Panel>
  );
}
