import { signal } from '@preact/signals';
import type { ApiError, DetectionReportDto, PathFieldDto } from 'rimstudio-ipc-types';
import { normalizeError } from '~/shared/ipc';
import { loadSources } from './sourcesStore';
import { getDetectionReport, runDetection, setPathOverride } from './api';

/** The last detection report; undefined on a first run before detection has run. */
export const report = signal<DetectionReportDto | undefined>(undefined);
/** True once the cached report was asked for, so the page can tell "first run" from "loading". */
export const reportLoaded = signal(false);
export const detecting = signal(false);
export const detectError = signal<ApiError | undefined>(undefined);

/** Ask for the cached report. */
export async function loadReport(): Promise<void> {
  try {
    report.value = await getDetectionReport();
    detectError.value = undefined;
  } catch (thrown) {
    detectError.value = normalizeError(thrown);
  } finally {
    reportLoaded.value = true;
  }
}

/** Run detection (the "Detect again" button, and the first run). */
export async function detectAgain(): Promise<void> {
  detecting.value = true;
  try {
    report.value = await runDetection(true);
    detectError.value = undefined;
    await loadSources();
  } catch (thrown) {
    detectError.value = normalizeError(thrown);
  } finally {
    detecting.value = false;
    reportLoaded.value = true;
  }
}

/** Set (or clear with undefined) the override of one path; the report is replaced. */
export async function overridePath(field: PathFieldDto, path: string | undefined): Promise<void> {
  try {
    const next = await setPathOverride(field, path);
    if (next) report.value = next;
    detectError.value = undefined;
    await loadSources();
  } catch (thrown) {
    detectError.value = normalizeError(thrown);
  }
}

/** Forget everything (tests, session change). */
export function resetDetect(): void {
  report.value = undefined;
  reportLoaded.value = false;
  detecting.value = false;
  detectError.value = undefined;
}
