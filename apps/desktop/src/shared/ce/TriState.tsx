import { SegmentedControl } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

export interface TriStateProps {
  label: string;
  /** Undefined means the conversion decides. */
  value: boolean | undefined;
  onChange: (value: boolean | undefined) => void;
  /** The text of the undecided choice. */
  autoLabel?: string;
}

const KEY = { auto: 'auto', yes: 'yes', no: 'no' } as const;

/** A yes, no or let the conversion decide choice for a member the block may leave out. */
export function TriState({ label, value, onChange, autoLabel }: TriStateProps) {
  const current = value === undefined ? KEY.auto : value ? KEY.yes : KEY.no;
  return (
    <SegmentedControl
      label={label}
      value={current}
      onValueChange={(v) => onChange(v === KEY.auto ? undefined : v === KEY.yes)}
      options={[
        { value: KEY.auto, label: autoLabel ?? t('ceblock.auto') },
        { value: KEY.yes, label: t('ceblock.yes') },
        { value: KEY.no, label: t('ceblock.no') },
      ]}
    />
  );
}
