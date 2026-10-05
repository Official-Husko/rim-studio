import { useState } from 'preact/hooks';
import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { Badge, Button, Panel } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';
import { severityText } from '../editor/DiagnosticNotes';
import { severityTone } from './labels';

export interface PlanProblemsProps {
  /** Sorted, errors first. */
  problems: readonly DiagnosticDto[];
  /** Move the focus to the field a pointer names. */
  onGoTo: (pointer: string) => void;
}

function Row({ d, onGoTo }: { d: DiagnosticDto; onGoTo: (pointer: string) => void }) {
  return (
    <li class="flex flex-col gap-1 text-body">
      <div class="flex items-start gap-2">
        <Badge tone={severityTone(d.severity)}>{severityText(d.severity)}</Badge>
        <span class="min-w-0 flex-1 break-words">{d.message}</span>
      </div>
      <div class="flex items-center justify-between gap-2 pl-1">
        <span class="min-w-0 break-all font-mono text-mono-small text-faint">{d.code}</span>
        {d.field?.startsWith('/') ? (
          <Button size="sm" variant="ghost" onClick={() => onGoTo(d.field as string)}>
            {t('designer.diagnostics.goTo')}
          </Button>
        ) : null}
      </div>
    </li>
  );
}

/**
 * Every problem of the plan with its code. Errors and warnings are always shown; the notes (info and
 * hints) sit behind a button. A problem with a field pointer links back to the field.
 */
export function PlanProblems({ problems, onGoTo }: PlanProblemsProps) {
  const [showNotes, setShowNotes] = useState(false);
  const errors = problems.filter((d) => d.severity === 'error').length;
  const main = problems.filter((d) => d.severity === 'error' || d.severity === 'warning');
  const notes = problems.filter((d) => d.severity !== 'error' && d.severity !== 'warning');
  return (
    <Panel
      title={t('designer.output.problems')}
      collapsible
      actions={
        <span class="font-mono text-mono-small text-muted">
          {errors > 0
            ? tn('designer.diagnostics.errors', errors)
            : t('designer.diagnostics.noErrors')}
        </span>
      }
    >
      <div class="flex flex-col gap-3">
        {problems.length === 0 ? (
          <p class="text-small text-muted">{t('designer.diagnostics.none')}</p>
        ) : null}
        {main.length > 0 ? (
          <ul class="flex flex-col gap-2">
            {main.map((d, i) => (
              <Row key={`${d.code}-${d.field ?? ''}-${i}`} d={d} onGoTo={onGoTo} />
            ))}
          </ul>
        ) : null}
        {notes.length > 0 ? (
          <div class="flex flex-col gap-2">
            <div>
              <Button
                size="sm"
                variant="ghost"
                aria-expanded={showNotes}
                onClick={() => setShowNotes(!showNotes)}
              >
                {showNotes
                  ? tn('designer.output.hideNotes', notes.length)
                  : tn('designer.output.showNotes', notes.length)}
              </Button>
            </div>
            {showNotes ? (
              <ul class="flex flex-col gap-2">
                {notes.map((d, i) => (
                  <Row key={`${d.code}-${d.field ?? ''}-${i}`} d={d} onGoTo={onGoTo} />
                ))}
              </ul>
            ) : null}
          </div>
        ) : null}
      </div>
    </Panel>
  );
}
