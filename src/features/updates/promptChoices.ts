// What the first-run prompt offers and what "Turn on" does with the ticks.

export interface PromptSwitches {
  mods: boolean;
  app: boolean;
}

/**
 * The switches the prompt offers: those that are still off. A user who already turned on mod
 * updates is offered only the app's; nothing that is on is offered again.
 */
export function promptOffers(modsEnabled: boolean, appEnabled: boolean): PromptSwitches {
  return { mods: !modsEnabled, app: !appEnabled };
}

/** Both settings must be known, something must be left to offer, and "Not now" must not be said. */
export function promptShown(
  modsEnabled: boolean | null,
  appEnabled: boolean | null,
  dismissed: boolean,
): boolean {
  if (modsEnabled === null || appEnabled === null || dismissed) return false;
  const offers = promptOffers(modsEnabled, appEnabled);
  return offers.mods || offers.app;
}

/** What "Turn on" switches on: the offered switches that are ticked. */
export function promptTurnOn(offers: PromptSwitches, ticked: PromptSwitches): PromptSwitches {
  return { mods: offers.mods && ticked.mods, app: offers.app && ticked.app };
}

/** "Turn on" needs at least one tick to do anything. */
export const anyTicked = (choice: PromptSwitches): boolean => choice.mods || choice.app;
