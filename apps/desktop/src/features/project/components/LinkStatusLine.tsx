import { Badge } from 'rimstudio-ui';
import type { ProjectLinkStatusDto } from 'rimstudio-ipc-types';
import { stateLabel, stateLine, stateTone } from './linkLabels';

/** The state badge and the sentence that explains it. */
export function LinkStatusLine({ status }: { status: ProjectLinkStatusDto }) {
  return (
    <div class="flex min-w-0 flex-1 items-start gap-3">
      <span class="mt-0.5 shrink-0">
        <Badge tone={stateTone(status.state)}>{stateLabel(status.state)}</Badge>
      </span>
      <p class="m-0 min-w-0 flex-1 break-words text-body text-fg">{stateLine(status)}</p>
    </div>
  );
}
