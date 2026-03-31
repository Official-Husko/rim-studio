import type {
  CompatibilityPatchEntry,
  ScannedModSummary,
} from '../types';

export function buildPatchPlaceholder(mod: ScannedModSummary): CompatibilityPatchEntry {
  return {
    modId: mod.id,
    displayName: mod.name,
    notes: '',
    generated: false,
    userEdited: false,
  };
}

export function getInputValue(event: Event): string {
  return (event.currentTarget as HTMLInputElement | HTMLSelectElement).value;
}

export function getErrorMessage(error: unknown): string {
  if (typeof error === 'string') {
    return error;
  }
  if (error && typeof error === 'object' && 'message' in error && typeof error.message === 'string') {
    return error.message;
  }
  return 'Unexpected error while processing the request.';
}
