import { callCommand } from '~/shared/ipc';
import type {
  DetectionReportDto,
  PathFieldDto,
  SettingsDto,
  SourceDto,
  SourcesProbeFolderResponse,
  FolderLayoutDto,
  LibraryScanResult,
  SourcesUpdateRequest,
} from 'rimstudio-ipc-types';

// One function per command; no state, no rules. The stores call these.

/** The last detection report, or undefined when detection never ran. */
export async function getDetectionReport(): Promise<DetectionReportDto | undefined> {
  return (await callCommand('detect_get_report')).report;
}

/** Run detection again. A job: it shows in the task centre. */
export function runDetection(force: boolean): Promise<DetectionReportDto> {
  return callCommand('detect_run', { force });
}

/** Set or clear (path undefined) one override; returns the report with the override applied. */
export async function setPathOverride(
  field: PathFieldDto,
  path: string | undefined,
): Promise<DetectionReportDto | undefined> {
  const request = path === undefined ? { field } : { field, path };
  return (await callCommand('detect_set_override', request)).report;
}

/** Every mod source in scan order. */
export async function listSources(): Promise<SourceDto[]> {
  return (await callCommand('sources_list')).sources;
}

/** Look at a folder before adding it. */
export function probeFolder(path: string): Promise<SourcesProbeFolderResponse> {
  return callCommand('sources_probe_folder', { path });
}

/** Add a custom folder with the probe suggestions for layout and depth. */
export function addFolder(input: {
  path: string;
  label?: string;
  layout?: FolderLayoutDto;
  scanDepth?: number;
}): Promise<SourceDto> {
  return callCommand('sources_add_folder', input);
}

/** Change a source; absent members stay as they are. */
export function updateSource(request: SourcesUpdateRequest): Promise<SourceDto> {
  return callCommand('sources_update', request);
}

/** Forget a source. Files are never touched. */
export async function removeSource(id: string): Promise<boolean> {
  return (await callCommand('sources_remove', { id })).removed;
}

/** Scan the library. A job: progress shows in the task centre. */
export function scanLibrary(full: boolean): Promise<LibraryScanResult> {
  return callCommand('library_scan', { full });
}

/** The settings, all sections. */
export function getSettings(): Promise<SettingsDto> {
  return callCommand('settings_get', { sections: [] });
}
