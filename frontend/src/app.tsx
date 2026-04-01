import './App.css';
import { h } from 'preact';
import { LoadingScreen } from './components/LoadingScreen';
import { NewProjectDialog } from './components/NewProjectDialog';
import { StartScreen } from './components/StartScreen';
import { NotificationCenter } from './components/notifications/NotificationCenter';
import { WorkspaceShell } from './components/workspace/WorkspaceShell';
import { useNotifications } from './hooks/useNotifications';
import { useStudioApp } from './hooks/useStudioApp';

export function App() {
  const notifications = useNotifications();
  const studio = useStudioApp(notifications);
  const { state, actions } = studio;

  if (state.loading) {
    return <LoadingScreen />;
  }

  return (
    <div className="app-shell">
      <div className="app-backdrop" />
      <NotificationCenter notifications={notifications.notifications} onDismiss={notifications.dismissNotification} />

      {state.projectState ? (
        <WorkspaceShell
          activeTab={state.activeTab}
          availableMods={state.bootstrap.availableMods}
          busy={state.busy}
          globalDraft={state.globalDraft}
          projectDraft={state.projectDraft}
          projectState={state.projectState}
          scanStatus={state.bootstrap.scanStatus}
          selectedMods={state.selectedMods}
          onBrowseGamePath={actions.browseGamePath}
          onCloseProject={actions.closeProjectWorkspace}
          onGlobalGamePathChange={actions.updateGlobalGamePath}
          onGlobalScanModsEnabledChange={actions.updateGlobalScanModsEnabled}
          onProjectSettingsChange={actions.updateProjectSettings}
          onSaveGlobalSettings={actions.saveGlobalSettings}
          onSaveProjectSettings={actions.saveProjectSettings}
          onTabChange={actions.setActiveTab}
          onToggleSelectedMod={actions.toggleSelectedMod}
          onTriggerRescan={actions.triggerRescan}
        />
      ) : (
        <StartScreen
          busy={state.busy}
          recentProjects={state.bootstrap.recentProjects}
          onLoadProject={() => actions.openProject()}
          onNewProject={() => actions.setShowNewProject(true)}
          onOpenRecentProject={actions.openProject}
        />
      )}

      <NewProjectDialog
        busy={state.busy}
        open={state.showNewProject}
        project={state.newProject}
        onBrowseLocation={actions.browseNewProjectLocation}
        onChange={actions.updateNewProjectField}
        onClose={() => actions.setShowNewProject(false)}
        onSubmit={actions.submitNewProject}
      />
    </div>
  );
}
