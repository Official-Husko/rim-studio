import { CodeView, KeyValueList, Panel } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { aboutModel } from './aboutStore';

/** The per version blocks and the text of About.xml, read only. */
export function AdvancedSection() {
  const about = aboutModel.value;
  if (!about) return null;
  const { descriptions, relations } = about.byVersion;
  const hasBlocks = descriptions.length > 0 || relations.length > 0;
  return (
    <Panel title={t('project.basics.advanced')} framed collapsible defaultCollapsed>
      <div class="flex flex-col gap-4 p-3">
        <div class="flex flex-col gap-2">
          <h3 class="m-0 font-display text-small tracking-display text-muted uppercase">
            {t('project.basics.advanced.byVersion')}
          </h3>
          <p class="m-0 text-small text-muted">{t('project.basics.advanced.byVersion.help')}</p>
          {hasBlocks ? (
            <>
              {descriptions.map((d) => (
                <KeyValueList
                  key={`d-${d.version}`}
                  label={t('project.basics.advanced.descriptionFor', { version: d.version })}
                  items={[
                    {
                      key: t('project.basics.advanced.descriptionFor', { version: d.version }),
                      value: d.text,
                    },
                  ]}
                />
              ))}
              {relations.map((r) => (
                <KeyValueList
                  key={`r-${r.version}`}
                  label={t('project.basics.advanced.relationsFor', { version: r.version })}
                  items={[
                    {
                      key: t('project.basics.advanced.relationsFor', { version: r.version }),
                      value: '',
                    },
                    {
                      key: t('project.basics.advanced.dependencies'),
                      value:
                        r.modDependencies.map((x) => x.packageId).join(', ') ||
                        t('project.info.none'),
                      mono: true,
                    },
                    {
                      key: t('project.basics.order.after'),
                      value: r.loadAfter.join(', ') || t('project.info.none'),
                      mono: true,
                    },
                    {
                      key: t('project.basics.order.before'),
                      value: r.loadBefore.join(', ') || t('project.info.none'),
                      mono: true,
                    },
                    {
                      key: t('project.basics.order.incompatible'),
                      value: r.incompatibleWith.join(', ') || t('project.info.none'),
                      mono: true,
                    },
                  ]}
                />
              ))}
            </>
          ) : (
            <p class="m-0 text-small text-faint">{t('project.basics.advanced.noBlocks')}</p>
          )}
          {about.unknownTags.length > 0 ? (
            <p class="m-0 text-small text-muted">
              {t('project.basics.advanced.unknown', { tags: about.unknownTags.join(', ') })}
            </p>
          ) : null}
        </div>
        <div class="flex flex-col gap-2">
          <h3 class="m-0 font-display text-small tracking-display text-muted uppercase">
            {t('project.basics.advanced.raw')}
          </h3>
          <CodeView
            code={about.rawText}
            label={t('project.basics.advanced.rawLabel')}
            heightClass="max-h-96"
            copyLabel={t('project.basics.copy')}
            copiedLabel={t('project.basics.copied')}
          />
          {about.rawTruncated ? (
            <p class="m-0 text-small text-faint">{t('project.basics.advanced.truncated')}</p>
          ) : null}
        </div>
      </div>
    </Panel>
  );
}
