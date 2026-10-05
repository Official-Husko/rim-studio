import { Banner, Button, Dialog, ProgressBar, Spinner } from 'rimstudio-ui';
import { t } from '~/shared/i18n';
import type { QuizStore } from '../../quiz-store';
import { EstimateSummary } from './EstimateSummary';
import { QuizQuestion } from './QuizQuestion';

export interface QuizStepperProps {
  store: QuizStore;
}

/** The estimate dialogue as a modal stepper: one question, the answers, back and a way out at any time. */
export function QuizStepper({ store }: QuizStepperProps) {
  const open = store.open.value;
  const step = store.step.value;
  const busy = store.busy.value;
  const error = store.error.value;
  const prompt = step?.prompt;
  const finished = step?.finished === true || (step !== undefined && prompt === undefined);

  return (
    <Dialog
      open={open}
      title={t('designer.quiz.title')}
      onClose={store.close}
      size="lg"
      closeLabel={t('designer.quiz.close')}
      footer={
        <>
          <Button disabled={busy || !step || step.answered === 0} onClick={() => void store.back()}>
            {t('designer.quiz.back')}
          </Button>
          {!finished && prompt ? (
            <>
              <Button disabled={busy} onClick={() => void store.answer({ kind: 'not-sure' })}>
                {t('designer.quiz.notSure')}
              </Button>
              <Button disabled={busy} onClick={() => void store.answer({ kind: 'skip' })}>
                {t('designer.quiz.skip')}
              </Button>
              <Button
                disabled={busy}
                onClick={() => void store.answer({ kind: 'use-what-i-have' })}
              >
                {t('designer.quiz.useWhatIHave')}
              </Button>
            </>
          ) : null}
          <Button variant="primary" onClick={store.close}>
            {finished ? t('designer.quiz.done') : t('designer.quiz.pause')}
          </Button>
        </>
      }
    >
      <div class="flex flex-col gap-4">
        {error ? (
          <Banner tone="error" title={error.code}>
            {error.message}
          </Banner>
        ) : null}
        {!step ? (
          <div class="flex justify-center py-6">
            <Spinner size="md" label={t('app.loading')} />
          </div>
        ) : (
          <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
            <div class="flex flex-col gap-3 md:col-span-2">
              {prompt && !finished ? (
                <>
                  <ProgressBar
                    label={t('designer.quiz.progressLabel')}
                    value={
                      prompt.aboutTotal > 0
                        ? Math.min(1, (prompt.number - 1) / prompt.aboutTotal)
                        : undefined
                    }
                    caption={t('designer.quiz.progress', {
                      n: prompt.number,
                      total: prompt.aboutTotal,
                    })}
                  />
                  <QuizQuestion
                    question={prompt.question}
                    disabled={busy}
                    onAnswer={(answer) => void store.answer(answer)}
                  />
                </>
              ) : (
                <Banner tone="success">{t('designer.quiz.finished')}</Banner>
              )}
            </div>
            <aside
              class="flex flex-col gap-2 border-l border-line-subtle pl-4"
              aria-label={t('designer.quiz.liveEstimate')}
            >
              <h3 class="font-display text-label tracking-label text-muted uppercase">
                {t('designer.quiz.liveEstimate')}
              </h3>
              {step.estimate ? (
                <EstimateSummary estimate={step.estimate} implied={step.implied} />
              ) : null}
            </aside>
          </div>
        )}
      </div>
    </Dialog>
  );
}
