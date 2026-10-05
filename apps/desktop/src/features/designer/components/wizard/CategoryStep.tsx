import { useState } from 'preact/hooks';
import type { ArchetypeCatalogDto, ArchetypeFamilyDto, ItemKindDto } from 'rimstudio-ipc-types';
import { Banner, Card, Spinner } from 'rimstudio-ui';
import { tn, t } from '~/shared/i18n';
import type { ApiError } from 'rimstudio-ipc-types';
import { SectionHeading } from './SectionHeading';
import { familiesOf, findArchetype } from './wizard-model';

export interface CategoryStepProps {
  catalog: ArchetypeCatalogDto | undefined;
  error: ApiError | undefined;
  /** The chosen archetype id. */
  chosen: string | undefined;
  onChoose: (archetypeId: string) => void;
}

function FamilyList({
  title,
  families,
  openId,
  onOpen,
}: {
  title: string;
  families: ArchetypeFamilyDto[];
  openId: string | undefined;
  onOpen: (id: string) => void;
}) {
  return (
    <section class="flex flex-col gap-2" aria-label={title}>
      <SectionHeading>{title}</SectionHeading>
      <ul class="grid grid-cols-2 gap-2 lg:grid-cols-1">
        {families.map((family) => (
          <li key={family.id}>
            <Card
              selected={family.id === openId}
              onSelect={() => onOpen(family.id)}
              label={family.label}
            >
              <span class="flex items-baseline justify-between gap-2">
                <span class="text-body font-semibold text-fg">{family.label}</span>
                <span class="text-small text-faint">
                  {tn('designer.wizard.category.types', family.archetypes.length)}
                </span>
              </span>
            </Card>
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Step one: pick the family of weapon, then the type, from the catalogue of the install. */
export function CategoryStep({ catalog, error, chosen, onChoose }: CategoryStepProps) {
  const chosenFamily = catalog?.families.find((f) => f.archetypes.some((a) => a.id === chosen));
  const [openId, setOpenId] = useState<string | undefined>(undefined);
  if (error) {
    return (
      <Banner tone="error" title={error.code}>
        {error.message}
      </Banner>
    );
  }
  if (!catalog) {
    return (
      <div class="flex items-center gap-2 text-muted">
        <Spinner label={t('designer.wizard.loading')} />
        <span>{t('designer.wizard.loading')}</span>
      </div>
    );
  }
  const familyId =
    openId ??
    chosenFamily?.id ??
    (catalog.families.find((f) => f.id === 'rifle') ?? catalog.families[0])?.id;
  const family = catalog.families.find((f) => f.id === familyId);
  const kinds: Array<{ kind: ItemKindDto; title: string }> = [
    { kind: 'ranged', title: t('designer.wizard.category.ranged') },
    { kind: 'melee', title: t('designer.wizard.category.melee') },
  ];
  const pick = findArchetype(catalog, chosen);
  return (
    <div class="flex flex-col gap-4">
      {!catalog.proposalsAvailable ? (
        <Banner tone="warning" title={t('designer.wizard.noInstall.title')}>
          {t('designer.wizard.noInstall.body')}
        </Banner>
      ) : null}
      <p class="text-body text-muted">{t('designer.wizard.category.intro')}</p>
      <div class="grid grid-cols-1 gap-5 lg:grid-cols-[16rem_1fr]">
        <div class="flex flex-col gap-4">
          {kinds.map(({ kind, title }) => (
            <FamilyList
              key={kind}
              title={title}
              families={familiesOf(catalog, kind)}
              openId={familyId}
              onOpen={setOpenId}
            />
          ))}
        </div>
        {family ? (
          <section
            class="flex min-w-0 flex-col gap-2"
            aria-label={t('designer.wizard.category.typesOf', { family: family.label })}
          >
            <SectionHeading>
              {t('designer.wizard.category.typesOf', { family: family.label })}
            </SectionHeading>
            <ul class="grid grid-cols-1 gap-2 xl:grid-cols-2">
              {family.archetypes.map((a) => (
                <li key={a.id}>
                  <Card selected={a.id === chosen} onSelect={() => onChoose(a.id)} label={a.label}>
                    <span class="flex flex-col gap-0.5">
                      <span class="text-body font-semibold text-fg">{a.label}</span>
                      <span class="text-small text-muted">{a.summary}</span>
                    </span>
                  </Card>
                </li>
              ))}
            </ul>
          </section>
        ) : null}
      </div>
      {pick ? (
        <p class="text-small text-muted" role="status">
          {t('designer.wizard.category.chosen', { name: pick.label })}
        </p>
      ) : null}
    </div>
  );
}
