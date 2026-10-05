// Public entry of shared/platform: the only module allowed to import @tauri-apps/* once the
// shell exists. In a plain browser it offers the web equivalents.
export { isTauri, openUrl, pickFile, pickFolder } from './dialogs';
export { FolderPickerHost } from './FolderPickerHost';
