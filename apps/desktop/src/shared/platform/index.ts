// Public entry of shared/platform: the only module that talks to the native dialogs and the opener
// (the Tauri plugins); the Tauri transport of shared/ipc is the one other module that imports
// the Tauri core API. In a plain browser the pickers are the folder browser of the development bridge.
export {
  isDesktop,
  isTauri,
  openUrl,
  pickFile,
  pickFolder,
  revealPath,
  type FileFilter,
} from './dialogs';
export { FolderPickerHost } from './FolderPickerHost';
