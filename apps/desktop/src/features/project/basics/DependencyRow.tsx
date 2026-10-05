import { Button, FormField, TextField } from 'rimstudio-ui';
import type { DiagnosticDto } from 'rimstudio-ipc-types';
import { t } from '~/shared/i18n';
import { FieldFindings } from './FieldFindings';
import { MoveButtons } from './MoveButtons';
import type { DraftDependency } from './aboutModel';

export interface DependencyRowProps {
  row: DraftDependency;
  index: number;
  count: number;
  findings: readonly DiagnosticDto[];
  disabled: boolean;
  onChange: (patch: Partial<DraftDependency>) => void;
  onRemove: () => void;
  onMove: (to: number) => void;
}

/** One dependency: display name, package id and Workshop link, with move and remove. */
export function DependencyRow({
  row,
  index,
  count,
  findings,
  disabled,
  onChange,
  onRemove,
  onMove,
}: DependencyRowProps) {
  const name = row.displayName || row.packageId || t('project.basics.deps.unnamed');
  return (
    <li>
      <div
        role="group"
        aria-label={t('project.basics.deps.row', { name })}
        class="flex flex-col gap-2 border border-line-subtle p-3"
      >
        <div class="grid grid-cols-1 gap-3 md:grid-cols-3">
          <FormField label={t('project.basics.deps.name')}>
            <TextField
              value={row.displayName}
              disabled={disabled}
              onValueChange={(displayName) => onChange({ displayName })}
            />
          </FormField>
          <FormField label={t('project.basics.deps.id')} required>
            <TextField
              value={row.packageId}
              disabled={disabled}
              onValueChange={(packageId) => onChange({ packageId })}
            />
          </FormField>
          <FormField label={t('project.basics.deps.url')}>
            <TextField
              value={row.steamWorkshopUrl ?? ''}
              disabled={disabled}
              onValueChange={(steamWorkshopUrl) => onChange({ steamWorkshopUrl })}
            />
          </FormField>
        </div>
        <FieldFindings items={findings} />
        {row.packageId.trim() === '' ? (
          <p class="m-0 text-small text-muted">{t('project.basics.deps.incomplete')}</p>
        ) : null}
        <div class="flex items-center justify-between gap-2">
          <MoveButtons
            name={name}
            canUp={index > 0}
            canDown={index < count - 1}
            disabled={disabled}
            onUp={() => onMove(index - 1)}
            onDown={() => onMove(index + 1)}
          />
          <Button
            size="sm"
            variant="ghost"
            icon="trash"
            disabled={disabled}
            aria-label={t('project.basics.deps.remove', { name })}
            onClick={onRemove}
          >
            {t('project.basics.deps.removeShort')}
          </Button>
        </div>
      </div>
    </li>
  );
}
