import type {
  ProjectLinkModeDto,
  ProjectLinkResultDto,
  ProjectLinkStatusDto,
} from 'rimstudio-ipc-types';
import { callCommand } from '~/shared/ipc';

// One function per command; no state, no rules. The link store calls these.

/** Whether the game can see the project and what is in the way. Read only. */
export function readLinkStatus(projectId: string): Promise<ProjectLinkStatusDto> {
  return callCommand('project_link_status', { projectId });
}

/** Link (or, on request, copy) the project into the game's Mods folder. */
export function createLink(
  projectId: string,
  mode: ProjectLinkModeDto,
  confirmGameRunning: boolean,
): Promise<ProjectLinkResultDto> {
  return callCommand('project_link_create', { projectId, mode, confirmGameRunning });
}

/** Remove the link or copy RimStudio made for the project. */
export function removeLink(projectId: string): Promise<ProjectLinkResultDto> {
  return callCommand('project_link_remove', { projectId });
}
