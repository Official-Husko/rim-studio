import { EventsOn } from '../../wailsjs/runtime/runtime';
import { useEffect, useMemo, useState } from 'preact/hooks';
import { api } from '../api';
import { emptyBootstrap, emptyProjectSettings } from '../constants';
import type {
  AppBootstrap,
  CreateProjectInput,
  GameScanSnapshot,
  GlobalSettings,
  NotificationItem,
  ProjectSettings,
  ProjectState,
  ScannedModSummary,
  ThemeID,
  WorkspaceTab,
} from '../types';
import { buildPatchPlaceholder, getErrorMessage } from '../utils/ui';

interface NotificationTools {
  notify: (notification: Omit<NotificationItem, 'id'>) => void;
  notifyError: (title: string, message: string) => void;
  removeNotificationByKey: (key: string) => void;
  upsertNotification: (notification: Omit<NotificationItem, 'id'> & { key: string }) => void;
}

export function useStudioApp({ notify, notifyError, removeNotificationByKey, upsertNotification }: NotificationTools) {
  const [bootstrap, setBootstrap] = useState<AppBootstrap>(emptyBootstrap);
  const [projectState, setProjectState] = useState<ProjectState | null>(null);
  const [projectDraft, setProjectDraft] = useState<ProjectSettings>(emptyProjectSettings());
  const [activeTab, setActiveTab] = useState<WorkspaceTab>('basics');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [currentTaskLabel, setCurrentTaskLabel] = useState('');
  const [showNewProject, setShowNewProject] = useState(false);
  const [newProject, setNewProject] = useState<CreateProjectInput>({
    name: '',
    packageId: '',
    author: '',
    location: '',
    targetVersion: '1.6',
  });
  const [globalDraft, setGlobalDraft] = useState<GlobalSettings>(emptyBootstrap.settings);

  useEffect(() => {
    void loadBootstrap();
  }, []);

  useEffect(() => {
    if (!bootstrap.settings.gamePath) {
      upsertNotification({
        key: 'system:rimworld-path',
        type: 'error',
        title: 'RimWorld path not configured',
        message: 'Set a RimWorld install path to index game data.',
        icon: 'fa-triangle-exclamation',
        persistent: true,
        actions: [
          {
            label: 'Set Path',
            onClick: quickSetGamePath,
          },
        ],
      });
      return;
    }

    if (bootstrap.scanStatus.state === 'error') {
      upsertNotification({
        key: 'system:rimworld-path',
        type: 'error',
        title: 'Game data scan failed',
        message: bootstrap.scanStatus.message,
        icon: 'fa-octagon-exclamation',
        persistent: true,
        actions: [
          {
            label: 'Set Path',
            onClick: quickSetGamePath,
          },
        ],
      });
      return;
    }

    removeNotificationByKey('system:rimworld-path');
  }, [bootstrap.settings.gamePath, bootstrap.scanStatus.message, bootstrap.scanStatus.state]);

  useEffect(() => {
    const unsubscribe = EventsOn('rimstudio:scan-status', (...eventData: unknown[]) => {
      const payload = eventData[0] as GameScanSnapshot | undefined;
      if (!payload) {
        return;
      }

      setBootstrap((current) => ({
        ...current,
        scanStatus: payload.scanStatus,
        availableMods: payload.availableMods,
        settings: {
          ...current.settings,
          cachedModIndex: payload.availableMods,
        },
      }));
    });

    return () => {
      if (typeof unsubscribe === 'function') {
        unsubscribe();
      }
    };
  }, []);

  useEffect(() => {
    setGlobalDraft(bootstrap.settings);
  }, [bootstrap]);

  useEffect(() => {
    if (projectState) {
      setProjectDraft(projectState.settings);
    }
  }, [projectState]);

  const selectedMods = useMemo(() => {
    if (!projectState) {
      return [];
    }

    const selectedIds = new Set(projectDraft.compatibility.selectedModIds);
    return bootstrap.availableMods.filter((mod) => selectedIds.has(mod.id));
  }, [bootstrap.availableMods, projectDraft.compatibility.selectedModIds, projectState]);

  const activeTaskLabel = useMemo(() => {
    if (currentTaskLabel) {
      return currentTaskLabel;
    }
    if (bootstrap.scanStatus.state === 'scanning') {
      return 'Indexing game data';
    }
    return '';
  }, [bootstrap.scanStatus.state, currentTaskLabel]);

  async function runBusyTask(label: string, action: () => Promise<void>) {
    setBusy(true);
    setCurrentTaskLabel(label);
    try {
      await action();
    } finally {
      setBusy(false);
      setCurrentTaskLabel('');
    }
  }

  async function loadBootstrap() {
    setLoading(true);

    try {
      const result = await api.getAppBootstrap();
      setBootstrap(result);

      if (result.currentProject?.path) {
        const current = await api.getProjectState(result.currentProject.path);
        setProjectState(current);
      }
    } catch (error) {
      notifyError('Failed to load RimStudio', getErrorMessage(error));
    } finally {
      setLoading(false);
    }
  }

  async function openProject(projectPath?: string) {
    try {
      await runBusyTask('Opening project', async () => {
        const targetPath = projectPath ?? await api.chooseDirectory('Open RimWorld Mod Folder');
        if (!targetPath) {
          return;
        }

        const result = await api.openProject(targetPath);
        setProjectState(result);
        setProjectDraft(result.settings);
        setActiveTab('basics');
        notify({
          type: 'tip',
          title: 'Project opened',
          message: result.summary.name,
          icon: 'fa-folder-open',
        });

        const freshBootstrap = await api.getAppBootstrap();
        setBootstrap(freshBootstrap);
      });
    } catch (error) {
      notifyError('Open project failed', getErrorMessage(error));
    }
  }

  async function browseNewProjectLocation() {
    try {
      const path = await api.chooseDirectory('Choose Parent Directory For New Mod', newProject.location);
      if (!path) {
        return;
      }

      setNewProject((current) => ({ ...current, location: path }));
    } catch (error) {
      notifyError('Folder selection failed', getErrorMessage(error));
    }
  }

  async function submitNewProject(event: Event) {
    event.preventDefault();

    try {
      await runBusyTask('Creating project', async () => {
        const result = await api.createProject(newProject);
        setProjectState(result);
        setProjectDraft(result.settings);
        setShowNewProject(false);
        setActiveTab('basics');
        notify({
          type: 'tip',
          title: 'Project created',
          message: result.summary.name,
          icon: 'fa-hammer',
        });
        setNewProject({
          name: '',
          packageId: '',
          author: '',
          location: '',
          targetVersion: '1.6',
        });

        const freshBootstrap = await api.getAppBootstrap();
        setBootstrap(freshBootstrap);
      });
    } catch (error) {
      notifyError('Create project failed', getErrorMessage(error));
    }
  }

  async function closeProjectWorkspace() {
    try {
      await runBusyTask('Closing workspace', async () => {
        const result = await api.closeProject();
        setBootstrap(result);
        setProjectState(null);
        setProjectDraft(emptyProjectSettings());
        setActiveTab('basics');
        notify({
          type: 'info',
          title: 'Workspace closed',
          message: 'Returned to the start screen.',
          icon: 'fa-house',
        });
      });
    } catch (error) {
      notifyError('Close workspace failed', getErrorMessage(error));
    }
  }

  async function browseGamePath() {
    try {
      const path = await api.chooseDirectory('Choose RimWorld Install Folder', globalDraft.gamePath);
      if (!path) {
        return;
      }

      setGlobalDraft((current) => ({ ...current, gamePath: path }));
    } catch (error) {
      notifyError('Folder selection failed', getErrorMessage(error));
    }
  }

  async function quickSetGamePath() {
    try {
      await runBusyTask('Updating game path', async () => {
        const path = await api.chooseDirectory('Choose RimWorld Install Folder', bootstrap.settings.gamePath);
        if (!path) {
          return;
        }

        const result = await api.updateGlobalSettings({
          gamePath: path,
          scanModsEnabled: bootstrap.settings.scanModsEnabled,
          themeId: globalDraft.themeId,
          customCssPath: globalDraft.customCssPath,
        });

        setBootstrap(result);
        notify({
          type: 'tip',
          title: 'RimWorld path updated',
          message: path,
          icon: 'fa-check',
        });
      });
    } catch (error) {
      notifyError('Set path failed', getErrorMessage(error));
    }
  }

  async function saveGlobalSettings() {
    try {
      await runBusyTask('Saving settings', async () => {
        const result = await api.updateGlobalSettings({
          gamePath: globalDraft.gamePath,
          scanModsEnabled: globalDraft.scanModsEnabled,
          themeId: globalDraft.themeId,
          customCssPath: globalDraft.customCssPath,
        });

        setBootstrap(result);
        notify({
          type: 'tip',
          title: 'Settings saved',
          message: 'Global settings updated successfully.',
          icon: 'fa-sliders',
        });
      });
    } catch (error) {
      notifyError('Save settings failed', getErrorMessage(error));
    }
  }

  async function triggerRescan() {
    try {
      await runBusyTask('Indexing game data', async () => {
        const result = await api.rescanGameData();
        setBootstrap((current) => ({
          ...current,
          scanStatus: result.scanStatus,
          availableMods: result.availableMods,
        }));
        notify({
          type: 'info',
          title: 'Rescan started',
          message: 'RimWorld data is being indexed again.',
          icon: 'fa-rotate-right',
        });
      });
    } catch (error) {
      notifyError('Rescan failed', getErrorMessage(error));
    }
  }

  async function saveProjectSettings() {
    if (!projectState) {
      return;
    }

    try {
      await runBusyTask('Saving project', async () => {
        const result = await api.updateProjectSettings({
          projectPath: projectState.summary.path,
          settings: projectDraft,
        });

        setProjectState(result);
        setProjectDraft(result.settings);
        notify({
          type: 'tip',
          title: 'Project settings saved',
          message: 'Compatibility settings were updated.',
          icon: 'fa-floppy-disk',
        });
      });
    } catch (error) {
      notifyError('Save project settings failed', getErrorMessage(error));
    }
  }

  function updateProjectSettings(settings: ProjectSettings) {
    setProjectDraft(settings);
  }

  function updateNewProjectField(field: keyof CreateProjectInput, value: string) {
    setNewProject((current) => ({
      ...current,
      [field]: value,
    }));
  }

  function updateGlobalGamePath(value: string) {
    setGlobalDraft((current) => ({
      ...current,
      gamePath: value,
    }));
  }

  function updateGlobalScanModsEnabled(enabled: boolean) {
    setGlobalDraft((current) => ({
      ...current,
      scanModsEnabled: enabled,
    }));
  }

  function updateGlobalThemeID(themeId: ThemeID) {
    setGlobalDraft((current) => ({
      ...current,
      themeId,
    }));
  }

  function updateGlobalCustomCSSPath(value: string) {
    setGlobalDraft((current) => ({
      ...current,
      customCssPath: value,
    }));
  }

  async function browseCustomCSSPath() {
    try {
      const path = await api.chooseCSSFile('Choose Custom Theme CSS', globalDraft.customCssPath);
      if (!path) {
        return;
      }

      updateGlobalCustomCSSPath(path);
    } catch (error) {
      notifyError('CSS file selection failed', getErrorMessage(error));
    }
  }

  function clearCustomCSSPath() {
    updateGlobalCustomCSSPath('');
  }

  function toggleSelectedMod(mod: ScannedModSummary) {
    const selected = new Set(projectDraft.compatibility.selectedModIds);
    const patches = { ...projectDraft.compatibility.patchEntries };

    if (selected.has(mod.id)) {
      selected.delete(mod.id);
      delete patches[mod.id];
    } else {
      selected.add(mod.id);
      patches[mod.id] = patches[mod.id] ?? buildPatchPlaceholder(mod);
    }

    setProjectDraft((current) => ({
      ...current,
      compatibility: {
        ...current.compatibility,
        selectedModIds: Array.from(selected),
        patchEntries: patches,
      },
    }));
  }

  return {
    state: {
      bootstrap,
      projectState,
      projectDraft,
      activeTab,
      loading,
      busy,
      showNewProject,
      newProject,
      globalDraft,
      selectedMods,
      activeTaskLabel,
    },
    actions: {
      setActiveTab,
      setShowNewProject,
      browseNewProjectLocation,
      closeProjectWorkspace,
      openProject,
      saveGlobalSettings,
      saveProjectSettings,
      submitNewProject,
      toggleSelectedMod,
      triggerRescan,
      browseGamePath,
      browseCustomCSSPath,
      clearCustomCSSPath,
      updateGlobalGamePath,
      updateGlobalThemeID,
      updateGlobalCustomCSSPath,
      updateGlobalScanModsEnabled,
      updateNewProjectField,
      updateProjectSettings,
    },
  };
}
