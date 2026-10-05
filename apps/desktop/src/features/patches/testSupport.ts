import type { ProjectSummaryDto, WritePlanDto } from 'rimstudio-ipc-types';
import { createMockTransport, loadFixture, type MockHandler } from 'rimstudio-testkit';
import { clearQueries, connection, setTransport } from '~/shared/ipc';
import { resetProjectStore, setCurrentProject } from '~/shared/project';
import { resetAnswers } from './answerStore';
import { resetLint } from './lintStore';
import { resetPlans } from './planStore';
import { resetScan, includeConverted } from './scanStore';
import { resetChoices } from './suggestStore';
import { resetApply } from './applyStore';

/** Handlers answering the commands of the page from fixtures recorded on a real install. */
export function patchesHandlers(): Record<string, MockHandler> {
  return {
    project_open: () => loadFixture('patches-project-plain'),
    designer_convert_scan: () => loadFixture('designer_convert_scan'),
    designer_export_plan: (request) => {
      const convert = (request as { convert?: { answers?: { ammoSet?: string } } }).convert;
      return convert?.answers?.ammoSet
        ? loadFixture<WritePlanDto>('patches-plan-ready')
        : loadFixture<WritePlanDto>('patches-plan-open');
    },
    designer_ce_suggest: () => loadFixture('patches-suggest-ranged'),
    designer_apply_plan: () => loadFixture('patches-apply-report'),
    project_read_file: () => ({
      path: 'Defs/ThingDefs_Misc/Weapons/RangedIndustrial.xml',
      role: 'defs-weapons',
      bytes: 40,
      text: '<Defs>\n  <ThingDef><defName>OH_G41m</defName></ThingDef>\n</Defs>\n',
      truncated: false,
      binary: false,
    }),
  };
}

/** Install a mock transport and a clean state; extra handlers win over the fixture ones. */
export function installTransport(extra: Record<string, MockHandler> = {}) {
  clearQueries();
  resetPage();
  resetProjectStore();
  const transport = createMockTransport({ handlers: { ...patchesHandlers(), ...extra } });
  setTransport(transport);
  connection.value = 'mock';
  return transport;
}

/** Make the Gewehr 41 copy of the fixtures the current project. */
export function openFixtureProject(fixture = 'patches-project-plain'): ProjectSummaryDto {
  const summary = loadFixture<ProjectSummaryDto>(fixture);
  setCurrentProject({
    projectId: summary.projectId,
    path: summary.path,
    name: summary.name,
    ...(summary.packageId ? { packageId: summary.packageId } : {}),
  });
  return summary;
}

/** Forget every store of the page. */
export function resetPage(): void {
  resetScan();
  resetAnswers();
  resetPlans();
  resetChoices();
  resetLint();
  resetApply();
  includeConverted.value = true;
}
