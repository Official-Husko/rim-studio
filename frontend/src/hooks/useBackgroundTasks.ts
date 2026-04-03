import { useCallback, useMemo, useRef, useState } from 'preact/hooks';
import type { BackgroundTask } from '../types';
import {
  appendBackgroundTask,
  removeBackgroundTask,
  summarizeBackgroundTasks,
  updateBackgroundTaskList,
  upsertBackgroundTask,
} from '../utils/backgroundTasks';

interface BackgroundTaskInput {
  id?: string;
  label: string;
  progress?: number;
  details?: string;
}

interface BackgroundTaskUpdate {
  label?: string;
  progress?: number;
  details?: string;
}

export function useBackgroundTasks() {
  const [tasks, setTasks] = useState<BackgroundTask[]>([]);
  const sequence = useRef(0);

  const startTask = useCallback((input: BackgroundTaskInput) => {
    const id = input.id ?? `background-task-${Date.now()}-${sequence.current++}`;
    setTasks((current) => appendBackgroundTask(current, { ...input, id }, Date.now()));
    return id;
  }, []);

  const updateTask = useCallback((id: string, update: BackgroundTaskUpdate) => {
    setTasks((current) => updateBackgroundTaskList(current, id, update));
  }, []);

  const upsertTask = useCallback((input: BackgroundTaskInput) => {
    const id = input.id ?? `background-task-${Date.now()}-${sequence.current++}`;
    setTasks((current) => upsertBackgroundTask(current, { ...input, id }, Date.now()));

    return id;
  }, []);

  const finishTask = useCallback((id: string) => {
    setTasks((current) => removeBackgroundTask(current, id));
  }, []);

  const runTask = useCallback(
    async <T>(input: BackgroundTaskInput, action: (taskID: string) => Promise<T>) => {
      const taskID = startTask(input);

      try {
        return await action(taskID);
      } finally {
        finishTask(taskID);
      }
    },
    [finishTask, startTask],
  );

  const summary = useMemo(() => summarizeBackgroundTasks(tasks), [tasks]);

  return {
    tasks,
    currentTask: summary.currentTask,
    extraTasks: summary.extraTasks,
    startTask,
    updateTask,
    upsertTask,
    finishTask,
    runTask,
  };
}
