import { useEffect, useState } from 'preact/hooks';
import { routeParams } from '~/app/route';
import { applyBridgeEvent } from '~/shared/ipc';
import { pickFolder } from '~/shared/platform';
import { Banner, Button, Dialog, EmptyState, ProgressBar, Spinner, Toast } from 'rimstudio-ui';
import { Demo, Section } from '../Section';

function simulateJobs(): void {
  applyBridgeEvent({
    type: 'job-progress',
    jobId: 'sim-1',
    command: 'library_scan',
    message: 'Reading Core',
    done: 142,
    total: 340,
  });
  applyBridgeEvent({
    type: 'job-progress',
    jobId: 'sim-2',
    command: 'designer_apply_plan',
    message: 'Writing Defs/Gun.xml',
    done: 1,
    total: 3,
  });
  applyBridgeEvent({
    type: 'job-finished',
    jobId: 'sim-3',
    command: 'sources_probe_folder',
    ok: true,
  });
  applyBridgeEvent({
    type: 'job-finished',
    jobId: 'sim-4',
    command: 'designer_export_plan',
    ok: false,
  });
}

/** Banners, progress, empty state, toasts and the dialog. */
export function FeedbackSection() {
  const [open, setOpen] = useState(() => routeParams.value.get('dialog') === 'open');
  const [picked, setPicked] = useState<string | null>(null);
  useEffect(() => {
    if (routeParams.value.get('picker') === 'open') void pickFolder().then(setPicked);
    if (routeParams.value.get('jobs') === 'sim') simulateJobs();
  }, []);
  return (
    <Section id="feedback" title="Feedback">
      <div class="flex max-w-3xl flex-col gap-2">
        <Banner tone="info" title="Datasets">
          Using the last copy from 2026-10-01.
        </Banner>
        <Banner tone="success">Three files written to the project.</Banner>
        <Banner tone="warning" title="Out of date" action={<Button size="sm">Regenerate</Button>}>
          The def changed after the patch was written.
        </Banner>
        <Banner tone="error" title="Write blocked" dismissLabel="Dismiss" onDismiss={() => {}}>
          The project folder is read only.
        </Banner>
      </div>
      <div class="flex max-w-xl flex-col gap-5">
        <ProgressBar label="Library scan" value={0.42} caption="142 of 340 mods" />
        <ProgressBar label="Building defs" caption="Reading Core" />
        <ProgressBar label="Failed step" value={0.7} tone="danger" />
        <ProgressBar label="Warning step" value={0.3} tone="warning" />
      </div>
      <Demo label="Spinner">
        <Spinner size="sm" label="Small" />
        <Spinner size="md" label="Medium" />
      </Demo>
      <div class="max-w-2xl border border-line">
        <EmptyState
          icon="crosshair"
          title="No weapons yet"
          description="Start from a clone of a vanilla weapon, a new weapon, or convert an existing mod."
          action={
            <Button variant="primary" icon="plus">
              New weapon
            </Button>
          }
        />
      </div>
      <div class="max-w-md border border-line">
        <EmptyState compact title="No results" description="Try a shorter search." />
      </div>
      <Demo label="Toasts (static)">
        <Toast
          tone="success"
          title="Written"
          message="3 files created."
          actionLabel="Show"
          onAction={() => {}}
          onDismiss={() => {}}
        />
        <Toast tone="error" message="Could not read the About file." onDismiss={() => {}} />
      </Demo>
      <Demo label="Task centre and folder browser">
        <Button onClick={simulateJobs}>Simulate jobs</Button>
        <Button icon="folder" onClick={() => void pickFolder().then(setPicked)}>
          Open folder browser
        </Button>
        {picked ? <span class="font-mono text-mono text-muted">{picked}</span> : null}
      </Demo>
      <Demo label="Dialog">
        <Button onClick={() => setOpen(true)}>Open dialog</Button>
        <Dialog
          open={open}
          title="Overwrite 2 files?"
          onClose={() => setOpen(false)}
          footer={
            <>
              <Button onClick={() => setOpen(false)}>Cancel</Button>
              <Button variant="danger" onClick={() => setOpen(false)}>
                Overwrite
              </Button>
            </>
          }
        >
          <p>Backups are written next to the files before they change.</p>
        </Dialog>
      </Demo>
    </Section>
  );
}
