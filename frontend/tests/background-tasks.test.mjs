import test from 'node:test';
import assert from 'node:assert/strict';
import {
  appendBackgroundTask,
  normalizeBackgroundTaskProgress,
  removeBackgroundTask,
  summarizeBackgroundTasks,
  updateBackgroundTaskList,
  upsertBackgroundTask,
} from '../.test-dist/src/utils/backgroundTasks.js';

test('normalizeBackgroundTaskProgress clamps and rounds values', () => {
  assert.equal(normalizeBackgroundTaskProgress(undefined), undefined);
  assert.equal(normalizeBackgroundTaskProgress(Number.NaN), undefined);
  assert.equal(normalizeBackgroundTaskProgress(-4), 0);
  assert.equal(normalizeBackgroundTaskProgress(47.7), 48);
  assert.equal(normalizeBackgroundTaskProgress(140), 100);
});

test('appendBackgroundTask preserves FIFO order and ignores duplicate ids', () => {
  const tasks = [];
  const withFirst = appendBackgroundTask(tasks, { id: 'first', label: 'Saving project' }, 100);
  const withSecond = appendBackgroundTask(withFirst, { id: 'second', label: 'Indexing game data' }, 200);
  const withDuplicate = appendBackgroundTask(withSecond, { id: 'first', label: 'Duplicate' }, 300);

  assert.deepEqual(withSecond.map((task) => task.id), ['first', 'second']);
  assert.equal(withDuplicate, withSecond);
});

test('upsertBackgroundTask updates an existing task without reordering it', () => {
  const initial = [
    { id: 'first', label: 'Saving project', startedAt: 100 },
    { id: 'second', label: 'Indexing game data', startedAt: 200 },
  ];

  const updated = upsertBackgroundTask(initial, { id: 'first', label: 'Saving project', progress: 62 }, 300);

  assert.deepEqual(updated.map((task) => task.id), ['first', 'second']);
  assert.equal(updated[0].progress, 62);
  assert.equal(updated[0].startedAt, 100);
});

test('removeBackgroundTask drops only the requested task', () => {
  const initial = [
    { id: 'first', label: 'Saving project', startedAt: 100 },
    { id: 'second', label: 'Indexing game data', startedAt: 200 },
  ];

  const next = removeBackgroundTask(initial, 'first');
  assert.deepEqual(next.map((task) => task.id), ['second']);
});

test('summarizeBackgroundTasks returns the first task as current and counts extras', () => {
  const tasks = [
    { id: 'first', label: 'Saving project', progress: 28, startedAt: 100 },
    { id: 'second', label: 'Indexing game data', progress: 64, startedAt: 200 },
    { id: 'third', label: 'Generating patches', startedAt: 300 },
  ];

  const summary = summarizeBackgroundTasks(tasks);

  assert.equal(summary.displayLabel, 'Saving project');
  assert.equal(summary.currentTask?.id, 'first');
  assert.equal(summary.extraCount, 2);
  assert.equal(summary.hasExtraTasks, true);
  assert.deepEqual(summary.extraTasks.map((task) => task.id), ['second', 'third']);
});

test('summarizeBackgroundTasks uses idle label when no tasks are active', () => {
  const summary = summarizeBackgroundTasks([], 'Idle right now');

  assert.equal(summary.displayLabel, 'Idle right now');
  assert.equal(summary.currentTask, null);
  assert.equal(summary.extraCount, 0);
  assert.equal(summary.hasActiveTask, false);
});

test('updateBackgroundTaskList changes only the matching task', () => {
  const initial = [
    { id: 'first', label: 'Saving project', startedAt: 100 },
    { id: 'second', label: 'Indexing game data', startedAt: 200 },
  ];

  const updated = updateBackgroundTaskList(initial, 'second', {
    label: 'Indexing defs',
    details: 'Core + DLC',
    progress: 15,
  });

  assert.equal(updated[0].label, 'Saving project');
  assert.equal(updated[1].label, 'Indexing defs');
  assert.equal(updated[1].details, 'Core + DLC');
  assert.equal(updated[1].progress, 15);
});
