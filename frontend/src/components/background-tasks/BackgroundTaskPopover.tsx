import { h } from 'preact';
import { useLayoutEffect, useRef, useState } from 'preact/hooks';
import type { BackgroundTask } from '../../types';

interface Props {
  tasks: BackgroundTask[];
  open: boolean;
  scrollable?: boolean;
  maxVisibleItems?: number;
  maxHeightPx?: number;
  onMouseEnter?: () => void;
  onMouseLeave?: () => void;
}

export function BackgroundTaskPopover({
  tasks,
  open,
  scrollable = false,
  maxVisibleItems = 4,
  maxHeightPx = 320,
  onMouseEnter,
  onMouseLeave,
}: Props) {
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const [overflowing, setOverflowing] = useState(false);

  useLayoutEffect(() => {
    if (!open || !scrollable || !popoverRef.current) {
      setOverflowing(false);
      return;
    }

    setOverflowing(popoverRef.current.scrollHeight > maxHeightPx);
  }, [maxHeightPx, open, scrollable, tasks.length]);

  if (tasks.length === 0) {
    return null;
  }

  const shouldScroll = scrollable && (tasks.length > maxVisibleItems || overflowing);

  return (
    <div
      className={`background-task-popover ${open ? 'is-open' : ''} ${shouldScroll ? 'is-scrollable' : ''}`}
      ref={popoverRef}
      role="status"
      style={shouldScroll ? { maxHeight: `${maxHeightPx}px` } : undefined}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
    >
      {tasks.map((task) => {
        const hasProgress = typeof task.progress === 'number';
        const progressLabel = hasProgress ? `${task.progress}%` : 'working';

        return (
          <div className="background-task-popover-item" key={task.id}>
            <div className="background-task-popover-row">
              <div className="background-task-popover-copy">
                <span className="background-task-popover-label is-active">{task.label}</span>
                {task.details ? <span className="background-task-popover-detail">{task.details}</span> : null}
              </div>
              <span className="background-task-popover-metric">{progressLabel}</span>
            </div>

            <div className="background-task-popover-progress" aria-hidden="true">
              <div
                className={`background-task-popover-fill ${hasProgress ? '' : 'is-indeterminate'}`}
                style={hasProgress ? { width: `${task.progress}%` } : undefined}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
