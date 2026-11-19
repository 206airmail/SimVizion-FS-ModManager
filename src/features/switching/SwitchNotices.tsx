import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { ActionErrorBanner } from '../ActionErrorBanner';
import type { SwitchingController } from './useSwitching';

const t = strings.switching;

export interface SwitchNoticesProps {
  switching: SwitchingController;
  /** The profile on screen: notices belong to the profile they happened in. */
  profileId: string;
  className?: string;
}

/** The outcome of "Set as active" and Undo, shown next to the button that caused it. */
export function SwitchNotices({ switching, profileId, className }: SwitchNoticesProps) {
  const { done, failure, busy } = switching;
  return (
    <>
      {failure?.scope === profileId && (
        <ActionErrorBanner
          className={className}
          title={failure.title}
          message={failure.message}
          onDismiss={switching.dismissFailure}
        />
      )}
      {done?.scope === profileId &&
        (done.kind === 'switched' ? (
          <Banner
            tone="success"
            className={className}
            title={t.doneTitle(done.name)}
            actions={
              <>
                <Button
                  size="compact"
                  disabled={busy !== null}
                  aria-busy={busy === 'undo' || undefined}
                  onClick={() => void switching.undo()}
                >
                  {busy === 'undo' ? t.undoing : t.undo}
                </Button>
                <Button size="compact" onClick={switching.clear}>
                  {strings.actionFailed.dismiss}
                </Button>
              </>
            }
          >
            {t.doneText}
          </Banner>
        ) : (
          <Banner
            tone="success"
            className={className}
            title={t.undoneTitle}
            actions={
              <Button size="compact" onClick={switching.clear}>
                {strings.actionFailed.dismiss}
              </Button>
            }
          >
            {t.undoneText(done.name)}
          </Banner>
        ))}
    </>
  );
}
