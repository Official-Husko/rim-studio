import { useEffect, useState } from 'preact/hooks';
import type { ConvertCandidateDto } from 'rimstudio-ipc-types';
import { Banner, KeyValueList, Tabs } from 'rimstudio-ui';
import { MissingList } from '~/shared/ce';
import { t, tn } from '~/shared/i18n';
import type { ProjectRef } from '~/shared/project';
import { familyAnswers, ownAnswers } from './answerStore';
import { devLink } from './devLinks';
import { DefinitionView } from './DefinitionView';
import { ExistingPatch } from './ExistingPatch';
import { familyOf, isConvertible, projectileOf } from './model';
import { PlanView } from './PlanView';
import { loadPlan, plans } from './planStore';
import { openRemoval, removalTarget } from './removeStore';
import { OptionsForm } from './OptionsForm';
import { QuestionsForm } from './QuestionsForm';
import { chipsOf } from './candidateView';
import { StatusChip } from './StatusChip';
import { TagChips } from './TagChips';
import { focusAsk, missingOf } from './missing';
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
    // the plan is asked for again when the weapon, the project or the answers change, and when the scan
    // was repeated (a run changed the files and dropped the plans, so the weapon is a new object)
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [candidate, project.path, answersKey, planned]);

  const projectile = projectileOf(candidate);
  const missing = convertible ? missingOf(candidate, entry?.plan?.diagnostics) : [];
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
            ...(chipsOf(candidate).length > 0
              ? [
                  {
                    key: t('patches.detail.tag'),
                    value: (
                      <TagChips
                        tags={candidate.tags ?? []}
                        classes={candidate.weaponClasses ?? []}
                      />
                    ),
                  },
                ]
              : []),
            ...(projectile
              ? [{ key: t('patches.detail.projectile'), value: projectile, mono: true }]
              : []),
            ...(candidate.file
              ? [{ key: t('patches.detail.file'), value: candidate.file, mono: true }]
              : []),
          ]}
        />
      </header>
      {candidate.status === 'already-ce' && candidate.existing ? (
        <ExistingPatch
          existing={candidate.existing}
          onRemove={() => void openRemoval(project, removalTarget(candidate, 'remove'))}
          onReplace={() => void openRemoval(project, removalTarget(candidate, 'replace'))}
        />
      ) : null}
      {!planned ? (
        <Banner tone="warning">
          {candidate.status === 'unsupported-kind'
            ? t('patches.detail.not-convertible.unsupported')
            : t('patches.detail.not-convertible.target')}
        </Banner>
      ) : null}
      {tab !== 'questions' && missing.length > 0 ? (
        <MissingList
          title={tn('patches.missing.title', missing.length)}
          items={missing}
          goLabel={t('patches.missing.go')}
          onGoTo={(field) => {
            setTab('questions');
            focusAsk(field);
          }}
          footer={t('patches.missing.footer')}
        />
      ) : null}
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
