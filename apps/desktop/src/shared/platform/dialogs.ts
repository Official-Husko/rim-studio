import { signal } from '@preact/signals';

/** A file type group of a file dialog: a label and the extensions without the dot. */
export interface FileFilter {
  name: string;
  extensions: string[];
}

/** A folder or file picker waiting for the user, shown by FolderPickerHost in browser mode. */
export interface PickerRequest {
  id: number;
  mode: 'folder' | 'file';
  start?: string;
  filters?: FileFilter[];
  resolve: (path: string | null) => void;
}

export const pickerRequest = signal<PickerRequest | null>(null);
let nextId = 0;

/**
 * True inside the Tauri webview: the native dialogs, the opener and the in process backend are
 * available. This is the capability probe the rest of the app uses (never `window.__TAURI_*`).
 */
export function isDesktop(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** The same probe under its first name. */
export const isTauri = isDesktop;

/** What a native dialog or opener call failed with, as an ApiError shaped object. */
function nativeFailure(thrown: unknown): never {
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  throw {
    code: 'platform.native-failed',
    message,
    errorId: `c-platform-${Date.now().toString(36)}`,
  };
}

async function nativePick(
  mode: PickerRequest['mode'],
  start?: string,
  filters?: FileFilter[],
): Promise<string | null> {
  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const chosen = await open({
      directory: mode === 'folder',
      multiple: false,
      defaultPath: start,
      ...(mode === 'file' && filters && filters.length > 0 ? { filters } : {}),
    });
    return typeof chosen === 'string' ? chosen : null;
  } catch (thrown) {
    return nativeFailure(thrown);
  }
}

function browserPick(
  mode: PickerRequest['mode'],
  start?: string,
  filters?: FileFilter[],
): Promise<string | null> {
  return new Promise((resolve) => {
    // a second request cancels the first so a promise is never left hanging
    pickerRequest.peek()?.resolve(null);
    nextId += 1;
    pickerRequest.value = { id: nextId, mode, start, filters, resolve };
  });
}

function pick(
  mode: PickerRequest['mode'],
  start?: string,
  filters?: FileFilter[],
): Promise<string | null> {
  return isDesktop() ? nativePick(mode, start, filters) : browserPick(mode, start, filters);
}

/**
 * Ask the user for a folder. The desktop shell opens the native dialog; a browser opens the folder
 * browser of the development bridge. Resolves with the absolute path, or null when cancelled.
 */
export function pickFolder(options: { start?: string } = {}): Promise<string | null> {
  return pick('folder', options.start);
}

/** Ask the user for a file, optionally limited to file types. Resolves with the path or null. */
export function pickFile(
  options: { start?: string; filters?: FileFilter[] } = {},
): Promise<string | null> {
  return pick('file', options.start, options.filters);
}

/**
 * Show a file or folder in the system file manager. Resolves true when it was shown; false in a
 * browser, where there is no file manager to open.
 */
export async function revealPath(path: string): Promise<boolean> {
  if (!isDesktop()) return false;
  try {
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
    await revealItemInDir(path);
    return true;
  } catch (thrown) {
    return nativeFailure(thrown);
  }
}

/** Open an https link in the system browser. Other schemes are refused. */
export function openUrl(url: string): boolean {
  if (!/^https:\/\//i.test(url)) return false;
  if (isDesktop()) {
    // the webview itself never navigates or opens windows; the opener plugin does it
    void import('@tauri-apps/plugin-opener').then(
      ({ openUrl: openLink }) => openLink(url),
      () => undefined,
    );
  } else {
    window.open(url, '_blank', 'noopener,noreferrer');
  }
  return true;
}
