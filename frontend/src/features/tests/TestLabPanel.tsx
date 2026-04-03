import { h } from 'preact';
import { useState } from 'preact/hooks';
import { Button } from '../../components/ui/Button';
import type { SimulatedTaskPlan } from './taskSimulator';
import { buildSimulatedTaskPlans } from './taskSimulator';
import './test-lab.css';

interface Props {
  activeDemoTaskCount: number;
  totalActiveTaskCount: number;
  onRunSimulatedTasks: (plans: SimulatedTaskPlan[]) => void;
  onClearSimulatedTasks: () => void;
}

export function TestLabPanel({
  activeDemoTaskCount,
  totalActiveTaskCount,
  onRunSimulatedTasks,
  onClearSimulatedTasks,
}: Props) {
  const [taskCount, setTaskCount] = useState(4);
  const [lastBatch, setLastBatch] = useState<SimulatedTaskPlan[]>([]);

  function runBatch(count: number) {
    const plans = buildSimulatedTaskPlans(count);
    setTaskCount(count);
    setLastBatch(plans);
    onRunSimulatedTasks(plans);
  }

  return (
    <div className="content-stack">
      <section className="panel-card test-lab-panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Manual UI Tests</p>
            <h3>Background Task Playground</h3>
          </div>
          <div className="test-lab-actions">
            <Button onClick={() => runBatch(taskCount)} variant="primary">
              Start Batch
            </Button>
            <Button disabled={activeDemoTaskCount === 0} onClick={onClearSimulatedTasks} variant="ghost">
              Clear Simulated Tasks
            </Button>
          </div>
        </div>

        <div className="test-lab-grid">
          <label>
            <span>Artificial Task Count</span>
            <input
              max={12}
              min={1}
              type="number"
              value={taskCount}
              onInput={(event) => setTaskCount(Math.max(1, Math.min(12, Number(event.currentTarget.value) || 1)))}
            />
          </label>

          <div className="test-lab-presets">
            {[1, 3, 5, 8].map((count) => (
              <Button key={count} onClick={() => runBatch(count)} variant="secondary">
                Run {count}
              </Button>
            ))}
          </div>
        </div>

        <div className="test-lab-summary">
          <div className="stat-card">
            <strong>{activeDemoTaskCount}</strong>
            <span>Simulated tasks currently active</span>
          </div>
          <div className="stat-card">
            <strong>{totalActiveTaskCount}</strong>
            <span>Total background tasks visible in the bottom bar</span>
          </div>
          <div className="stat-card">
            <strong>{lastBatch.length}</strong>
            <span>Tasks in the last generated batch</span>
          </div>
        </div>
      </section>

      <section className="panel-card test-lab-panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Generated Batch</p>
            <h3>Latest Simulated Tasks</h3>
          </div>
        </div>

        {lastBatch.length === 0 ? (
          <div className="empty-card">
            <h4>No batch generated yet</h4>
            <p>
              Start a batch to create artificial background tasks with random durations and unique activity labels for
              the current run.
            </p>
          </div>
        ) : (
          <div className="test-lab-list">
            {lastBatch.map((task) => (
              <article className="test-lab-item" key={task.id}>
                <div>
                  <strong>{task.label}</strong>
                  <span>{task.id}</span>
                </div>
                <div className="test-lab-metrics">
                  <span>{(task.durationMs / 1000).toFixed(1)}s</span>
                  <span>{task.tickMs}ms tick</span>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
