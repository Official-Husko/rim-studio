import type {
  CeRatingDto,
  CeSourceDto,
  FileActionDto,
  FileKindDto,
  SeverityDto,
} from 'rimstudio-ipc-types';
import type { BadgeTone } from 'rimstudio-ui';
import { t } from '~/shared/i18n';

/** The word for what the plan does with a file. */
export function actionText(action: FileActionDto): string {
  switch (action) {
    case 'create':
      return t('designer.output.action.create');
    case 'update-region':
      return t('designer.output.action.update');
    default:
      return t('designer.output.action.unchanged');
  }
}

/** The tone of an action badge: a new or changed file stands out, an unchanged one does not. */
export function actionTone(action: FileActionDto): BadgeTone {
  return action === 'create' ? 'success' : action === 'update-region' ? 'info' : 'neutral';
}

/** The role of a planned file in the mod layout. */
export function kindText(kind: FileKindDto): string {
  switch (kind) {
    case 'vanilla-defs':
      return t('designer.output.kind.defs');
    case 'ce-patch':
      return t('designer.output.kind.cePatch');
    case 'load-folders':
      return t('designer.output.kind.loadFolders');
    default:
      return t('designer.output.kind.about');
  }
}

/** The word for how reliable the backend rates a derived number. */
export function ratingText(rating: CeRatingDto): string {
  switch (rating) {
    case 'reliable':
      return t('designer.output.rating.reliable');
    case 'rough':
      return t('designer.output.rating.rough');
    case 'unreliable':
      return t('designer.output.rating.unreliable');
    default:
      return t('designer.output.rating.unmeasured');
  }
}

/** The tone of a rating badge. */
export function ratingTone(rating: CeRatingDto): BadgeTone {
  switch (rating) {
    case 'reliable':
      return 'success';
    case 'rough':
      return 'warning';
    case 'unreliable':
      return 'danger';
    default:
      return 'neutral';
  }
}

/** Where a derived or held number came from, in a few words. */
export function sourceText(source: CeSourceDto): string {
  switch (source.kind) {
    case 'typed':
      return t('designer.output.source.typed');
    case 'anchor':
      return t('designer.output.source.anchor');
    case 'answered':
      return t('designer.output.source.answered');
    case 'identity':
      return t('designer.output.source.identity', { n: source.n });
    case 'predicted':
      return t('designer.output.source.predicted', { n: source.n });
    case 'vanilla':
      return t('designer.output.source.vanilla');
    default:
      return t('designer.output.source.firstOfSet');
  }
}

/** The tone of a severity badge. */
export function severityTone(severity: SeverityDto): BadgeTone {
  switch (severity) {
    case 'error':
      return 'danger';
    case 'warning':
      return 'warning';
    case 'info':
      return 'info';
    default:
      return 'neutral';
  }
}
