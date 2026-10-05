import type {
  DiagnosticDto,
  DiagnosticSummaryDto,
  DuplicateGameDto,
  DuplicateReasonDto,
  SourceKindDto,
  SourceStatusDto,
} from 'rimstudio-ipc-types';
import type { MessageKey } from '~/shared/i18n';

/** Package id of Combat Extended. */
export const CE_PACKAGE_ID = 'ceteam.combatextended';

export const SOURCE_KIND_KEYS: Record<SourceKindDto, MessageKey> = {
  'game-data': 'setup.source.kind.game-data',
  'game-mods': 'setup.source.kind.game-mods',
  workshop: 'setup.source.kind.workshop',
  custom: 'setup.source.kind.custom',
};

export const SOURCE_STATUS_KEYS: Record<SourceStatusDto, MessageKey> = {
  ready: 'setup.source.status.ready',
  disabled: 'setup.source.status.disabled',
  offline: 'setup.source.status.offline',
  'not-directory': 'setup.source.status.not-directory',
};

/** What the game does with a copy of a duplicated mod. */
export const DUPLICATE_GAME_KEYS: Record<DuplicateGameDto, MessageKey> = {
  loaded: 'setup.dup.game.loaded',
  rejected: 'setup.dup.game.rejected',
  'not-visible': 'setup.dup.game.not-visible',
};

/** The rung of the choice ladder that decided which copy is kept. */
export const DUPLICATE_REASON_KEYS: Record<DuplicateReasonDto, MessageKey> = {
  available: 'setup.dup.reason.available',
  pinned: 'setup.dup.reason.pinned',
  'source-priority': 'setup.dup.reason.source-priority',
  'version-match': 'setup.dup.reason.version-match',
  newer: 'setup.dup.reason.newer',
  'path-order': 'setup.dup.reason.path-order',
};

/** One group of the diagnostics list: a code, its total and the samples the backend kept. */
export interface DiagnosticGroup {
  code: string;
  count: number;
  severity: DiagnosticDto['severity'];
  samples: DiagnosticDto[];
}

const SEVERITY_ORDER: Record<DiagnosticDto['severity'], number> = {
  error: 0,
  warning: 1,
  info: 2,
  hint: 3,
};

/**
 * Group the summary by code for display. Counts come from the backend; samples are the ones it
 * kept. Order is severity first (worst first), then count, then code.
 */
export function groupDiagnostics(summary: DiagnosticSummaryDto): DiagnosticGroup[] {
  const groups = new Map<string, DiagnosticGroup>();
  for (const [code, count] of Object.entries(summary.counts)) {
    groups.set(code, { code, count, severity: 'info', samples: [] });
  }
  for (const sample of summary.samples) {
    const group = groups.get(sample.code) ?? {
      code: sample.code,
      count: 0,
      severity: sample.severity,
      samples: [],
    };
    group.severity = sample.severity;
    group.samples.push(sample);
    groups.set(sample.code, group);
  }
  return [...groups.values()].sort(
    (a, b) =>
      SEVERITY_ORDER[a.severity] - SEVERITY_ORDER[b.severity] ||
      b.count - a.count ||
      a.code.localeCompare(b.code),
  );
}
