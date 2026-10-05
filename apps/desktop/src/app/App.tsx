import { useEffect } from 'preact/hooks';
import { ProjectSelector } from '~/features/project';
import { AppShell } from './AppShell';
import { ErrorBoundary } from './ErrorBoundary';
import { startRouter } from './route';

/** The root component: router, the app level error boundary and the shell. */
export function App() {
  useEffect(() => startRouter(), []);
  return (
    <ErrorBoundary region="RimStudio">
      <AppShell projectSlot={<ProjectSelector />} />
    </ErrorBoundary>
  );
}
