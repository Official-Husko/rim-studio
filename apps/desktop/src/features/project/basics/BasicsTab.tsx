import { useEffect } from 'preact/hooks';
import { Banner, Button, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import { AdvancedSection } from './AdvancedSection';
import { ChangesBar } from './ChangesBar';
import { DependenciesSection } from './DependenciesSection';
import { DescriptionSection } from './DescriptionSection';
import { FindingsSummary } from './FindingsSummary';
import { IdentitySection } from './IdentitySection';
import { ImagesSection } from './ImagesSection';
import { LoadOrderSection } from './LoadOrderSection';
import { VersionsSection } from './VersionsSection';
import { aboutError, aboutModel, aboutState, loadAbout } from './aboutStore';

export interface BasicsTabProps {
  projectId: string;
}

/** The basics of the mod: About.xml as a form with live findings, a review of the change and a save. */
export function BasicsTab({ projectId }: BasicsTabProps) {
  useEffect(() => {
    void loadAbout(projectId, true);
  }, [projectId]);
  const about = aboutModel.value;

  if (aboutState.value === 'error' && !about) {
    return (
      <div class="p-4">
        <Banner
          tone="error"
          title={t('project.basics.error')}
          action={
            <Button size="sm" variant="secondary" onClick={() => void loadAbout(projectId)}>
              {t('project.basics.retry')}
            </Button>
          }
        >
          {aboutError.value?.message}
        </Banner>
      </div>
    );
  }
  if (!about) {
    return (
      <div class="p-4">
        <Spinner label={t('project.basics.loading')} />
      </div>
    );
  }
  return (
    <div class="flex min-h-0 flex-col">
      <div class="flex flex-col gap-4 p-4">
        {!about.editable ? (
          <Banner tone="warning" title={t('project.basics.locked')}>
            {about.notEditableReason ?? t('project.basics.locked.body')}
          </Banner>
        ) : null}
        <FindingsSummary />
        <IdentitySection />
        <DescriptionSection />
        <VersionsSection />
        <DependenciesSection />
        <LoadOrderSection />
        <ImagesSection />
        <AdvancedSection />
      </div>
      <ChangesBar projectId={projectId} />
    </div>
  );
}
