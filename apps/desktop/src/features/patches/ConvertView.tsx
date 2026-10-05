import { useEffect, useMemo, useState } from 'preact/hooks';
import { EmptyState, SplitPane, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { DiagnosticList } from './DiagnosticList';
import { CandidateTable } from './CandidateTable';
import { DetailPane } from './DetailPane';
import { openReview } from './applyStore';
import { familyOf, isConvertible } from './model';
import { ScanBar } from './ScanBar';
import { ScanProblem } from './ScanProblem';
import {
  checked,
  focused,
  includeConverted,
  runScan,
  scan,
  setAllChecked,
  setChecked,
} from './scanStore';

/** True when the window is wide enough for the list and the weapon side by side. */
function useWide(): boolean {
  const query = '(min-width: 1180px)';
  const [wide, setWide] = useState(() => window.matchMedia?.(query).matches ?? true);
  useEffect(() => {
    const media = window.matchMedia?.(query);
    if (!media) return;
    const onChange = (): void => setWide(media.matches);
    media.addEventListener('change', onChange);
    return () => media.removeEventListener('change', onChange);
  }, []);
  return wide;
}

export interface ConvertViewProps {
  project: ProjectRef;
}

/** The convert tab: the scan table on the left, the focused weapon on the right. */
export function ConvertView({ project }: ConvertViewProps) {
  const wide = useWide();
  const state = scan.value;
  const selected = checked.value;
  const focusName = focused.value;
  const data = state.phase === 'ready' || state.phase === 'loading' ? state.data : undefined;
  const list = data?.candidates ?? [];
  const families = useMemo(() => {
    const sizes = new Map<string, number>();
    for (const c of data?.candidates ?? []) {
      const family = familyOf(c);
      if (family && isConvertible(c)) sizes.set(family, (sizes.get(family) ?? 0) + 1);
    }
    return sizes;
  }, [data]);

  if (state.phase === 'idle' || (state.phase === 'loading' && !data)) {
    return <Spinner label={t('patches.scan.running')} />;
  }
  if (state.phase === 'error') {
    return <ScanProblem error={state.error} onRetry={() => void runScan(project)} />;
  }
  if (!data) return null;
  if (data.candidates.length === 0) {
    return (
      <div class="flex flex-col gap-3">
        <EmptyState
          icon="crosshair"
          title={t('patches.empty.title')}
          description={t('patches.empty.body')}
        />
        {data.diagnostics.length > 0 ? (
          <DiagnosticList diagnostics={data.diagnostics} label={t('patches.scan.diagnostics')} />
        ) : null}
      </div>
    );
  }
  const current = list.find((c) => c.defName === focusName);
  const convertible = list.filter(isConvertible);
  return (
    <div class="min-h-0 flex-1 overflow-hidden rounded-sm border border-line">
      <SplitPane
        label={t('patches.split.label')}
        direction={wide ? 'horizontal' : 'vertical'}
        defaultSize={wide ? 640 : 340}
        min={wide ? 440 : 200}
        max={wide ? 900 : 560}
        first={
          <div class="flex flex-col">
            <ScanBar
              counts={data.counts}
              includeConverted={includeConverted.value}
              onIncludeConverted={(on) => {
                includeConverted.value = on;
                void runScan(project);
              }}
              onRescan={() => void runScan(project)}
              rescanning={state.phase === 'loading'}
              convertible={convertible.length}
              checked={convertible.filter((c) => selected.has(c.defName)).length}
              onCheckAll={setAllChecked}
              onConvert={() =>
                void openReview(
                  project,
                  convertible.filter((c) => selected.has(c.defName)),
                )
              }
            />
            {data.diagnostics.length > 0 ? (
              <div class="px-3 pb-3">
                <DiagnosticList
                  diagnostics={data.diagnostics}
                  label={t('patches.scan.diagnostics')}
                />
              </div>
            ) : null}
            <CandidateTable
              candidates={list}
              focused={focusName}
              onFocus={(name) => {
                focused.value = name;
              }}
              checked={selected}
              onCheck={setChecked}
            />
          </div>
        }
        second={
          current ? (
            <DetailPane
              key={current.defName}
              project={project}
              candidate={current}
              familySize={families.get(familyOf(current)) ?? 1}
            />
          ) : (
            <EmptyState compact icon="crosshair" title={t('patches.detail.none')} />
          )
        }
      />
    </div>
  );
}
