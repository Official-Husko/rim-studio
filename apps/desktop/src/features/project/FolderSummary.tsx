import { Badge, KeyValueList } from 'rimstudio-ui';
import type { LayoutIssueDto, TreeNodeDto } from 'rimstudio-ipc-types';
import { formatBytes, formatNumber } from '~/shared/format';
import { t, tn } from '~/shared/i18n';
import { ROLE_LABEL } from './model';

export interface FolderSummaryProps {
  node: TreeNodeDto;
  issues: readonly LayoutIssueDto[];
}

/** What the viewer shows for a folder: its role, size and the issues on or below it. */
export function FolderSummary({ node, issues }: FolderSummaryProps) {
  return (
    <div class="flex flex-col gap-3 p-4">
      <KeyValueList
        label={t('project.folder.label')}
        items={[
          {
            key: t('project.folder.path'),
            value: node.path === '' ? node.name : node.path,
            mono: true,
          },
          {
            key: t('project.folder.role'),
            value: <Badge tone="info">{t(ROLE_LABEL[node.role])}</Badge>,
          },
          { key: t('project.folder.files'), value: formatNumber(node.files, 0), mono: true },
          { key: t('project.folder.size'), value: formatBytes(node.bytes), mono: true },
          {
            key: t('project.folder.children'),
            value: formatNumber(node.children.length, 0),
            mono: true,
          },
        ]}
      />
      {issues.length > 0 ? (
        <ul class="m-0 flex list-none flex-col gap-1 p-0">
          <li class="text-muted">{tn('project.folder.issues', issues.length)}</li>
          {issues.map((issue) => (
            <li key={`${issue.code}:${issue.path}`} class="text-small">
              {issue.message}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
