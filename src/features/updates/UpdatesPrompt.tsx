import { useState } from 'react';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Checkbox } from '../../ui/Checkbox';
import { Banner } from '../../ui/Feedback';
import type { AppUpdatesController } from '../appUpdate/useAppUpdates';
import { anyTicked, promptOffers, promptShown, promptTurnOn } from './promptChoices';
import type { UpdatesController } from './useUpdates';

const t = strings.updates.prompt;

/**
 * The first-run offer to turn on the two online checks (mod updates and app updates), at the top
 * of the library. Both are ticked; "Turn on" switches on what is ticked and closes the offer, and
 * "Not now" closes it for good. It offers only what is still off, and says in a line for each
 * what is sent.
 */
export function UpdatesPrompt({
  updates,
  appUpdates,
  className,
}: {
  updates: UpdatesController;
  appUpdates: AppUpdatesController;
  className?: string;
}) {
  const [ticked, setTicked] = useState({ mods: true, app: true });
  const modsEnabled = updates.settings?.enabled ?? null;
  const appEnabled = appUpdates.settings?.enabled ?? null;
  if (!promptShown(modsEnabled, appEnabled, updates.promptDismissed)) return null;

  const offers = promptOffers(modsEnabled === true, appEnabled === true);
  const turnOn = promptTurnOn(offers, ticked);
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
          <Button
            size="compact"
            variant="primary"
            disabled={!anyTicked(turnOn)}
            onClick={() => {
              if (turnOn.mods) updates.setEnabled(true);
              if (turnOn.app) appUpdates.setEnabled(true);
              updates.dismissPrompt();
            }}
          >
            {t.turnOn}
          </Button>
        </>
      }
    >
      <span className="mm-prompt__lead">{t.lead}</span>
      <span className="mm-prompt__choices">
        {offers.mods && (
          <Checkbox checked={ticked.mods} onChange={(mods) => setTicked({ ...ticked, mods })}>
            <span className="mm-prompt__label">{t.mods.label}</span>
            <span className="mm-prompt__detail">{t.mods.detail}</span>
          </Checkbox>
        )}
        {offers.app && (
          <Checkbox checked={ticked.app} onChange={(app) => setTicked({ ...ticked, app })}>
            <span className="mm-prompt__label">{t.app.label}</span>
            <span className="mm-prompt__detail">{t.app.detail}</span>
          </Checkbox>
        )}
      </span>
    </Banner>
  );
}
