import type { ConvertCountsDto } from 'rimstudio-ipc-types';
import { Badge, Button, Checkbox, Switch } from 'rimstudio-ui';
import { t, tn } from '~/shared/i18n';

export interface ScanBarProps {
  counts: ConvertCountsDto;
  includeConverted: boolean;
  onIncludeConverted: (on: boolean) => void;
  onRescan: () => void;
  rescanning: boolean;
  convertible: number;
  checked: number;
  onCheckAll: (on: boolean) => void;
  onConvert: () => void;
}

/** The counts of the scan and the controls above the table. */
export function ScanBar(props: ScanBarProps) {
  const { counts } = props;
  return (
    <div class="flex flex-col gap-3 p-3">
      <ul aria-label={t('patches.counts.label')} class="m-0 flex list-none flex-wrap gap-2 p-0">
        <li>
          <Badge tone="info">{t('patches.counts.not-converted', { n: counts.notConverted })}</Badge>
        </li>
        <li>
          <Badge tone="success">{t('patches.counts.already-ce', { n: counts.alreadyCe })}</Badge>
        </li>
        {counts.unsupportedKind > 0 ? (
          <li>
            <Badge tone="warning">
              {t('patches.counts.unsupported', { n: counts.unsupportedKind })}
            </Badge>
          </li>
        ) : null}
        {counts.targetNotFound > 0 ? (
          <li>
            <Badge tone="danger">{t('patches.counts.target', { n: counts.targetNotFound })}</Badge>
          </li>
        ) : null}
      </ul>
      <div class="flex flex-wrap items-center gap-3">
        <Switch checked={props.includeConverted} onCheckedChange={props.onIncludeConverted}>
          {t('patches.include-converted')}
        </Switch>
        <Button
          size="sm"
          variant="ghost"
          icon="refresh"
          loading={props.rescanning}
          onClick={props.onRescan}
        >
          {t('patches.rescan')}
        </Button>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <Checkbox
          checked={props.convertible > 0 && props.checked === props.convertible}
          indeterminate={props.checked > 0 && props.checked < props.convertible}
          disabled={props.convertible === 0}
          onCheckedChange={props.onCheckAll}
        >
          {tn('patches.select-all', props.convertible)}
        </Checkbox>
        <Button
          variant="primary"
          icon="patch"
          disabled={props.checked === 0}
          onClick={props.onConvert}
        >
          {tn('patches.convert-selected', props.checked)}
        </Button>
      </div>
    </div>
  );
}
