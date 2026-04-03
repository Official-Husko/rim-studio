export function normalizeBackgroundTaskProgress(progress) {
    if (typeof progress !== 'number' || Number.isNaN(progress)) {
        return undefined;
    }
    return Math.max(0, Math.min(100, Math.round(progress)));
}
export function appendBackgroundTask(tasks, input, startedAt) {
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
export function updateBackgroundTaskList(tasks, id, update) {
    return tasks.map((task) => task.id === id
        ? {
            ...task,
            ...(update.label !== undefined ? { label: update.label } : {}),
            ...(update.details !== undefined ? { details: update.details } : {}),
            ...(update.progress !== undefined ? { progress: normalizeBackgroundTaskProgress(update.progress) } : {}),
        }
        : task);
}
export function upsertBackgroundTask(tasks, input, startedAt) {
    if (!tasks.some((task) => task.id === input.id)) {
        return appendBackgroundTask(tasks, input, startedAt);
    }
    return updateBackgroundTaskList(tasks, input.id, {
        label: input.label,
        details: input.details,
        progress: input.progress,
    });
}
export function removeBackgroundTask(tasks, id) {
    return tasks.filter((task) => task.id !== id);
}
export function summarizeBackgroundTasks(tasks, idleLabel = 'No background task') {
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
