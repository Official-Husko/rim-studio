import { FormField, TextField } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { CeBlock, CeBlockPatch } from './blockModel';
import { Group } from './Group';
import { SourcedNumberField } from './SourcedNumberField';
import { TriState } from './TriState';

export interface CeBowFieldsProps {
  block: CeBlock;
  onChange: (patch: CeBlockPatch) => void;
}

function summaryOf(block: CeBlock): string {
  const parts: string[] = [];
  if (block.bow === true) parts.push(t('ceblock.bow.isBow'));
  if (block.bow === false) parts.push(t('ceblock.bow.notBow'));
  if (block.ammoGenPerMag)
    parts.push(t('ceblock.bow.perMagSummary', { n: block.ammoGenPerMag.value }));
  if (block.reloadOneAtATime !== undefined) parts.push(t('ceblock.bow.reloadSummary'));
  if (block.recoilPattern) parts.push(block.recoilPattern);
  return parts.length > 0 ? parts.join(', ') : t('ceblock.auto');
}

/**
 * The bow choice and the ammo, reload and recoil members that a gun or a bow can carry. A bow is found
 * from its shape by default; the first choice overrides that. Everything left on "decide for me" is
 * written the way the conversion decides.
 */
export function CeBowFields({ block, onChange }: CeBowFieldsProps) {
  return (
    <Group title={t('ceblock.bow.title')} summary={summaryOf(block)}>
      <p class="m-0 text-small text-muted">{t('ceblock.bow.help')}</p>
      <FormField label={t('ceblock.bow.kind')} help={t('ceblock.bow.kindHelp')}>
        <TriState
          label={t('ceblock.bow.kind')}
          value={block.bow}
          autoLabel={t('ceblock.bow.fromShape')}
          onChange={(bow) => onChange({ bow })}
        />
      </FormField>
      <SourcedNumberField
        label={t('ceblock.bow.perMag')}
        help={t('ceblock.bow.perMagHelp')}
        whole
        value={block.ammoGenPerMag}
        onChange={(ammoGenPerMag) => onChange({ ammoGenPerMag })}
      />
      <SourcedNumberField
        label={t('ceblock.bow.mass')}
        help={t('ceblock.bow.massHelp')}
        unit="kg"
        value={block.mass}
        onChange={(mass) => onChange({ mass })}
      />
      <FormField label={t('ceblock.bow.runAndGun')} help={t('ceblock.bow.runAndGunHelp')}>
        <TriState
          label={t('ceblock.bow.runAndGun')}
          value={block.allowWithRunAndGun}
          onChange={(allowWithRunAndGun) => onChange({ allowWithRunAndGun })}
        />
      </FormField>
      <FormField label={t('ceblock.bow.reload')} help={t('ceblock.bow.reloadHelp')}>
        <TriState
          label={t('ceblock.bow.reload')}
          value={block.reloadOneAtATime}
          onChange={(reloadOneAtATime) => onChange({ reloadOneAtATime })}
        />
      </FormField>
      <FormField label={t('ceblock.bow.recoil')} help={t('ceblock.bow.recoilHelp')}>
        <TextField
          aria-label={t('ceblock.bow.recoil')}
          value={block.recoilPattern ?? ''}
          onValueChange={(v) => onChange({ recoilPattern: v === '' ? undefined : v })}
        />
      </FormField>
    </Group>
  );
}
