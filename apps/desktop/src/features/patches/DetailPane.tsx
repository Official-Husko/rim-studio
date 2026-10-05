import { useEffect, useState } from 'preact/hooks';
import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { Banner, KeyValueList, Tabs } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { familyAnswers, ownAnswers } from './answerStore';
import { devLink } from './devLinks';
import { DefinitionView } from './DefinitionView';
import { familyOf, familyParts, isConvertible } from './model';
import { PlanView } from './PlanView';
import { loadPlan, plans } from './planStore';
import { OptionsForm } from './OptionsForm';
import { QuestionsForm } from './QuestionsForm';
import { StatusChip } from './StatusChip';
import { loadChoices } from './suggestStore';

export interface DetailPaneProps {
  project: ProjectRef;
  candidate: ConvertCandidateDto;
  familySize: number;
}

/** How long the plan waits after the last answer before it is asked for again. */
const PLAN_DELAY_MS = 350;

/** One weapon: what the scan knows, the questions, the generated plan and the definition file. */
export function DetailPane({ project, candidate, familySize }: DetailPaneProps) {
  const convertible = isConvertible(candidate);
  const planned = convertible || candidate.status === 'already-ce';
  // the page remounts this pane for every weapon, so the first tab is chosen once
  const [tab, setTab] = useState(
    () =>
      devLink('tab') ??
      (convertible && candidate.asks.length > 0 ? 'questions' : planned ? 'plan' : 'definition'),
  );
  // reading the answers here makes the plan follow them
  const own = ownAnswers.value[candidate.defName];
  const shared = familyOf(candidate) ? familyAnswers.value[familyOf(candidate)] : undefined;
  const entry = plans.value[candidate.defName];
  const answersKey = JSON.stringify([own, shared]);

  useEffect(() => {
    if (convertible) void loadChoices(candidate);
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [candidate.defName]);

  useEffect(() => {
    if (!planned) return;
    const timer = setTimeout(() => void loadPlan(project, candidate), PLAN_DELAY_MS);
    return () => clearTimeout(timer);
    // the plan is asked for again when the weapon, the project or the answers change
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [candidate.defName, project.path, answersKey, planned]);

  const parts = familyParts(familyOf(candidate));
  const open = entry?.plan
    ? entry.plan.diagnostics.filter((d) => d.code === 'designer.convert-needs-answer').length
    : undefined;
  const tabs = [
    ...(convertible
      ? [
          {
            id: 'questions',
            label: t('patches.tab.questions'),
            badge: String(open ?? candidate.asks.length),
          },
        ]
      : []),
    ...(convertible ? [{ id: 'options', label: t('patches.tab.options') }] : []),
    ...(planned ? [{ id: 'plan', label: t('patches.tab.plan') }] : []),
    { id: 'definition', label: t('patches.tab.definition') },
  ];
  return (
    <div class="flex min-h-0 flex-col gap-4 p-4">
      <header class="flex flex-col gap-2">
        <div class="flex flex-wrap items-center gap-2">
          <h2 class="m-0 font-display text-heading font-semibold">
            {candidate.label || candidate.defName}
          </h2>
          <StatusChip status={candidate.status} />
        </div>
        <p class="m-0 text-small text-muted">{candidate.reason}</p>
        <KeyValueList
          label={t('patches.detail.facts')}
          items={[
            { key: t('patches.detail.def'), value: candidate.defName, mono: true },
            ...(candidate.kind
              ? [
                  {
                    key: t('patches.detail.kind'),
                    value: t(
                      candidate.kind === 'ranged' ? 'patches.kind.ranged' : 'patches.kind.melee',
                    ),
                  },
                ]
              : []),
            ...(parts.tag ? [{ key: t('patches.detail.tag'), value: parts.tag, mono: true }] : []),
            ...(parts.projectile
              ? [{ key: t('patches.detail.projectile'), value: parts.projectile, mono: true }]
              : []),
            ...(candidate.file
              ? [{ key: t('patches.detail.file'), value: candidate.file, mono: true }]
              : []),
          ]}
        />
      </header>
      {!planned ? <Banner tone="warning">{t('patches.detail.not-convertible')}</Banner> : null}
      <Tabs tabs={tabs} value={tab} onValueChange={setTab} label={t('patches.detail.tabs')}>
        {(id) => (
          <div class="pt-3">
            {id === 'questions' ? (
              <QuestionsForm candidate={candidate} familySize={familySize} project={project} />
            ) : null}
            {id === 'options' ? (
              <OptionsForm candidate={candidate} familySize={familySize} />
            ) : null}
            {id === 'plan' ? (
              <PlanView
                candidate={candidate}
                entry={entry}
                onRetry={() => void loadPlan(project, candidate, true)}
                onOpenQuestions={() => setTab('questions')}
              />
            ) : null}
            {id === 'definition' ? (
              <DefinitionView project={project} file={candidate.file} />
            ) : null}
          </div>
        )}
      </Tabs>
    </div>
  );
}
