import { SegmentedControl } from 'rimstudio-ui';
import { density, setDensity, type Density } from '~/app/theme';
import { ActionsSection } from './sections/ActionsSection';
import { BalanceSection } from './sections/BalanceSection';
import { CodeSection } from './sections/CodeSection';
import { DataSection } from './sections/DataSection';
import { FeedbackSection } from './sections/FeedbackSection';
import { FieldsSection } from './sections/FieldsSection';
import { IconsSection } from './sections/IconsSection';
import { LayoutSection } from './sections/LayoutSection';

const LINKS = [
  ['actions', 'Actions'],
  ['fields', 'Fields'],
  ['balance', 'Balance'],
  ['feedback', 'Feedback'],
  ['data', 'Data'],
  ['code', 'Code'],
  ['layout', 'Layout'],
  ['icons', 'Icons'],
] as const;

/** The development gallery: every rimstudio-ui component with realistic props. */
export default function Gallery() {
  return (
    <div class="h-full overflow-auto" id="gallery-scroll">
      <header class="sticky top-0 z-(--rs-z-menu) flex flex-wrap items-center gap-4 border-b border-line bg-surface px-6 py-2">
        <h1 class="font-display text-title font-semibold tracking-display">Component gallery</h1>
        <nav aria-label="Gallery sections" class="flex flex-wrap gap-3 text-small">
          {LINKS.map(([id, label]) => (
            <a
              key={id}
              href={`#${id}`}
              class="text-accent underline-offset-2 hover:underline"
              onClick={(e) => {
                e.preventDefault();
                document.getElementById(id)?.scrollIntoView();
              }}
            >
              {label}
            </a>
          ))}
        </nav>
        <div class="ml-auto">
          <SegmentedControl
            label="Density"
            value={density.value}
            onValueChange={(v) => setDensity(v as Density)}
            options={[
              { value: 'compact', label: 'Compact' },
              { value: 'comfortable', label: 'Comfortable' },
              { value: 'roomy', label: 'Roomy' },
            ]}
          />
        </div>
      </header>
      <div class="flex flex-col gap-8 p-6">
        <ActionsSection />
        <FieldsSection />
        <BalanceSection />
        <FeedbackSection />
        <DataSection />
        <CodeSection />
        <LayoutSection />
        <IconsSection />
      </div>
    </div>
  );
}
