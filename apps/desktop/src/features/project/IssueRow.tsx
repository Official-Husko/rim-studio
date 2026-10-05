import { Badge, Button, type BadgeTone } from 'rimstudio-ui';
import type { LayoutIssueDto, SeverityDto } from 'rimstudio-ipc-types';
import type { MessageKey } from '~/shared/i18n';
import { t } from '~/shared/i18n';
import { hasPlannedFix } from './model';

const TONES: Record<SeverityDto, BadgeTone> = {
  error: 'danger',
  warning: 'warning',
  info: 'info',
  hint: 'neutral',
};

const SEVERITY_KEYS: Record<SeverityDto, MessageKey> = {
  error: 'project.severity.error',
  warning: 'project.severity.warning',
  info: 'project.severity.info',
  hint: 'project.severity.hint',
};

/** A short title for each layout code; an unknown code shows the code itself. */
export const ISSUE_TITLES: Record<string, MessageKey> = {
  'layout.missing-about': 'project.issue.missing-about',
  'layout.missing-folder': 'project.issue.missing-folder',
  'layout.folder-case': 'project.issue.folder-case',
  'layout.weapon-misplaced': 'project.issue.weapon-misplaced',
  'layout.def-wrong-category': 'project.issue.def-wrong-category',
  'layout.ce-outside-gate': 'project.issue.ce-outside-gate',
  'layout.ce-folder-ungated': 'project.issue.ce-folder-ungated',
  'layout.ce-legacy-folder': 'project.issue.ce-legacy-folder',
  'layout.load-folders-missing-folder': 'project.issue.load-folders-missing-folder',
  'layout.wrong-root': 'project.issue.wrong-root',
  'layout.unparsable-file': 'project.issue.unparsable-file',
  'layout.texture-missing': 'project.issue.texture-missing',
};

export interface IssueRowProps {
  issue: LayoutIssueDto;
  onShowPath: (path: string) => void;
  /** Opens the fix plan for this finding; when absent no Fix button is shown. */
  onFix?: (issue: LayoutIssueDto) => void;
}

/** One layout issue: severity, title, the path, the explanation and the suggested fix. */
export function IssueRow({ issue, onShowPath, onFix }: IssueRowProps) {
  const titleKey = ISSUE_TITLES[issue.code];
  const hasFix = issue.fix.kind !== 'none';
  return (
    <li class="flex flex-col gap-1 px-3 py-2">
      <div class="flex flex-wrap items-center gap-2">
        <Badge tone={TONES[issue.severity]}>{t(SEVERITY_KEYS[issue.severity])}</Badge>
        <span class="font-semibold">{titleKey ? t(titleKey) : issue.code}</span>
        {issue.fix.kind === 'create-folder' ? (
          <span class="font-mono text-mono text-muted">{issue.path}</span>
        ) : (
          <Button size="sm" variant="ghost" onClick={() => onShowPath(issue.path)}>
            <span class="font-mono text-mono">{issue.path}</span>
          </Button>
        )}
        {issue.fix.automatic ? <Badge tone="success">{t('project.issue.automatic')}</Badge> : null}
        {onFix && hasPlannedFix(issue) ? (
          <>
            <span class="flex-1" />
            <Button
              size="sm"
              icon="check"
              aria-label={t('project.fix.issueLabel', { path: issue.path })}
              onClick={() => onFix(issue)}
            >
              {t('project.fix.issue')}
            </Button>
          </>
        ) : null}
      </div>
      <p class="m-0 text-small">{issue.message}</p>
      {hasFix ? (
        <p class="m-0 text-small text-muted">
          <span class="font-semibold">
            {issue.fix.automatic ? t('project.issue.fix') : t('project.issue.suggestion')}
          </span>{' '}
          {issue.fix.summary}
        </p>
      ) : null}
    </li>
  );
}
