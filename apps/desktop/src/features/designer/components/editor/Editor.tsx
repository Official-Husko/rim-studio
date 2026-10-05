import { useRef, useState } from 'preact/hooks';
import { Banner, Badge } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import * as api from '../../api';
import { isCeRequiredMissing } from '../../output-model';
import { importProblems } from '../../model/assets';
import { focusField, groupByField, suggested, suggestionsByField } from '../../model/draft';
import type { Designer } from '../../stores';
import { ClonePanel } from './ClonePanel';
import { CloneNotes } from './CloneNotes';
import { CombatPanel } from './CombatPanel';
import { CostPanel } from './CostPanel';
import { DiagnosticList } from './DiagnosticList';
import { EstimatePanel } from './EstimatePanel';
import { FieldEnvContext, type FieldEnv } from './fieldEnv';
import { FitPanel } from './FitPanel';
import { IdentityPanel } from './IdentityPanel';
import { LookPanel } from './LookPanel';
import { OtherFieldsPanel } from './OtherFieldsPanel';
import { ProjectilePanel } from './ProjectilePanel';
import { QuizStepper } from './QuizStepper';
import { ReadoutsPanel } from './ReadoutsPanel';
import { SaveIndicator } from './SaveIndicator';
import { SoundsPanel } from './SoundsPanel';
import { StructureBanner } from './StructureBanner';
import { TagsPanel } from './TagsPanel';
import { TexturePanel } from './TexturePanel';
import { ToolsPanel } from './ToolsPanel';

export interface EditorProps {
  designer: Designer;
}

/** The form of the open draft: live readouts, the fit meter, the estimate and the field panels. */
export function Editor({ designer }: EditorProps) {
  const { editor, reference, quiz, drafts } = designer;
  const draft = editor.draft.value;
  const preview = editor.preview.value;
  const root = useRef<HTMLDivElement>(null);
  const [calibrating, setCalibrating] = useState(false);
  if (!draft) return null;

  const plan = designer.output.plan.value;
  const grouped = groupByField([...(preview?.diagnostics ?? []), ...importProblems(plan)]);
  const env: FieldEnv = {
    spec: draft.spec,
    suggestions: suggestionsByField(preview?.suggestions),
    pools: reference.pools.value,
    diagnostics: grouped,
    setField: editor.setField,
    setOwnProjectile: (own) => void editor.setOwnProjectile(own),
    ownBusy: editor.ownBusy.value,
    projectileNotes: editor.projectileNotes.value,
    roles: reference.roles.value,
  };
  const kind = draft.kind;
  const identity = draft.spec.identity;
  const error = editor.error.value;

  const calibrate = async (): Promise<void> => {
    setCalibrating(true);
    try {
      await api.calibrate(kind, true);
      editor.refresh();
    } catch {
      /* the task centre shows the failed job */
    } finally {
      setCalibrating(false);
    }
  };

  return (
    <FieldEnvContext.Provider value={env}>
      <div ref={root} class="flex flex-col gap-3 p-3" onFocusOut={() => void editor.flush()}>
        <header class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex min-w-0 items-center gap-2">
            <h2 class="truncate text-title font-semibold text-fg">
              {identity.label || identity.defName}
            </h2>
            <span class="font-mono text-mono-small text-faint">{identity.defName}</span>
            <Badge>
              {kind === 'ranged' ? t('designer.kind.ranged') : t('designer.kind.melee')}
            </Badge>
          </div>
          <SaveIndicator state={editor.saveState.value} />
        </header>
        {error ? (
          <Banner tone="error" title={error.code}>
            {error.message}
          </Banner>
        ) : null}
        {drafts.notesFor.value !== undefined && drafts.notesFor.value === editor.entryId.value ? (
          <CloneNotes notes={drafts.notes.value} onDismiss={() => (drafts.notes.value = [])} />
        ) : null}
        <StructureBanner suggestion={editor.structure.value} onApply={editor.applyStructure} />
        <ReadoutsPanel readouts={preview?.readouts} busy={editor.previewing.value} />
        <DiagnosticList
          diagnostics={(preview?.diagnostics ?? []).filter((d) => !isCeRequiredMissing(d))}
          onFocusField={(pointer) => root.current && focusField(root.current, pointer)}
        />
        <ClonePanel diff={editor.diff.value} />
        <FitPanel
          report={editor.fit.value}
          suggestions={preview?.suggestions ?? []}
          calibrating={calibrating}
          onCalibrate={() => void calibrate()}
          onSetPredicted={(pointer, value) => editor.setField(pointer, suggested(value))}
        />
        <EstimatePanel
          draft={draft}
          estimate={preview?.estimate}
          onChange={editor.edit}
          onFill={editor.fillFromEstimate}
          onStartQuiz={() => void quiz.start()}
        />
        <IdentityPanel />
        <CombatPanel kind={kind} />
        {kind === 'ranged' ? <ProjectilePanel /> : null}
        <CostPanel kind={kind} />
        <ToolsPanel kind={kind} />
        <TagsPanel />
        <TexturePanel assets={designer.assets} plan={plan} />
        <LookPanel />
        <SoundsPanel assets={designer.assets} plan={plan} />
        <OtherFieldsPanel />
        <QuizStepper store={quiz} />
      </div>
    </FieldEnvContext.Provider>
  );
}
