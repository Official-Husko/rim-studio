import { signal } from '@preact/signals';

/** A folder or file picker waiting for the user, shown by FolderPickerHost in browser mode. */
export interface PickerRequest {
  id: number;
  mode: 'folder' | 'file';
  start?: string;
  resolve: (path: string | null) => void;
}

export const pickerRequest = signal<PickerRequest | null>(null);
let nextId = 0;

/** True inside the Tauri webview. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function open(mode: PickerRequest['mode'], start?: string): Promise<string | null> {
  if (isTauri()) {
    // The native dialog arrives with the shell crate; until then fail loudly.
    return Promise.reject({
      code: 'platform.unavailable',
      message: 'The native dialog is not available yet.',
      errorId: 'c-platform',
    });
  }
  return new Promise((resolve) => {
    // a second request cancels the first so a promise is never left hanging
    pickerRequest.peek()?.resolve(null);
    nextId += 1;
    pickerRequest.value = { id: nextId, mode, start, resolve };
  });
}

/** Ask the user for a folder. Resolves with the absolute path, or null when cancelled. */
export function pickFolder(options: { start?: string } = {}): Promise<string | null> {
  return open('folder', options.start);
}

/** Ask the user for a file. Resolves with the absolute path, or null when cancelled. */
export function pickFile(options: { start?: string } = {}): Promise<string | null> {
  return open('file', options.start);
}

/** Open an https link in the system browser. Other schemes are refused. */
export function openUrl(url: string): boolean {
  if (!/^https:\/\//i.test(url)) return false;
  window.open(url, '_blank', 'noopener,noreferrer');
  return true;
}
