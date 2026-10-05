import type { CeSuggestionDto, DesignSpecDto } from 'rimstudio-ipc-types';
import { SegmentedControl } from 'rimstudio-ui';
import { CeExtrasEditor } from '~/shared/ce';
import { t } from '~/shared/i18n';
import { getAt } from '../../model/pointer';
import type { OutputStore } from '../../output-store';
import { sourced } from '../../output-model';
import { AmmoSetAsk } from './ce/AmmoSetAsk';
import { CeChecklist } from './CeChecklist';
import { CeChoiceRow } from './CeChoiceRow';
import { CeFieldRow } from './CeFieldRow';
import { CePatchNumbers } from './CePatchNumbers';

export interface CeBodyProps {
  store: OutputStore;
  suggestion: CeSuggestionDto;
  spec: DesignSpecDto;
  onGoTo: (pointer: string) => void;
}

function Heading({ children }: { children: string }) {
  return (
    <h3 class="font-display text-label font-semibold tracking-label text-muted uppercase">
      {children}
    </h3>
  );
}

/** The suggestion of the engine as a form: the numbers, the choices and what is still to answer. */
export function CeBody({ store, suggestion, spec, onGoTo }: CeBodyProps) {
  const accepted = new Set(store.accepted.value);
  const mode = store.acceptMode.value;
  const labels = new Map<string, string>([
    ...suggestion.fields.map((f) => [f.field, f.label] as const),
    ...suggestion.choices.map((c) => [c.field, c.label] as const),
  ]);
  const ammoSets = (
    suggestion.choices.find((c) => c.field === '/ce/ammoSet')?.candidates ?? []
  ).map((c) => ({ value: c.name, label: c.name }));
  const flags = suggestion.choices.filter((c) => c.kind === 'flag');
  // custom ammunition brings its own default projectile, so that question is not asked
  const choices = suggestion.choices.filter(
    (c) => c.kind !== 'flag' && !(spec.ce?.customAmmo && c.field === '/ce/defaultProjectile'),
  );
  return (
    <div class="flex flex-col gap-3">
      {suggestion.classLabel ? (
        <p class="text-small text-muted">
          {t('designer.output.ce.basis', { basis: suggestion.classLabel })}
        </p>
      ) : null}
      <CeChecklist pending={store.pending.value} labels={labels} onGoTo={onGoTo} />
      <div class="flex flex-col gap-1">
        <Heading>{t('designer.output.ce.takeHeading')}</Heading>
        <SegmentedControl
          label={t('designer.output.ce.takeHeading')}
          value={mode}
          onValueChange={(v) => {
            if (v === 'none' || v === 'reliable' || v === 'all') store.setAcceptMode(v);
          }}
          options={[
            { value: 'none', label: t('designer.output.ce.take.none') },
            { value: 'reliable', label: t('designer.output.ce.take.reliable') },
            { value: 'all', label: t('designer.output.ce.take.all') },
            {
              value: 'custom',
              label: t('designer.output.ce.take.custom'),
              disabled: mode !== 'custom',
            },
          ]}
        />
        <p class="text-small text-muted">{t('designer.output.ce.takeHelp')}</p>
      </div>
      <div class="flex flex-col gap-1">
        <Heading>{t('designer.output.ce.choicesHeading')}</Heading>
        <div class="flex flex-col gap-3">
          {choices.map((choice) =>
            choice.field === '/ce/ammoSet' ? (
              <AmmoSetAsk key={choice.field} store={store} spec={spec} choice={choice} />
            ) : (
              <CeChoiceRow
                key={choice.field}
                choice={choice}
                value={getAt(spec, choice.field) as string | undefined}
                accepted={accepted.has(choice.field)}
                onAccept={(on) => store.setAccepted(choice.field, on)}
                onChoose={(v) => store.answer(choice.field, v)}
              />
            ),
          )}
          {flags.map((choice) => (
            <CeChoiceRow
              key={choice.field}
              choice={choice}
              value={getAt(spec, choice.field) === true}
              accepted={false}
              onAccept={() => undefined}
              onChoose={(v) => store.answer(choice.field, v)}
            />
          ))}
        </div>
      </div>
      <div class="flex flex-col gap-1">
        <Heading>{t('designer.output.ce.numbersHeading')}</Heading>
        <div class="flex flex-col gap-3">
          {suggestion.fields.map((field) => (
            <CeFieldRow
              key={field.field}
              field={field}
              accepted={accepted.has(field.field)}
              onAccept={(on) => store.setAccepted(field.field, on)}
              onValue={(value, source) =>
                store.answer(field.field, value === undefined ? undefined : sourced(value, source))
              }
            />
          ))}
        </div>
      </div>
      <CePatchNumbers numbers={suggestion.patchNumbers} />
      {spec.ce ? (
        <div class="flex flex-col gap-1">
          <Heading>{t('designer.output.ce.extrasHeading')}</Heading>
          <p class="text-small text-muted">{t('designer.output.ce.extrasHelp')}</p>
          <CeExtrasEditor
            block={spec.ce}
            onChange={store.patchBlock}
            options={suggestion.options ?? []}
            ammoSets={ammoSets}
          />
        </div>
      ) : null}
      {suggestion.notes.length > 0 ? (
        <ul class="flex flex-col gap-1">
          {suggestion.notes.map((note) => (
            <li key={note} class="text-small text-muted">
              {note}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
