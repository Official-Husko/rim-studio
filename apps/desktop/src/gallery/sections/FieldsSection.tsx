import { useState } from 'preact/hooks';
import { Chip, Combobox, FormField, NumberField, Select, TextField } from 'rimstudio-ui';
import { Demo, Section } from '../Section';

const ANCHORS = [
  { value: 'Gun_AssaultRifle', label: 'Assault rifle', hint: 'Gun_AssaultRifle' },
  { value: 'Gun_Revolver', label: 'Revolver', hint: 'Gun_Revolver' },
  { value: 'Gun_BoltActionRifle', label: 'Bolt-action rifle', hint: 'Gun_BoltActionRifle' },
  { value: 'Gun_ChargeRifle', label: 'Charge rifle', hint: 'Gun_ChargeRifle' },
  { value: 'Gun_Shotgun', label: 'Pump shotgun', hint: 'Gun_Shotgun' },
];

/** Text, number, select and combobox fields inside FormField. */
export function FieldsSection() {
  const [name, setName] = useState('Plasma carbine');
  const [defName, setDefName] = useState('Gun bad name');
  const [damage, setDamage] = useState<number | undefined>(14);
  const [range, setRange] = useState<number | undefined>(27.9);
  const [warmup, setWarmup] = useState<number | undefined>(undefined);
  const [kind, setKind] = useState<string | undefined>('ranged');
  const [anchor, setAnchor] = useState<string | undefined>('Gun_AssaultRifle');
  const [notes, setNotes] = useState('Short barrel, high rate of fire.');
  return (
    <Section id="fields" title="Fields">
      <div class="grid max-w-3xl grid-cols-2 gap-4">
        <FormField label="Label" help="The name players see.">
          <TextField value={name} onValueChange={setName} />
        </FormField>
        <FormField label="defName" error="Use letters, digits and underscores only." required>
          <TextField value={defName} onValueChange={setDefName} />
        </FormField>
        <FormField label="Damage" help="Arrow keys step by 1; Shift steps by 10.">
          <NumberField
            value={damage}
            onValueChange={setDamage}
            step={1}
            min={1}
            max={60}
            unit="dmg"
            chip={<Chip kind="typed" />}
          />
        </FormField>
        <FormField label="Range">
          <NumberField
            value={range}
            onValueChange={setRange}
            step={0.1}
            min={1}
            max={80}
            unit="tiles"
            chip={<Chip kind="suggested" />}
          />
        </FormField>
        <FormField label="Warmup (empty)">
          <NumberField
            value={warmup}
            onValueChange={setWarmup}
            step={0.05}
            min={0}
            max={10}
            unit="s"
            placeholder="not set"
          />
        </FormField>
        <FormField label="Out of range">
          <NumberField value={99} onValueChange={() => {}} min={1} max={60} unit="dmg" invalid />
        </FormField>
        <FormField label="Item kind">
          <Select
            value={kind}
            onValueChange={setKind}
            options={[
              { value: 'ranged', label: 'Ranged weapon' },
              { value: 'melee', label: 'Melee weapon' },
              { value: 'apparel', label: 'Apparel', group: 'Wearable' },
            ]}
          />
        </FormField>
        <FormField label="Anchor weapon" help="Type to filter.">
          <Combobox
            options={ANCHORS}
            value={anchor}
            onValueChange={setAnchor}
            placeholder="Search weapons"
          />
        </FormField>
      </div>
      <Demo label="Multi line, read only and disabled">
        <div class="w-80">
          <TextField aria-label="Notes" multiline rows={3} value={notes} onValueChange={setNotes} />
        </div>
        <div class="w-60">
          <TextField
            aria-label="Folder"
            readOnly
            value="/mods/Plasma Carbine"
            onValueChange={() => {}}
            prefix="@"
          />
        </div>
        <div class="w-60">
          <TextField aria-label="Disabled" disabled value="Locked" onValueChange={() => {}} />
        </div>
      </Demo>
    </Section>
  );
}
