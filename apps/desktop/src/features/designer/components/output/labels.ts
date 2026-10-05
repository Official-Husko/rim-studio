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
    case 'replace':
      return t('designer.output.action.replace');
    default:
      return t('designer.output.action.unchanged');
  }
}

/** The tone of an action badge: a new or changed file stands out, an unchanged one does not. */
export function actionTone(action: FileActionDto): BadgeTone {
  switch (action) {
    case 'create':
      return 'success';
    case 'update-region':
      return 'info';
    case 'replace':
      return 'warning';
    default:
      return 'neutral';
  }
}

/**
 * The role of a planned file in the mod layout. The path tells a texture from a sound clip and the
 * sound definitions from the weapon definitions; it is used for the wording only.
 */
export function kindText(kind: FileKindDto, path = ''): string {
  switch (kind) {
    case 'copy':
      return /\.png$/i.test(path)
        ? t('designer.output.kind.texture')
        : /\.(wav|ogg)$/i.test(path)
          ? t('designer.output.kind.clip')
          : t('designer.output.kind.copy');
    case 'vanilla-defs':
      return /(^|\/)SoundDefs\//.test(path)
        ? t('designer.output.kind.soundDefs')
        : t('designer.output.kind.defs');
    case 'ce-patch':
      return t('designer.output.kind.cePatch');
    case 'ce-defs':
      return t('designer.output.kind.ceDefs');
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
