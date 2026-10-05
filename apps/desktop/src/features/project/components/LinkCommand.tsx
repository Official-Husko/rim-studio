import { CodeView } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { isWindowsCommand } from './linkLabels';

/** The command to run by hand, with a copy button. */
export function LinkCommand({ command }: { command: string }) {
  return (
    <div class="flex flex-col gap-2">
      <h3 class="m-0 font-display text-label font-semibold uppercase tracking-label text-muted">
        {t('project.link.manual.title')}
      </h3>
      <p class="m-0 text-small text-muted">
        {t(isWindowsCommand(command) ? 'project.link.manual.windows' : 'project.link.manual.unix')}
      </p>
      <CodeView
        code={command}
        language="text"
        lineNumbers={false}
        label={t('project.link.manual.label')}
        copyLabel={t('project.link.manual.copy')}
        copiedLabel={t('project.link.manual.copied')}
      />
    </div>
  );
}
