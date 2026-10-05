import { useState } from 'preact/hooks';
import { Card, KeyValueList, Panel, SplitPane, Tabs } from 'rimstudio-ui';
import { Section } from '../Section';

/** Panels, cards, tabs and the split pane. */
export function LayoutSection() {
  const [tab, setTab] = useState('def');
  const [card, setCard] = useState('a');
  return (
    <Section id="layout" title="Panels, cards, tabs and split pane">
      <div class="grid max-w-4xl grid-cols-2 items-start gap-4">
        <Panel title="Readouts" framed>
          <KeyValueList
            items={[
              { key: 'Cycle time', value: '0.62 s', mono: true },
              { key: 'Hit adjusted DPS', value: '9.4', mono: true },
              { key: 'Armor penetration', value: '0.35', mono: true },
            ]}
          />
        </Panel>
        <Panel title="Raw values" collapsible defaultCollapsed>
          <p class="text-muted">Hidden until expanded.</p>
        </Panel>
        <Card selected={card === 'a'} onSelect={() => setCard('a')} label="Anchor Assault rifle">
          <p class="font-semibold">Assault rifle</p>
          <p class="font-mono text-mono text-muted">index 1.00</p>
        </Card>
        <Card
          tone="raised"
          selected={card === 'b'}
          onSelect={() => setCard('b')}
          label="Anchor Revolver"
        >
          <p class="font-semibold">Revolver</p>
          <p class="font-mono text-mono text-muted">index 0.71</p>
        </Card>
      </div>
      <div class="max-w-3xl">
        <Tabs
          label="Output"
          value={tab}
          onValueChange={setTab}
          tabs={[
            { id: 'def', label: 'Definition' },
            { id: 'patch', label: 'CE patch', badge: '2' },
            { id: 'files', label: 'Files', disabled: true },
            { id: 'about', label: 'About' },
          ]}
        >
          {(id) => <p class="text-muted">Panel for {id}.</p>}
        </Tabs>
      </div>
      <div class="h-48 max-w-3xl border border-line">
        <SplitPane
          label="Resize the item list"
          defaultSize={220}
          min={140}
          max={420}
          first={<p class="p-3">Item list</p>}
          second={<p class="p-3">Design form</p>}
        />
      </div>
    </Section>
  );
}
