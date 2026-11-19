import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import type { UpdatesController } from './useUpdates';

const t = strings.updates.prompt;

/**
 * The first-run offer to turn on online update checks, at the top of the library. It shows only
 * while they are off and the user has not said "Not now"; it says in one sentence what is sent.
 */
export function UpdatesPrompt({
  updates,
  className,
}: {
  updates: UpdatesController;
  className?: string;
}) {
  if (!updates.promptVisible) return null;
  return (
    <Banner
      tone="info"
      title={t.title}
      className={className}
      actions={
        <>
          <Button size="compact" onClick={updates.dismissPrompt}>
            {t.notNow}
          </Button>
          <Button size="compact" variant="primary" onClick={() => updates.setEnabled(true)}>
            {t.turnOn}
          </Button>
        </>
      }
    >
      {t.text}
    </Banner>
  );
}
