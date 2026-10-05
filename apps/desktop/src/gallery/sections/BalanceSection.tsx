import { useState } from 'preact/hooks';
import { KeyValueList, Meter, RulerSlider } from 'rimstudio-ui';
import { Section } from '../Section';

/** The fit meter and the ruler slider with illustrative numbers. */
export function BalanceSection() {
  const [damage, setDamage] = useState(14);
  const [warmup, setWarmup] = useState(1.1);
  return (
    <Section id="balance" title="Fit meter and ruler slider">
      <div class="grid max-w-3xl grid-cols-2 gap-6">
        <Meter
          label="Hit adjusted DPS"
          unit="dps"
          min={0}
          max={20}
          value={9.4}
          p50={[7, 11]}
          p80={[5, 14]}
          prediction={9}
          level="typical"
        />
        <Meter
          label="Mass"
          unit="kg"
          min={0}
          max={10}
          value={6.2}
          p50={[3, 4.5]}
          p80={[2, 6]}
          level="unusual"
        />
        <Meter
          label="Market value"
          unit="silver"
          min={0}
          max={1500}
          value={780}
          p50={[500, 700]}
          p80={[300, 900]}
          level="plausible"
        />
        <Meter label="Range" unit="tiles" min={0} max={60} value={30} level="unknown" />
      </div>
      <div class="max-w-2xl">
        <RulerSlider
          label="Damage"
          unit="dmg"
          min={0}
          max={40}
          step={1}
          value={damage}
          onValueChange={setDamage}
          marks={[
            { value: 5, label: 'p10 5' },
            { value: 16, label: 'median 16' },
            { value: 30, label: 'p90 30' },
            { value: damage, label: 'this item', kind: 'item' },
          ]}
          p50={[11, 21]}
          p80={[7, 27]}
          suggested={16}
        />
        <p class="mt-2 font-mono text-mono text-muted">damage {damage}</p>
      </div>
      <div class="max-w-2xl">
        <RulerSlider
          label="Warmup"
          unit="s"
          min={0}
          max={4}
          step={0.05}
          value={warmup}
          onValueChange={setWarmup}
          p50={[0.8, 1.6]}
          p80={[0.5, 2.2]}
          suggested={1.2}
        />
        <KeyValueList items={[{ key: 'warmup', value: `${warmup.toFixed(2)} s`, mono: true }]} />
      </div>
    </Section>
  );
}
