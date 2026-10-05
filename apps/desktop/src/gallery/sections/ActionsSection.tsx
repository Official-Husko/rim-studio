import { useState } from 'preact/hooks';
import {
  Badge,
  Button,
  Checkbox,
  Chip,
  IconButton,
  SegmentedControl,
  Spinner,
  Switch,
  Tooltip,
} from 'rimstudio-ui';
import { Demo, Section } from '../Section';

/** Buttons, toggles and small labels. */
export function ActionsSection() {
  const [kind, setKind] = useState('ranged');
  const [ce, setCe] = useState(false);
  const [pin, setPin] = useState(true);
  const [pressed, setPressed] = useState(false);
  return (
    <Section id="actions" title="Buttons, toggles and labels">
      <Demo label="Button variants">
        <Button variant="primary">Write files</Button>
        <Button variant="secondary">Preview</Button>
        <Button variant="ghost">Cancel</Button>
        <Button variant="danger">Delete draft</Button>
      </Demo>
      <Demo label="Sizes, icons, loading, disabled">
        <Button size="sm" icon="plus">
          New weapon
        </Button>
        <Button icon="play" variant="primary">
          Run dry run
        </Button>
        <Button loading variant="primary">
          Applying
        </Button>
        <Button disabled>Unavailable</Button>
        <Button iconEnd="chevron">Next</Button>
      </Demo>
      <Demo label="Icon buttons with tooltips">
        <IconButton icon="refresh" label="Rescan library" />
        <IconButton icon="copy" label="Copy path" variant="subtle" />
        <IconButton icon="trash" label="Delete" variant="danger" />
        <IconButton
          icon="lock"
          label="Lock value"
          pressed={pressed}
          onClick={() => setPressed(!pressed)}
        />
        <IconButton icon="settings" label="Settings" disabled />
      </Demo>
      <Demo label="Segmented control">
        <SegmentedControl
          label="Item kind"
          value={kind}
          onValueChange={setKind}
          options={[
            { value: 'ranged', label: 'Ranged', icon: 'crosshair' },
            { value: 'melee', label: 'Melee' },
            { value: 'apparel', label: 'Apparel', disabled: true },
          ]}
        />
      </Demo>
      <Demo label="Checkbox and switch">
        <Checkbox checked={pin} onCheckedChange={setPin}>
          Keep a backup of changed files
        </Checkbox>
        <Checkbox checked={false} indeterminate onCheckedChange={() => {}}>
          Some weapons selected
        </Checkbox>
        <Switch checked={ce} onCheckedChange={setCe}>
          Write a Combat Extended patch
        </Switch>
        <Switch checked disabled onCheckedChange={() => {}}>
          Vanilla definition
        </Switch>
      </Demo>
      <Demo label="Badges, chips and status">
        <Badge>draft</Badge>
        <Badge tone="info">plausible</Badge>
        <Badge tone="success">written</Badge>
        <Badge tone="warning">out of date</Badge>
        <Badge tone="danger">blocked</Badge>
        <Chip kind="typed" />
        <Chip kind="suggested" />
        <Chip kind="anchor">Anchor: Assault rifle</Chip>
        <Chip kind="derived" />
        <Chip removeLabel="Remove tag" onRemove={() => {}}>
          weapon
        </Chip>
        <Spinner size="md" label="Scanning" />
        <Tooltip text="Tooltips are CSS only">
          <button type="button" class="rounded-md border border-line-strong px-2 py-1 text-small">
            Hover or focus me
          </button>
        </Tooltip>
      </Demo>
    </Section>
  );
}
