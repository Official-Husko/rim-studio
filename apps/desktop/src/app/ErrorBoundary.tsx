import { Component, type ComponentChildren } from 'preact';
import { Banner, Button } from 'rimstudio-ui';
import { clientErrorId } from '~/shared/ipc';
import { t } from '~/shared/i18n';

interface Props {
  children?: ComponentChildren;
  /** Shown in the error card so it is clear which region failed. */
  region?: string;
}

interface State {
  error?: unknown;
  errorId?: string;
  copied?: boolean;
}

function describe(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

/** Catches render errors of a region and offers Retry and Copy diagnostics; the rest of the app keeps working. */
export class ErrorBoundary extends Component<Props, State> {
  override state: State = {};

  override componentDidCatch(error: unknown): void {
    this.setState({ error, errorId: clientErrorId() });
  }

  private copy = async (): Promise<void> => {
    const { error, errorId } = this.state;
    const text = [
      `region: ${this.props.region ?? 'app'}`,
      `errorId: ${errorId ?? ''}`,
      describe(error),
      error instanceof Error ? (error.stack ?? '') : '',
    ].join('\n');
    try {
      await navigator.clipboard.writeText(text);
      this.setState({ copied: true });
    } catch {
      /* clipboard blocked */
    }
  };

  override render() {
    const { error, errorId, copied } = this.state;
    if (error === undefined) return this.props.children;
    return (
      <div class="p-4" role="group" aria-label={this.props.region}>
        <Banner
          tone="error"
          title={t('error.title')}
          action={
            <div class="flex gap-2">
              <Button size="sm" onClick={() => this.setState({ error: undefined, copied: false })}>
                {t('error.retry')}
              </Button>
              <Button size="sm" icon="copy" onClick={this.copy}>
                {copied ? t('error.copied') : t('error.copy')}
              </Button>
            </div>
          }
        >
          <span class="break-words">{describe(error)}</span>
          <span class="ml-2 font-mono text-mono-small text-faint">
            {t('error.id', { id: errorId ?? '' })}
          </span>
        </Banner>
      </div>
    );
  }
}
