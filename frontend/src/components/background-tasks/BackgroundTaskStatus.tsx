import { h } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import type { BackgroundTask } from '../../types';
import { summarizeBackgroundTasks } from '../../utils/backgroundTasks';
import { BackgroundTaskPopover } from './BackgroundTaskPopover';

interface Props {
  tasks: BackgroundTask[];
  idleLabel?: string;
}

export function BackgroundTaskStatus({ tasks, idleLabel = 'No background task' }: Props) {
  const [open, setOpen] = useState(false);
  const hideTimeoutRef = useRef<number | null>(null);
  const summary = summarizeBackgroundTasks(tasks, idleLabel);
  const hasActiveTask = summary.hasActiveTask;

  useEffect(() => () => {
    if (hideTimeoutRef.current !== null) {
      window.clearTimeout(hideTimeoutRef.current);
    }
  }, []);

  function cancelHidePopover() {
    if (hideTimeoutRef.current !== null) {
      window.clearTimeout(hideTimeoutRef.current);
      hideTimeoutRef.current = null;
    }
  }

  function showPopover() {
    if (summary.hasExtraTasks) {
      cancelHidePopover();
      setOpen(true);
    }
  }

  function scheduleHidePopover() {
    cancelHidePopover();
    hideTimeoutRef.current = window.setTimeout(() => {
      setOpen(false);
      hideTimeoutRef.current = null;
    }, 140);
  }

  return (
    <div
      className={`background-task-status ${open ? 'is-open' : ''}`}
      onMouseEnter={showPopover}
      onMouseLeave={scheduleHidePopover}
      onFocusCapture={showPopover}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          scheduleHidePopover();
        }
      }}
    >
      <div
        className={`bottom-bar-task ${hasActiveTask ? 'is-active' : ''}`}
        aria-expanded={summary.hasExtraTasks ? open : undefined}
        aria-haspopup={summary.hasExtraTasks ? 'true' : undefined}
        tabIndex={summary.hasExtraTasks ? 0 : undefined}
      >
        <span className={`bottom-task-indicator ${hasActiveTask ? 'is-active' : ''}`} aria-hidden="true" />
        <span className={`bottom-task-text ${hasActiveTask ? 'is-active' : ''}`}>{summary.displayLabel}</span>
        {summary.hasExtraTasks ? <span className="bottom-task-extra-count">+ {summary.extraCount}</span> : null}
      </div>

      <BackgroundTaskPopover
        maxHeightPx={320}
        maxVisibleItems={4}
        open={open && summary.hasExtraTasks}
        onMouseEnter={cancelHidePopover}
        onMouseLeave={scheduleHidePopover}
        scrollable
        tasks={summary.extraTasks}
      />
    </div>
  );
}
