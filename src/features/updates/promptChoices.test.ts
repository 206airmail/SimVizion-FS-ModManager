import { describe, expect, it } from 'vitest';
import { anyTicked, promptOffers, promptShown, promptTurnOn } from './promptChoices';

describe('promptOffers', () => {
  it('offers both on a first run, and only what is still off afterwards', () => {
    expect(promptOffers(false, false)).toEqual({ mods: true, app: true });
    expect(promptOffers(true, false)).toEqual({ mods: false, app: true });
    expect(promptOffers(false, true)).toEqual({ mods: true, app: false });
    expect(promptOffers(true, true)).toEqual({ mods: false, app: false });
  });
});

describe('promptShown', () => {
  it('waits until both settings are known', () => {
    expect(promptShown(null, false, false)).toBe(false);
    expect(promptShown(false, null, false)).toBe(false);
  });

  it('shows while something is off and "Not now" was not said', () => {
    expect(promptShown(false, false, false)).toBe(true);
    expect(promptShown(true, false, false)).toBe(true);
  });

  it('stays away once dismissed, or when everything is already on', () => {
    expect(promptShown(false, false, true)).toBe(false);
    expect(promptShown(true, true, false)).toBe(false);
  });
});

describe('promptTurnOn', () => {
  const both = { mods: true, app: true };

  it('turns on both when both are offered and ticked', () => {
    expect(promptTurnOn(both, both)).toEqual(both);
  });

  it('turns on only the ticked ones', () => {
    expect(promptTurnOn(both, { mods: true, app: false })).toEqual({ mods: true, app: false });
    expect(promptTurnOn(both, { mods: false, app: true })).toEqual({ mods: false, app: true });
  });

  it('never turns on a switch that was not offered', () => {
    expect(promptTurnOn({ mods: false, app: true }, both)).toEqual({ mods: false, app: true });
  });
});

describe('anyTicked', () => {
  it('is false only when nothing is ticked', () => {
    expect(anyTicked({ mods: false, app: false })).toBe(false);
    expect(anyTicked({ mods: false, app: true })).toBe(true);
  });
});
