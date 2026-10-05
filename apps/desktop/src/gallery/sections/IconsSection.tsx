import { ICON_NAMES, Icon } from 'rimstudio-ui';
import { Section } from '../Section';

/** Every icon of the set at both sizes. */
export function IconsSection() {
  return (
    <Section id="icons" title="Icons">
      <ul class="grid max-w-4xl grid-cols-5 gap-2">
        {ICON_NAMES.map((name) => (
          <li key={name} class="flex items-center gap-2 border border-line-subtle p-2 text-muted">
            <Icon name={name} size={16} />
            <Icon name={name} size={20} />
            <span class="font-mono text-mono-small">{name}</span>
          </li>
        ))}
      </ul>
    </Section>
  );
}
