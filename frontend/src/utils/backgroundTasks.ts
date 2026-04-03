import type { BackgroundTask } from '../types';

export interface BackgroundTaskInput {
  id: string;
  label: string;
  progress?: number;
  details?: string;
}

export interface BackgroundTaskSummary {
  currentTask: BackgroundTask | null;
  extraTasks: BackgroundTask[];
  displayLabel: string;
  extraCount: number;
  hasActiveTask: boolean;
  hasExtraTasks: boolean;
}

export function normalizeBackgroundTaskProgress(progress?: number) {
  if (typeof progress !== 'number' || Number.isNaN(progress)) {
    return undefined;
  }

  return Math.max(0, Math.min(100, Math.round(progress)));
}

export function appendBackgroundTask(tasks: BackgroundTask[], input: BackgroundTaskInput, startedAt: number): BackgroundTask[] {
  if (tasks.some((task) => task.id === input.id)) {
    return tasks;
  }

  return [
    ...tasks,
    {
      id: input.id,
      label: input.label,
      progress: normalizeBackgroundTaskProgress(input.progress),
      details: input.details,
      startedAt,
    },
  ];
}

export function updateBackgroundTaskList(
  tasks: BackgroundTask[],
  id: string,
  update: Partial<Pick<BackgroundTask, 'label' | 'progress' | 'details'>>,
): BackgroundTask[] {
  return tasks.map((task) =>
    task.id === id
      ? {
          ...task,
          ...(update.label !== undefined ? { label: update.label } : {}),
          ...(update.details !== undefined ? { details: update.details } : {}),
          ...(update.progress !== undefined ? { progress: normalizeBackgroundTaskProgress(update.progress) } : {}),
        }
      : task,
  );
}

export function upsertBackgroundTask(tasks: BackgroundTask[], input: BackgroundTaskInput, startedAt: number): BackgroundTask[] {
  if (!tasks.some((task) => task.id === input.id)) {
    return appendBackgroundTask(tasks, input, startedAt);
  }

  return updateBackgroundTaskList(tasks, input.id, {
    label: input.label,
    details: input.details,
    progress: input.progress,
  });
}

export function removeBackgroundTask(tasks: BackgroundTask[], id: string): BackgroundTask[] {
  return tasks.filter((task) => task.id !== id);
}

export function summarizeBackgroundTasks(tasks: BackgroundTask[], idleLabel = 'No background task'): BackgroundTaskSummary {
  const currentTask = tasks[0] ?? null;
  const extraTasks = tasks.slice(1);

  return {
    currentTask,
    extraTasks,
    displayLabel: currentTask?.label ?? idleLabel,
    extraCount: extraTasks.length,
    hasActiveTask: currentTask !== null,
    hasExtraTasks: extraTasks.length > 0,
  };
}
